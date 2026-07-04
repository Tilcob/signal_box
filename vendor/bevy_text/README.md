# Bevy Text

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/bevyengine/bevy#license)
[![Crates.io](https://img.shields.io/crates/v/bevy_text.svg)](https://crates.io/crates/bevy_text)
[![Downloads](https://img.shields.io/crates/d/bevy_text.svg)](https://crates.io/crates/bevy_text)
[![Docs](https://docs.rs/bevy_text/badge.svg)](https://docs.rs/bevy_text/latest/bevy_text/)
[![Discord](https://img.shields.io/discord/691052431525675048.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/bevy)

## Local modifications (Signal Box vendor)

This is upstream `bevy_text` (Bevy 0.18) with two glyph-rendering fixes applied.
Both work around text corruption we hit in the game's German UI under heavy menu
churn. They are **workarounds that hide the bug, not root-cause fixes**. Both
live in `src/font_atlas.rs`; grep for `STELLWERK` to find them.

1. **Atlas page size 512 → 2048** (`add_glyph_to_atlas`)
   Glyphs that spill onto a *second* atlas page render corrupted in Bevy 0.18 —
   their quads sample the wrong page's texture (wrong-size letters). A German UI
   with umlauts overflows a 512² page per font size once the 4 subpixel-bin
   variants per glyph accumulate. Bumping every page to 2048² (~16× capacity)
   means a second page never appears in practice, so the sampling bug never
   triggers. Root cause is in Bevy's multi-page atlas sampling.

2. **Isolated `SwashCache` per rasterization** (`get_outlined_glyph_texture`)
   Under rapid text churn the shared swash `ScaleContext` inside the app-wide
   `SwashCache` drifts and rasterizes glyphs at the wrong size; that bad raster
   is then cached in the atlas permanently. We sidestep it by rasterizing each
   unique glyph through a throwaway `SwashCache::new()` with a pristine
   `ScaleContext`. Cheap, because a glyph is rasterized exactly once and then
   served from the atlas forever (a few hundred calls total, never per frame).
   Root cause lives in `cosmic_text` / `swash`, not `bevy_text` itself — Bevy
   only holds one shared instance.

Neither has been reported upstream yet: both need a minimal reproduction and a
re-check against current Bevy `main` first. See the inline comments at each site
for details.

## Testing whether the patches are still needed (after a Bevy upgrade)

The corruption only shows up under text churn, so use the built-in soak test.
`STELLWERK_AUTOCYCLE` cycles the game through its states/menus automatically and
`STELLWERK_WINDOWED` gives you a visible window to watch:

```sh
STELLWERK_AUTOCYCLE=1 STELLWERK_WINDOWED=1 cargo run
```

Let it run a few minutes and watch for wrong-size / garbled glyphs in the UI.

To test a **new upstream bevy_text unmodified**, drop the vendored crate in the
workspace `Cargo.toml`:

```toml
# [patch.crates-io]
# bevy_text = { path = "vendor/bevy_text" }
```

Then bump `bevy` to the new version and run the soak test above. Isolate each
patch by reverting one at a time inside `src/font_atlas.rs`:

- **Patch 1:** change `.max(2048)` back to `.max(512)`.
- **Patch 2:** rasterize through the shared cache again —
  `swash_cache.get_image_uncached(font_system, physical_glyph.cache_key)`
  instead of the throwaway `isolated` cache.

If the soak test stays clean with a patch reverted, that patch is obsolete on
the new version — delete it.

## Real crate bug vs. our usage (how to classify before reporting)

Decision rule once you can reproduce corruption:

- **Reproduces in a clean, minimal Bevy example** (default settings, no unusual
  app code) → real crate bug → worth an upstream issue.
- **Only reproduces with our app's usage pattern** (many distinct font sizes /
  fractional scale factors multiplying subpixel-bin glyph variants, or text
  rebuilt every frame) → self-inflicted; upstream will (correctly) decline it.

Current best guess, unconfirmed until reproduced:

- **Patch 2 (ScaleContext drift) — likely a real crate bug.** A shared
  `SwashCache` should never rasterize the same cache key at the wrong size; that
  is internal state leaking across calls. Root cause is in `cosmic_text` /
  `swash`, not `bevy_text`. Report to `pop-os/cosmic-text` if it reproduces.
- **Patch 1 (atlas overflow) — probably partly our own doing.** A second atlas
  page should only appear if we generate enough distinct glyph variants to
  overflow 512², which points at our font-size / subpixel usage. The *sampling*
  of a second page returning the wrong texture, however, would be a genuine Bevy
  bug — so if a minimal example that deliberately overflows to a 2nd page
  corrupts, that part is worth reporting to `bevyengine/bevy`.
