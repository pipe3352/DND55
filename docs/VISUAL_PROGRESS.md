# Visual progress (compact handoff)

## Done (premium pixel-art pass)
* **Pixel engine** `apps/dnd_desktop/src/pixel.rs`: low-res canvas, 3-tone ramps (`ramp`), shaded
  primitives, auto 1 px ink outline, ground shadow, nearest sampling, `pixelize()` (2× downsample,
  colour quantize, hard/dithered alpha) for procedural art.
* **Characters & monsters** `sprites.rs`: 32×32 (dragons 48×48), paper-doll humanoid generator
  (headgear, face, armour, cape, wings, weapon, off-hand) + beast/spider/flyer/snake/slime/
  elemental/golem/ghost/eye/plant/mimic/fish/dragon drawers. Key = `class:<icon>` or
  `mon:<type>:<name>`; name keywords pick archetype + colours (goblin/hobgoblin/bugbear/orc/kobold,
  skeleton/zombie/ghoul/mummy/lich/vampire/ghost/wraith, oozes, wolves, bears, cats, spiders,
  bats/birds, snakes, elementals, fiends, celestials, golems, giants, dragons by colour…).
  Unknown monsters fall back on their creature type with hash-varied colours.
* **Tokens** `tokens.rs`: sprites on a side ground marker (friendly gold ring / hostile spiked red /
  neutral broken bronze); Large 1.8, Huge 2.6, Gargantuan 3.4 cells **visual only**; portraits still
  use the round frame. `token_egui` shows sprites in initiative, creator, bestiary list.
* **Objects** `pixel_objects.rs`: ~95 hand-drawn 3/4-view sprites (furniture, nature, dungeon,
  floor magic/traps). Others: pixelized procedural art (`image()` fallback). Palette thumbnails use them.
* **Terrain**: tiles, transitions (dithered edges) and decor pixelized in `map_render::ArtCache`.
* **Menu** `menu_scene.rs`: bigger castle + bridge, valley lights, moonlit foreground rocks (plus the
  earlier dark-gothic UI in `chrome.rs`).

## Remaining
* Hand-drawn sprites for the rest of the 328 bestiary entries that share archetypes (variants only).
* Pixel-native terrain tiles (currently pixelized procedural), wall tile detail, torch-flicker sprites.
* Menu: painted moon craters/clouds still procedural vector shapes.

## Checks
`cargo test -p dnd_desktop sprites pixel_objects`; sheets: `DND55_SPRITE_SHEET=… cargo test dump_sprite_sheet -- --ignored`,
`DND55_OBJ_SHEET=… cargo test dump_object_sheet -- --ignored`.
