---
title: svg-rasterization
sources:
  - src/svg.rs
covers: >
  usvg's resolved size as the rasterization target, the premultiplied-alpha
  boundary out of tiny-skia, the system font database, and the two SVG-specific
  error variants
max_lines: 50
generated: 2026-08-16
---

# SVG rasterization

`src/svg.rs:rasterize` turns SVG bytes into the same `DynamicImage` every other
input fills. Nothing downstream of it knows the input was vector — that is §2.1's
claim being cashed rather than asserted.

It is a deterministic function of `bytes`; the `path` it also takes is read only
to label errors, following `src/error.rs:TikrayError::io`'s precedent.

## The target size is the SVG's own, never the viewport's

`usvg` resolves a size for *every* parseable input: `width`/`height` when
present, the `viewBox`'s dimensions when only a viewBox, the content bounding box
when neither, and `Options::default_size` (100×100) for an empty root. So an SVG
arrives at `src/display.rs:fit` with a native size exactly like a raster, and the
sizing arithmetic needs no vector special case.

Deriving the raster size from the viewport would buy nothing — `fit`'s scale
never exceeds `1.0`, so the fit size is never larger than native — and would cost
`src/load.rs:load` its signature, since the viewport is not discovered until two
steps later. `tiny_skia::Size::to_int_size()` **rounds**: `width="10mm"
height="5mm"` resolves to 37.795 × 18.898 and becomes 38 × 19.

## Two silent failures, both guarded

`tiny_skia::Pixmap` is **premultiplied**. `RgbaImage::from_raw(w, h,
pixmap.take())` carries 50%-opacity red through as `[128, 0, 0, 128]`,
compositing over white as (191,127,127) instead of (255,127,127) — so
`take_demultiplied()` ships, which is a `PremultipliedColorU8::demultiply()` pass
over every pixel. `tests/gate_phase2.rs` asserts the pixel value, because
dimensions cannot catch this; swapping the call back fails that one assertion.

`usvg::Options::default()` has an **empty font database**, and `<text>` then
renders zero non-transparent pixels with no error. `load_system_fonts()` is
called for that reason, at ≈800 faces of startup cost.

## What is not supported, by decision

`resources_dir` stays `None`, so an SVG referencing a raster by relative `href`
silently drops that element: a self-contained SVG is what is supported. Errors
are `src/error.rs:TikrayError::SvgParse` (usvg refused bytes that detected as
SVG — including a false positive such as HTML carrying an inline `<svg`) and
`Rasterize` (the buffer could not be allocated or did not match its size).
