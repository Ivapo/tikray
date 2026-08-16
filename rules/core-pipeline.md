---
title: core-pipeline
sources:
  - src/lib.rs
  - src/load.rs
  - src/error.rs
covers: >
  the DynamicImage waist, the content-sniffing load path, the per-phase input
  allowlist, and the error taxonomy the CLI renders to stderr
max_lines: 50
generated: 2026-08-16
---

# Core pipeline

Every capability is the same pipeline with a different edge enabled:
**decode-or-rasterize → one in-memory buffer → display-or-encode.** As of Phase 1
only the raster-decode edge and the display edge exist.

## The waist is `DynamicImage`

`src/load.rs:load` returns `image::DynamicImage`, not a fixed RGBA8 raster. It
keeps the source's channel layout and bit depth, where `to_rgba8()` would
quantize a 16-bit PNG to 8 on load — invisible for display, material for the
convert edge, which reads a written file back and compares it. Individual edges
convert to concrete buffers where they need to; the waist does not.

## Format comes from the bytes, never the extension

`src/load.rs:load` builds the reader from an already-open file:

```rust
ImageReader::new(BufReader::new(File::open(path)?)).with_guessed_format()?
```

The construction is load-bearing. `ImageReader::open` seeds the format from the
path extension, and `with_guessed_format` is `format.or(self.format)` — so under
that construction a failed guess *keeps the extension's answer*, and a text file
named `.png` is reported as a corrupt PNG. Building from an open file seeds
nothing, so a failed guess has nothing to fall back to.

The externally visible consequence, and the reason it is worth the extra line: a
PNG named `.txt` loads, and a text file named `.png` is `FormatUndetermined`
rather than `Decode`. `tests/gate.rs` asserts both, and mutating `load` to the
seeded construction fails exactly that one assertion.

## Support is an allowlist, not whatever links in

`src/load.rs:ALLOWED` is `[Png, Jpeg]`. With default features `image` also
decodes GIF, BMP, TIFF, ICO, QOI and WebP, so a decodable-but-unallowed input is
refused by name (`src/error.rs:format_name` renders the detected format) rather
than silently succeeding on a format no phase has gated. Sniffing is
feature-independent — `image`'s `MAGIC_BYTES` table carries no `cfg` — so the
detected name is right even for formats the build cannot decode.

## Errors are text, one variant per failure

`src/error.rs:TikrayError` implements `Display` and `std::error::Error`;
`src/main.rs` prints it to stderr and exits non-zero. Nothing panics or prints a
`Debug` dump at the user.
