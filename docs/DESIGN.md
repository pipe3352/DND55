# D&D 5.5 Desktop RPG — Technical Design

Status: **living document** · Ruleset: **SRD 5.2.1 (2024 revised rules)** · Last updated: 2026-11-15

This document is the architecture reference. What is *built* vs *planned* is tracked in
[`ROADMAP.md`](ROADMAP.md); rules coverage in [`RULES_COVERAGE.md`](RULES_COVERAGE.md).

---

## 1. Product vision

A desktop fantasy RPG in which one person — the **Dungeon Master** — builds a world of rooms on a
grid, connects them into a campaign graph, and runs a live session for friends who join over the
network with their own characters. It should feel like a 2D RPG video game (animated tokens, light
and darkness, spell and weapon effects, floating combat text, atmospheric audio) while the rules are
those of the **2024 revised fifth edition** as published in SRD 5.2.1.

**Core loop**

```
DM prepares            Session                                   Between sessions
──────────             ───────                                   ────────────────
map editor  ─┐   players join ─► explore room (fog, light) ─┐    autosaves / named saves
room graph  ─┼─►  DM narrates ◄─ interact, roll checks      │    character library
monsters    ─┘   DM starts combat ─► initiative ─► turns ───┤    campaign export
                 DM moves party to next room (graph edge) ◄─┘
```

The DM always has the final word: every automated result can be overridden (HP, conditions,
teleport, initiative order, travel without an edge), and overrides are logged as such.

## 2. Technology stack

| Concern | Choice | Why |
|---|---|---|
| Language | **Rust 1.97.1** (pinned in `rust-toolchain.toml`) | Required; safety + performance; one language for host and client. |
| Engine / rendering | **Bevy 0.20** (ECS, 2D sprites, Text2d, gizmos, audio, wgpu) | Rust-native game engine; ECS suits many tokens/effects; cross-platform via wgpu (DX12/Vulkan/Metal). |
| UI | **egui 0.36 via bevy_egui 0.43** | Mature immediate-mode UI for dense editor/DM tooling; themed as parchment. Bevy UI may later replace in-world HUD pieces. |
| Async / networking | **Tokio** + framed TCP (length-prefixed JSON) | Simple, debuggable, reliable ordering. QUIC (quinn) is the planned upgrade when a relay is added (§6). |
| Serialization | **serde / serde_json** | Human-inspectable saves and wire messages; schema evolution via format versions. |
| Persistence | **SQLite (rusqlite, bundled)** | Single-file, transactional, crash-safe, no server. |
| Images | `image` crate (PNG) | Validating/normalizing untrusted portrait uploads. |
| File dialogs | `rfd` | Native "open PNG" dialogs. |
| Audio | Bevy audio (rodio) | WAV/OGG playback; current SFX are synthesized at startup (license-free). |

Rejected: **Tauri/web UI** (would make the game a web page in a window — explicitly not wanted);
**WebSockets** (no benefit for a native client; easy to add later for a web spectator).

## 3. System architecture

```
                          ┌───────────────────── dnd_desktop (Bevy app) ─────────────────────┐
                          │  menu · character creator · HUD (egui)   map/tokens/fx (Bevy ECS) │
                          │  net_sync ◄── NetClient (own thread) ──────────────┐              │
                          └──────────────────────────────────────────────────── │ ────────────┘
                                                                                │ TCP (JSON frames)
   DM machine only ──► HostRuntime (Tokio) ─► dnd_net::host ◄───────────────────┘   ◄── other players
                                                │  one task owns the Session; per-connection tasks parse frames
                                                ▼
                         dnd_session  (pure state machine: Command → validate → apply → Events + Views)
                           │            │                         │
                           ▼            ▼                         ▼
                      dnd_rules     dnd_world                dnd_persist (SQLite) ◄─ save hook
                     (rules math)  (maps, graph, path, LOS, light)
```

**Crates and boundaries**

| Crate | Responsibility | Depends on | IO? |
|---|---|---|---|
| `dnd_rules` | SRD rules: abilities, dice, d20 tests, damage, HP/death, conditions, equipment, characters, monsters, attacks | — | no |
| `dnd_world` | Grid maps & layers, asset catalog, A* (2024 grid rules), LOS, lighting & vision, room graph | — | no |
| `dnd_session` | Authoritative game state, command validation, combat flow, per-principal views (hidden-info boundary) | rules, world | no |
| `dnd_persist` | SQLite saves, migrations, backups, export/import, character library | session | disk |
| `dnd_net` | Host (listen server) + client transport, auth, rate limiting | session | network |
| `dnd_desktop` | Rendering, input, UI, audio, localization, hosting glue | all | yes |

