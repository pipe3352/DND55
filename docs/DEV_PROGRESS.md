# Development progress (compact handoff)

## Current request: editor fixes, asset expansion, tokens, Character Manager

| # | Item | State |
|---|---|---|
| 1 | Wheel/click routing: map zooms/clicks only over the map canvas | ✅ `src/pointer.rs` (`MapPointer`, computed in `ui_game::game_ui` after panels; used by `input.rs`, bestiary drop). Tests: `pointer::tests` |
| 2 | Object library +81 objects (Natural/Furniture/Dungeon), distinct procedural art, group filters, layers, size, packs | ✅ catalog table in `dnd_world/src/catalog.rs`; art in `src/object_art.rs`; `ObjectLayer` + `is_door` in `dnd_world/src/map.rs`; filters in `editor_ui::palette`; PNG packs `src/asset_packs.rs` |
| 3 | Fantasy class tokens replacing red discs | ✅ `src/token_art.rs`, `tokens.rs` (`TokenArt`, `token_indicators`, `token_egui`), `CharacterSheet.token_icon`, `CreatureView.token`, picker in `character_ui::token_picker` |
| 4 | Character Manager resizable/maximizable 3-panel window | ✅ `src/char_manager.rs`; size in `GameSettings.char_window(_max)` |
| 5 | Full tests, clippy, GUI click test, Windows build | ✅ 172 tests, clippy clean, click + wheel GUI tests pass, Windows cross-build + Wine (Vulkan) |

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
