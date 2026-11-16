# Legal & Content Licensing

## Rules content — SRD 5.2.1 (CC-BY-4.0)

All rules content comes from the System Reference Document 5.2.1, which Wizards of the Coast
publishes under the Creative Commons Attribution 4.0 International License. The SRD's Legal
Information page requires this exact attribution, which the app shows on its main menu and which
is reproduced here:

> This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of
> the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the
> Creative Commons Attribution 4.0 International License, available at
> https://creativecommons.org/licenses/by/4.0/legalcode.

The SRD also asks that no *other* attribution to Wizards be included. The product may describe itself
as "compatible with fifth edition" or "5E compatible". Avoid Wizards' trademarks (logos, "Dungeons &
Dragons" branding) in the product name, store listings and art. The working title in the code is a
placeholder that should be replaced before any public release.

**Not in the SRD, so not included:** Player's Handbook / DMG / Monster Manual content beyond the SRD
(other subclasses, backgrounds, feats, spells, monsters, named settings and characters). Users can add
their own homebrew. Content they import from books they own stays in their local files and is never
redistributed by the app.

`reference/srd-5.2.1.txt` is a machine extraction of the SRD PDF, kept so rules can be verified offline.
It falls under the same license and attribution.

## Art, audio, fonts

* **Tiles/objects/icons/menu scene/materials:** procedurally generated at runtime by
  `terrain_art.rs`, `icons.rs`, `menu_scene.rs` and `theme.rs`; water and lava come from the
  `shaders/liquid.wgsl` shader. This is original work, so there is no third-party art.
* **Sound effects:** synthesized at startup by `audio.rs` (original).
* **Fonts:** three SIL OFL 1.1 fonts are embedded (licenses in `apps/dnd_desktop/assets/fonts/` and in
  the release's `LICENSE.txt`): *UnifrakturCook* (Blackletter titles), *Grenze Gotisch* (headings) and
  *Crimson Text* (body text). They come from the Google Fonts repository and were chosen for their
  license and Italian character coverage. egui's and Bevy's bundled fonts are used as symbol fallbacks.
* **Art direction references:** the reference images supplied by the project owner (illustrated
  medieval emblems, Blackletter alphabet, tile sheets) only inspired the style. Nothing was traced or
  copied; every icon, tile, object and the menu scene is drawn by code in this repository.
* **Portraits:** uploaded by users. The host re-encodes them and keeps them inside the campaign.
* **Future asset packs** must ship with a license file; the importer will record source and license for each pack.

No Pokémon assets or code, and no unlicensed D&D artwork, are used. The map editor follows the
*workflow* of tile editors in general, not any proprietary implementation.