Rules (`dnd_rules`) know nothing about Bevy, sockets or SQL and are tested in isolation. The session
is IO-free, so authority and hidden-information rules are unit-testable without a network.

**Planned modules** (not yet crates): `dnd_events` (trigger/condition/action scripting), `dnd_assets`
(asset-pack import/validation), `dnd_relay` (NAT relay service), `dnd_content` (data-file content packs).

## 4. Repository layout

```
dnd55/
├── Cargo.toml                 workspace, shared dependency versions
├── rust-toolchain.toml        pinned compiler
├── crates/
│   ├── dnd_rules/src/         ability, dice, d20, damage, hp, conditions, equipment, content,
│   │                          character, monster, attack, rng
│   ├── dnd_world/src/         ids, catalog, map, path, vision, graph
│   ├── dnd_session/src/       model, command, event, session, view, assets, demo, hexbytes
│   │              tests/      session_flow.rs (authority, hidden info, combat, travel …)
│   ├── dnd_persist/src/       lib.rs (store, migrations, export/import)
│   └── dnd_net/src/           protocol, host, client;  tests/multiplayer.rs (real TCP)
├── apps/dnd_desktop/
│   ├── src/                   main, state, menu, character_ui, ui_game, map_render, tokens,
│   │                          fx, input, audio, textures, theme, i18n, hosting, net_sync, autotest
│   └── i18n/                  en.json, it.json
├── docs/                      DESIGN, RULES_COVERAGE, ROADMAP, LEGAL, screenshots/
└── reference/                 SRD 5.2.1 extracted text (CC-BY-4.0) used to verify rules
```

Coding standards: `cargo fmt`, `cargo clippy --workspace --all-targets` clean, no `unwrap()` on
untrusted input, every rule cites its SRD section, every rules change ships with a test.

## 5. Data models

All persistent types derive `Serialize/Deserialize`; IDs are stable newtypes never reused within a
campaign (`RoomId(u32)`, `EdgeId(u32)`, `EntityId(u64)`, `ObjectId(u64)`, `PlayerId(u32)`).

```rust
GameState {                                  // everything saved; nothing UI-only
  format_version, ruleset: "srd-5.2.1", campaign_name, settings: Settings,
  graph: WorldGraph,                         // nodes = RoomNode, edges = RoomEdge
  rooms: BTreeMap<RoomId, RoomState>,        // map + entry + explored fog + extra lights
  party_room, dm_view_room,
  creatures: BTreeMap<EntityId, Creature>,   // PCs and monsters, each with a room + cell
  players: BTreeMap<PlayerId, PlayerInfo>,
  combat: Option<Combat>, custom_monsters, log: Vec<LogEntry>, id counters …
}
RoomNode  { id, name, description, dm_notes, region, chapter, secret, editor_pos }
RoomEdge  { id, from, to, one_way, secret, locked, prerequisite, label, arrival, return_arrival }
RoomState { map: GridMap, map_rev, entry, visited, explored: Vec<bool>, extra_lights, music }
GridMap   { width, height, palette: Vec<PaletteEntry>, layers: [Layer; 6], objects: Vec<MapObject>,
            ambient: Bright|Dim|Darkness, background, indoor }
Layer     { kind: Terrain|FloorDecoration|Walls|Furniture|Overhead|Effects, visible, locked, tiles: Vec<u16> }
MapObject { id, asset_id, x, y, w, h, rotation, hidden, blocks_movement, blocks_sight, difficult,
            light, interactable, open, label, dm_notes }
Creature  { id, name, room, pos, kind: Pc{sheet} | Monster{template_id}, controller: Dm|Player(id),
            faction, size, hp: HitPoints, conditions: ConditionSet, armor_class, speed_ft, senses,
            defenses, attacks: Vec<AttackProfile>, initiative_bonus, hidden, portrait: Option<sha256>,
            effects: {sapped_by, vex, dodging}, carried_light }
CharacterSheet { name, species, size, class, level, background, score_method, base_scores,
                 background_bonus, class_skills, species_skill, body_armor, shield, weapons,
                 weapon_masteries, experience_points, biography }   // derived stats are computed
Combat    { room, round, order: Vec<InitiativeEntry>, index, turn: TurnState, reactions_used,
            pending: Option<PendingReaction>, paused }
```

