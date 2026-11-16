# Vendored dependencies

## winit 0.30.13 (Apache-2.0, see `winit-0.30.13/LICENSE`)

Unmodified upstream source except `src/platform_impl/windows/monitor.rs`. Upstream asserts that every
display mode from `EnumDisplaySettingsExW` has width, height, bit depth and refresh rate. Under Wine,
and with some virtual or remote-desktop display drivers, a mode can lack a field. Bevy enumerates video
modes at startup, so the assertion crashed DND55 before the window appeared. The patch skips such modes
instead (search for "DND55 patch"). Remove the `[patch.crates-io]` entry in the root `Cargo.toml` once
upstream handles this.
