# Development progress (compact handoff)

## Current request: editor fixes, asset expansion, tokens, Character Manager

| # | Item | State |
|---|---|---|
| 1 | Wheel/click routing: map zooms/clicks only over the map canvas | ✅ `src/pointer.rs` (`MapPointer`, computed in `ui_game::game_ui` after panels; used by `input.rs`, bestiary drop). Tests: `pointer::tests` |
| 2 | Object library +81 objects (Natural/Furniture/Dungeon), distinct procedural art, group filters, layers, size, packs | ✅ catalog table in `dnd_world/src/catalog.rs`; art in `src/object_art.rs`; `ObjectLayer` + `is_door` in `dnd_world/src/map.rs`; filters in `editor_ui::palette`; PNG packs `src/asset_packs.rs` |
| 3 | Fantasy class tokens replacing red discs | ✅ `src/token_art.rs`, `tokens.rs` (`TokenArt`, `token_indicators`, `token_egui`), `CharacterSheet.token_icon`, `CreatureView.token`, picker in `character_ui::token_picker` |
| 4 | Character Manager resizable/maximizable 3-panel window | ✅ `src/char_manager.rs`; size in `GameSettings.char_window(_max)` |
| 5 | Full tests, clippy, GUI click test, Windows build | ✅ 172 tests, clippy clean, click + wheel GUI tests pass, Windows cross-build + Wine (Vulkan) |

## UI restyle (dark gothic, per user mockups) — done
* `src/chrome.rs`: dark panels, gold frames, tile buttons, top tabs, pills, gold buttons, red gem dice,
  list rows, segmented control, decorations (dragon crest, banner, candle, skull, wax seal, compass, archway).
* Menu (`menu.rs`, `menu_scene.rs`): crest + title, banner, plaques with chevrons (open page/Continue glow),
  top-right status/name/settings/help, bottom bar (version, SRD link, licenses, support), moonlit scene.
* Game (`ui_game.rs`): top bar with view tabs (Campaign / Map Editor / Character Manager → `UiState.show_party`,
  players → `show_sheet`), room row with room switcher (`ViewRoom`) + editor tools, dark left panel (tile tabs),
  framed map, dark dice/log panel with local timestamps (`platform::local_hhmm`, `Presentation.log_stamps`).
* Editor (`editor_ui.rs`): Paint Mode tiles, 2-column asset library (lists | grid + sub-group chips), Room Details
  inspector (rename, size, ambient segmented, entry, music/ambience with local preview `UiState.audio_preview`),
  status bar (layer combo with per-layer settings, grid, snap, object/effect view filters), minimap with compass/zoom.

## Decisions
* Egui's `is_pointer_over_egui` is wrong for our hand-made background root `Ui`; never use it for map input — use `MapPointer`.
* Left press is captured by where it started (`press_on_map` Local in `input::map_click`).
* Object sub-group = first tag (`tree`, `plant`, `rock`, `table`, `seating`, `storage`, `decor`, `architecture`, `atmosphere`, `gameplay`).
* `MapObject.layer` is `#[serde(default)]` (saves/net are JSON) → old saves load as Standard.
* Art packs are client-local overrides keyed by object id and require a license field.

## Next step
Request complete. Open ideas: translate object names (IT), pack art in palette thumbnails, multi-cell
tokens for Large creatures, real-Windows acceptance pass.

## Test helpers
* `scripts/gui-wheel-test.sh` — real wheel events (needs Xvfb, xdotool, ImageMagick).
* `DND55_ART_SHEET=… cargo test dump_contact_sheet -- --ignored`, `DND55_TOKEN_SHEET=… cargo test dump_token_sheet -- --ignored` — raw RGBA contact sheets.
* `DND55_SEED_DB=<data>/campaigns.db cargo test seed_demo_characters -- --ignored` — two demo characters.

## DM combat controls (initiative & HP) — done
* Session (`dnd_session`): `DmCommand::{SetInitiative, RerollInitiative, SetActiveCombatant, AddToCombat,
  RemoveFromCombat, ManualHp{op: HpOp}, UndoHp}`; `GameState.hp_undo` (serde default, cap 20) restores
  HP, conditions and the initiative entry. Re-sort is stable by total (DM's tie order kept), the active
  turn/round are never changed except by `SetActiveCombatant`. Tests: `tests/dm_combat_controls.rs`.
* Client: `ui_game::combat_tracker` (click = select, drag cards = reorder, DragValue = edit initiative,
  right-click menu) shown also in the Map Editor during combat; `ui_dm::hp_panel` (Damage rules/direct,
  Heal, Set HP, max, temp, restore, set 0 with confirm, undo).

## Big expansion request (map editor + RPG systems) — status
| Phase | Item | State |
|---|---|---|
| 1 | Resize map (anchor, loss report/confirm, creatures never removed, deep undo) | ✅ `GridMap::resized`, `DmCommand::ResizeMap`, inspector "Resize map" |
| 1 | Universal eraser (layer/category/objects/creatures/lights/drawings/all, brush, drag preview, protect, locks, undo) | ✅ `DmCommand::EraseArea`, `UiState::erase_*` |
| 1 | Rotation: R/Shift+R, pre-placement rotation + rotated ghost, reset, creature facing (mirror + N/S pip) | ✅ (fixed old w/h swap bug) |
| 1 | +11 native pixel terrains (`pixel_tiles.rs`), +17 pixel objects | ✅ |
| 2 | Locks/keys/pick/force/break/inspect, player object menu, DM lock/break/description props, script SetObjectLocked | ✅ `Command::ObjectAction` |
| 2 | Levers/triggers | ✅ existed (scripts WHEN/IF(skill check)/THEN, depth-guarded); + lock/quest actions |
| 3 | DM journal (8 sections, share), quests (main/side, status, objectives hidden/optional, rewards, NPCs, DM notes), room notes | ✅ `journal_ui.rs`, `DmCommand::{SaveJournalEntry,…,SetRoomNotes}`, `ScriptAction::UpdateQuest` |
| 3 | Campaign graph improvements | existing graph editor kept (thumbnails, regions, search, drag, notes in DM data) |
| 4 | Merchants/shops (buy/sell, stock, prices, buy rate, funds, DM approval queue), condition durations (rounds, tick at end of turn, shown on tokens/inspector) | ✅ `model::Merchant`, `Command::{Buy,Sell}`, `DmCommand::{SaveMerchant,ResolveTrade,…}`, `world_extras.rs`, `shop_ui.rs` |
| 4 | Equipment slots UI, targeting/AoE preview polish, pixel condition icons | ⏳ not started (`GiveItem` trading already exists) |
| 5 | Campaign clock (day/time, rests advance it, DM popup in top bar), day/night tint on outdoor maps, 8 map templates, prefabs (save selection/place), roofs (Overhead layer above tokens, fades when party is under; DM Auto/Show/Hide) | ✅ `CampaignTime`, `DmCommand::{AdvanceTime,SetTime,SavePrefab,PlacePrefab,ApplyTemplate}`, `map_render::roof_visibility` |
| 5 | Elevation | ⏳ not started |

Next step: equipment slots, targeting previews, condition icons, elevation.
Tests: `tests/editor_tools.rs`, `tests/interactions_and_journal.rs`, `tests/shops_time_prefabs.rs`.
Top bar: below 1900 px wide, Invite/Save/Journal/Shop are icon-only and tabs sit left (no overlap).