Derived values (AC, HP max, attack bonuses, saves, skills, initiative) are **computed from the sheet**,
never stored, so they always update automatically.

## 6. Networking design

**Model: authoritative listen server.** The DM's application runs `dnd_net::host` in-process on a
Tokio runtime. One game task owns the `Session`; connection tasks only frame/parse and forward
messages, so all game logic is serialized (no races). The DM's own UI connects over loopback with a
random 128-bit **DM secret** — the same code path as players.

**Joining:** players enter `host:port` + a 6-character **session code**. The `Hello` carries the
protocol version (mismatch → rejected), name, and optionally a **resume token** (returned in
`Welcome`) which reclaims the same seat and character after a disconnect.

**Per-message flow:** `ClientMsg::Cmd{seq, Command}` → `Session::apply(principal, cmd)` → on success
every connection receives (1) the events it may see and (2) a fresh `ClientView` built for its
principal; the map is only resent when its revision changed. On failure only the sender gets
`CmdResult{error}` and **state is untouched** (handlers validate before mutating; enforced by tests).

**Security:** clients are untrusted. Frames ≤ 12 MB; first message must be `Hello` within 10 s; token
bucket rate limit per connection; slow consumers are dropped (they can resume); all dice are rolled
by the host (ChaCha20, OS-seeded, unbiased rejection sampling); portraits are decoded, size-checked,
resized and re-encoded before storage; DM-only commands check the principal; a player's view never
contains hidden creatures, hidden objects, DM notes, secret rooms/edges, exact enemy HP/AC/attacks or
rooms the party hasn't visited (tests assert the serialized player payload contains no trace).

**Hosting options** (honest status):

| Option | Status | Notes |
|---|---|---|
| LAN | ✅ works | Players connect to the DM's LAN IP. |
| Internet via port-forward | ✅ works | DM forwards the TCP port. |
| Private overlay (Tailscale/ZeroTier) | ✅ works | Recommended today: no router changes. |
| Relay service (session code → relay) | ⏳ planned | `dnd_relay`: tiny forwarding service + QUIC; nobody needs port forwarding. |
| Dedicated headless host | ⏳ planned | Same `dnd_net::host` in a CLI binary. |

**Scaling of sync:** currently full filtered views per change (fine for ~6 players on small maps); the
planned optimization is per-client view diffing (send only changed creatures/cells).

## 7. Rules design

* **Source of truth:** SRD 5.2.1 (CC-BY-4.0), downloaded from dndbeyond.com and kept as text in
  `reference/` so every rule can be checked offline. Each implemented rule cites its section; tests
  reproduce the SRD's own worked examples (e.g. damage order 28 → 23 → 11 → 22, massive damage 12/6/18,
  healing 14+8 → 20, Leather AC 14 / Chain Mail AC 16).
* **2014 vs 2024 differences handled explicitly**, e.g. Exhaustion is −2×level on D20 Tests and −5 ft
  Speed per level (death at 6); weapon **Mastery** properties; background-based ability increases
  (+2/+1 or +1/+1/+1, max 20) and origin feats; Surprise = Disadvantage on Initiative; Invisible gives
  Advantage on Initiative; Unseen Attackers/Targets; grid diagonals cost 5 ft (variant alternating
  5/10 is an opt-in setting); monsters die at 0 HP unless the DM treats them as characters;
  2024 stat-block format with Initiative modifier/score.
* **Typed, data-driven:** weapons/armor/species/classes/backgrounds are static tables; monsters are
  `MonsterTemplate` data (custom templates live in the campaign; content packs planned).
  Provenance is recorded (`source: "srd-5.2.1" | "homebrew"`), keeping SRD, DM custom rules,
  homebrew and overrides distinct.
* **Pure functions + explainable results:** `resolve_attack` returns the d20(s), advantage and
  disadvantage *reasons*, each damage roll and the defenses applied; the combat log prints that
  breakdown. Animations only visualize these validated results.
