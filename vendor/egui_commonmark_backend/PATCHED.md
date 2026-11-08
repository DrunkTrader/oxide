# Patched copy of egui_commonmark_backend 0.25.0

Source: https://crates.io/crates/egui_commonmark_backend (MIT OR Apache-2.0, licence files kept).

One change, in `src/misc.rs`: bold text and headings also request weight 650–700 on the variable font's `wght` axis, so Markdown bold is rendered bold with Oxide's bundled Inter font. egui's `strong()` alone only changes the text colour.

Used through `[patch.crates-io]` in the workspace `Cargo.toml`. Drop the patch when upstream supports font weights.
