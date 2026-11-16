# Roadmap & Progress

## How things were verified

| Method | What it means |
|---|---|
| **Unit/integration tests** | `cargo test --workspace`: 150+ tests (rules, world, session, persistence, real TCP multiplayer, relay, client logic, i18n coverage). |
| **Headless run** | Two real app processes (DM host and player) on virtual X displays with software Vulkan, connected over TCP and driven by `DND55_AUTO` (see `apps/dnd_desktop/src/autotest.rs`). Screenshots are in `docs/screenshots/`. |
| **Click test** | `scripts/gui-click-test.sh`: real mouse events (xdotool) in two game windows. The player casts Fire Bolt from the spellbook at a goblin, and the DM drags a monster from the bestiary onto the map. It asserts on the adventure log. |
| **Wine** | The Windows `.exe` started under Wine on Linux (not real Windows). |
| **Manual (Windows)** | Not done yet. Use [`WINDOWS_ACCEPTANCE_CHECKLIST.md`](WINDOWS_ACCEPTANCE_CHECKLIST.md). |

## Milestone 4 — Editor fixes, object library, tokens, Character Manager (this release)

| Area | Status | Verified by |
|---|---|---|
| Mouse wheel/clicks reach the map only over the map canvas (panels, lists, windows scroll themselves) | ✅ | `pointer` unit tests + `scripts/gui-wheel-test.sh` (real xdotool wheel events; fails on the old code, passes now) |
| +81 objects (Natural: trees/plants/rocks; Furniture: tables/seating/storage/decor; Dungeon: architecture/atmosphere/gameplay), each with its own art | ✅ | Art tests (no generic fallback, pairwise distinct); contact sheet `docs/screenshots/after/objects.png` |
| Browser: search (names + tags), category chips, sub-group chips, thumbnails, favorites, recent | ✅ | Screenshot; real-input scroll test |
| Object size (scaling), rotation, collision, light, draw layer (floor/standard/overhead), save/load, multiplayer sync | ✅ | World tests (old saves load, round-trip of every object, doors/gates open) |
| PNG art packs with license manifest (`asset_packs/`) | ✅ | Unit test (license required, bad ids/files rejected). Rendering of a real pack not screenshot-tested |
| Illustrated tokens: 14 class icons + 14 monster-type emblems, side shown by frame shape, shadow, selection/turn rings, HP bar, optional names; chooser + preview in the creator | ✅ | Token tests (distinct emblems, distinct frame shapes); screenshots |
| Character Manager: resizable, maximize/restore, remembered size, 3 panels that stack when narrow, search/create/delete | ✅ | Window-rect unit test; real xdotool session (open, new, maximize, restore, drag-resize to 800×600 → stacked layout, size saved) |

## Milestone 3 — Visual overhaul

| Area | Status | Verified by |
|---|---|---|
| Design system: palette, OFL Blackletter/gothic/serif fonts, materials, components, icon set | ✅ | Font coverage + icon tests; screenshots |
| Main menu scene, creator class cards, sheet medallions, HUD (initiative, dice, banners, titles) | ✅ | Screenshots (before/after) |
| Terrain materials, variants, organic transitions, AO, shadows, decor, illustrated objects | ✅ | Art tests (sizes, seamless edges, masks); screenshots |
| Animated water and lava shader; lava as a light source | ✅ | Light test; screenshots (software Vulkan). DX12 on real Windows not verified |
| Soft fog of war without leaks | ✅ | Unit test |
| Weather/atmosphere, combat VFX, crit shake, quality presets, reduced motion, FPS cap | ✅ | Screenshots; settings tests |
| Map editor workspace (toolbar, palette, inspector, layers bar, minimap, overlays, context menu, preview) | ✅ | Screenshot; drag-from-bestiary click test |
| Campaign graph editor (cards, styled edges, drag-connect, edge editing, layout, minimap, regions, confirmed travel) | ✅ | Layout/reachability/secrecy tests; screenshot. Canvas interactions not click-tested |

## Milestone 2 — "Complete game + Windows release" (this release)

| Area | Status | Verified by |
|---|---|---|
| A. Spellcasting (339 SRD spells; 31 fully + 61 partly automated; slots, prepared spells, concentration, AoE, upcasting, light spells, reactions) | ✅ | Gameplay tests (light in a dark cave, Fireball, Magic Missile, Hold Person concentration, Shield) + click test + screenshots |
| B. Class features, subclasses, level 1–20 progression, rests, XP | ✅ (see RULES_COVERAGE for per-class gaps) | Gameplay tests (Rage, Sneak Attack, Extra Attack/Surge/Second Wind, leveling, rests) |
| C. Bestiary: 328 SRD monsters, search/filters/stat blocks, drag-to-map, custom monster editor, save actions with recharge | ✅ | Dragon breath test + click test (drag-to-map) + screenshots |
| D. Map editor: select, box/Shift multi-select, drag-move, copy/paste/duplicate, rotate, delete, properties panel, container loot, layer order/visibility/lock, undo/redo, drawing tools | ✅ | Editor multi-object/undo test; screenshots. Drag-move and shortcuts not exercised by the click test. |
| E. Visual campaign graph (drag rooms, connect, delete edges, travel, delete room, new room) | ✅ | Screenshot; session tests for edges/transitions. Canvas interactions not click-tested. |
| F. Online multiplayer: relay server + invites, LAN/direct invites, UPnP, auto-reconnect, ping, friendly errors | ✅ relay/LAN; 🟡 UPnP | Relay integration test (join, bad invite, rejoin). UPnP untested (no router available). |
| G. Scripted events editor (traps, levers, chest rewards, boss defeat, dialogue with choices, skill checks) | ✅ | Scripts test; screenshot of the editor |
| Inventory/loot, potions, music/ambience, spell VFX | ✅ | Tests (potions, chests, give); synthesized music/ambience unit test; VFX seen in screenshots |
| Windows: DND55.exe, installer, portable zip, data dirs, crash reports, GPU backend fallback, no-audio fallback | 🟡 | Built and Wine-tested; **not run on real Windows** |

## Earlier milestone — First playable vertical slice

All 16 requirements are done: hosting, joining, character creation with a portrait, map editing,
movement, monsters, dice, initiative, 2024 attack rules, floating damage, DM-controlled travel, and
saving/loading. Tests and headless runs cover them.

## Known limitations

* No real-Windows test yet (see the checklist). No code signing, so SmartScreen warns on first run.
* Network traffic isn't encrypted (TLS ⏳). No public relay is provided.
* About 250 spells and many higher-level class features are "DM adjudicates" (the text is shown and the DM resolves them).
* Large creatures occupy one cell for movement and targeting.
* Music and ambience are simple procedural loops; importing audio packs is ⏳.
* The UI's rule texts (spell and monster descriptions) are in English only. The SRD is English.
* Audio output and real-GPU rendering weren't observed in the development environment (no sound
  device, software renderer only).

## Next milestones

1. **Windows acceptance pass** on 2–3 real PCs (NVIDIA/AMD/Intel), fix what it finds, sign the installer.
2. **TLS for game traffic** (rustls) and relay authentication; optionally a hosted relay.
3. **More automation**: Counterspell, Turn Undead, Wild Shape, Metamagic, Cunning Strike, Help/Hide/Grapple/Shove actions, cover.
4. **Large creatures** (multi-cell footprint), map templates and test-play mode in the editor.
5. **Audio packs** (import OGG/WAV with license metadata) and a richer music system.
6. **Quests and journal**, shops and magic items.