* **No silent violations:** unimplemented features are refused or flagged rather than approximated
  (e.g. rolled ability scores are rejected until the host rolls them; Extra Attack is "not yet
  automated"; untrained armor produces an explicit warning). DM overrides are always logged.
* **Optional choices are never auto-consumed:** Opportunity Attacks pause movement and prompt the
  reactor's controller. (Graze/Sap/Vex apply automatically because they carry no downside for the user;
  Topple/Push/Slow — real choices — are pending a prompt UI.)

## 8. Map editor design

* **Grid:** square, 5 ft per cell, configurable size up to 256×256, background color, ambient light,
  indoor flag. Rendering: one sprite per non-empty tile per visible layer + objects; fog sprites per
  cell above tokens; grid via gizmos; zoom/pan camera.
* **Layers:** Terrain, Floor decoration, Walls, Furniture, Overhead, Effects — each hideable and
  lockable (locked layers reject edits on the host). Layer reordering is planned.
* **Palette & collision:** tiles reference a per-map palette entry that *copies* the asset's collision
  flags (blocks movement / sight / difficult) so the DM can edit collision per map without touching the
  library. Objects span multiple cells, rotate in quarter turns, can be hidden, interactable (doors,
  chests, levers) and light-emitting.
* **Tools (implemented):** play/select, pencil (drag), eraser, rectangle, bucket fill, place object,
  remove object, entry point, place light, undo/redo (64 steps per room, host-side snapshots),
  categorized searchable library with favorites. **Planned:** multi-select, move/rotate/scale/duplicate,
  copy/paste, templates, collision paint mode, test-play.
* **Assets:** `AssetDef { id, name, category, color, pattern, size, collision, light, sprite, tags }`.
  The built-in library is procedural placeholder art (generated at runtime → no third-party art).
  Asset packs (PNG/spritesheets + JSON metadata, validated on import) plug into the same registry.

## 9. Save system

* SQLite `campaigns.db` in the user data dir (`%APPDATA%\dnd55` on Windows), WAL + `synchronous=FULL`.
* Tables: `campaigns`, `saves(kind manual|auto|checkpoint, label, format_version, state_json, sha256)`,
  `assets(hash, bytes)` (content-addressed), `characters` (player's local library).
* Every save is **one transaction** → a crash mid-save leaves earlier saves intact (tested by aborting a
  transaction). Each save is checksummed; loading verifies it and `load_latest_valid` falls back to the
  newest intact save (tested with deliberate corruption).
* Autosaves every 3 minutes when state changed and on every room transition; newest 5 kept; manual
  saves and checkpoints never pruned. Online backup via `VACUUM INTO`.
* Versioning: `PRAGMA user_version` for the schema; `GameState.format_version` with JSON migration
  steps; saves from newer builds are refused instead of misread.
* Export/import: single JSON bundle (state + assets, hash-verified).
* UI state (camera, panels, selections) is deliberately **not** saved with the campaign.

## 10. UI/UX plan

* **Main menu:** Play (quick demo), Create, Join, Load, Character Manager, Settings, Exit; SRD
  attribution footer.
* **In game:** the map fills the center. Top bar (campaign, room, round, code/port, save, leave);
  left DM panel with tabs *Editor / World / Monsters / Session*; right panel with initiative, party
  health, character sheet; bottom panel with dice tray (quick dice, expressions, advantage/disadvantage,
  private/hidden rolls) and adventure log + chat. Contextual windows: combat action bar on your turn,
  Opportunity Attack prompt, character creator, toasts, room-title fade, "Your turn!" banner.
* **Visual language:** parchment fills, dark wood borders, gold accents, red/blue/green resource bars
  (matching the provided character-sheet mock), torch glows with flicker, gray darkvision wash.
* **Accessibility:** UI scale 0.75–1.75, keyboard camera (WASD/arrows), all strings localized
  (English, Italian) via key lookup with tests that every key exists in both languages.
* **Planned:** dedicated graph-editor canvas (drag nodes, drag edges), radial token menus,
  configurable keybindings, 3D dice option, gamepad.

## 11. Development roadmap

See [`ROADMAP.md`](ROADMAP.md) for the phase plan and the vertical-slice checklist with verification
status.

## 12. First implementation

The vertical slice is implemented end to end (rules → session → persistence → network → client) and
verified by 103 automated tests plus a headless two-process run (DM host + player over TCP, rendered
with software Vulkan) whose screenshots are in `docs/screenshots/`. See ROADMAP for exactly which
items were verified how.
