# Roadmap & Progress

## Vertical slice (First Playable Milestone) — status

"Verified" says *how* each item was checked. "Headless run" = two real app processes (DM host + player)
on a virtual X display with software Vulkan, connected over TCP, scripted via `DND55_AUTO` (see
`apps/dnd_desktop/src/autotest.rs`); screenshots in `docs/screenshots/`. Mouse-driven UI flows could not
be clicked in this environment and are marked accordingly.

| # | Requirement | Status | Verified by |
|---|---|---|---|
| 1 | DM launches the desktop app | ✅ | Headless run |
| 2 | DM creates a campaign | ✅ | `Store::create_campaign` tests; menu button implemented (not clicked headless) |
| 3 | Two connected rooms | ✅ | Demo campaign + `CreateRoom`/`ConnectRooms` session tests; World tab UI implemented |
| 4 | Customize a room with tiles & objects | ✅ | Paint/fill/rect/object/undo session tests; editor UI implemented (not clicked headless) |
| 5 | Player connects | ✅ | TCP integration tests + headless run |
| 6 | Player creates a basic character | ✅ | Rules validation tests; host rejects invalid sheets; creator UI implemented (not clicked headless) |
| 7 | Player uploads a PNG portrait | ✅ | Upload/validate/re-encode tests; portrait token visible in headless run |
| 8 | Token appears on the map | ✅ | Headless run |
| 9 | Player moves on the grid | ✅ | Session tests (speed, Dash, OA) + headless run |
| 10 | DM spawns a monster | ✅ | Session tests + headless run |
| 11 | Both roll dice | ✅ | Session/TCP tests; player roll shown in DM and player dice trays in headless run |
| 12 | DM starts a combat encounter | ✅ | Session tests + headless run (initiative tracker) |
| 13 | Basic attack with 2024 rules | ✅ | Rules + session tests (hit/miss/crit/Sap/Vex/Graze/auto-crit) |
| 14 | Floating damage text | ✅ | Headless run ("-4" over the goblin) |
| 15 | DM authorizes movement to the connected room | ✅ | Session tests (players can't) + headless run |
| 16 | Campaign saves and loads | ✅ | Persistence tests; autosave on room transition observed in headless run's DB |

**Not verified here:** audio output (container has no sound device; WAV generation is tested), real
GPU rendering (software renderer used), a running Windows build (the code compiles for Windows; see
README), mouse-driven flows listed above.

## Phases

| Phase | Scope | Status |
|---|---|---|
| 1 Research & architecture | SRD 5.2.1 study, stack, architecture, data models, legal | ✅ (this repo's docs) |
| 2 Core desktop engine | Workspace, Bevy app, menu, grid, camera, input, loop | ✅ |
| 3 Map editor | Tiles, library, layers, collision, tools, serialization | 🟡 core tools done; multi-select/transform/copy/templates/test-play ⏳ |
| 4 Campaign graph | Rooms, edges, transitions, room state | 🟡 logic done; visual node-graph canvas ⏳ (list UI today) |
| 5 Multiplayer | Hosting, lobby, join, sync, authority, reconnect | 🟡 done for LAN/port-forward/VPN; relay ⏳; ready-check lobby screen ⏳ |
| 6 Character system | Creation, sheets, portraits, progression, inventory | 🟡 level-1 creation & sheet; leveling, inventory management ⏳ |
| 7 Rules & combat | Dice, initiative, actions, reactions, conditions, spells | 🟡 see RULES_COVERAGE; spells ⏳ |
| 8 Lighting & graphics | Fog, LOS, lights, VFX, damage feedback | 🟡 done for lights/darkvision/fog; spell VFX library ⏳ |
| 9 Audio & atmosphere | Music, ambience, SFX, polish | 🟡 synthesized SFX + volumes; music/ambience import ⏳ |
| 10 Persistence & systems | Saves ✅; quests, scripted events, encounters, import/export | 🟡 saves/export done; events/quests ⏳ |
| 11 Quality & release | Profiling, installer, docs | ⏳ |

## Next milestones (proposed order)

1. **Spellcasting core** — spell data model, slots, save DCs, spell attacks, concentration; first
   spells: *Light*, *Fire Bolt*, *Cure Wounds*, *Magic Missile*, *Shield* (reaction prompt). Light
   spells create light sources (engine already supports them).
2. **Class features L1–5** for all 12 classes (Rage, Sneak Attack, Second Wind, Action Surge, Extra
   Attack, Bonus Actions) + level-up flow and rests.
3. **Bestiary as data** — load SRD monsters from JSON packs with a checker against `reference/`.
4. **Editor 2.0** — selection, move/rotate/duplicate, copy/paste, map templates, test-play mode,
   visual graph canvas.
5. **Relay + QUIC** — join by code without port forwarding; reconnect across IP changes.
6. **Event system** — triggers (enter area, interact, creature defeated) → conditions → actions
   (open door, reveal, spawn, dialogue, award item), editable without code.
7. **Inventory & loot**, **DM drawing tools**, **music/ambience packs**.
8. **Windows installer** (WiX/MSI or Inno Setup), crash reporting, performance profiling on large maps.
