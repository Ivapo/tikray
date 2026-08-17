---
title: core-pipeline
sources:
  - src/lib.rs
  - src/load.rs
  - src/svg.rs
  - src/error.rs
covers: >
  the DynamicImage waist, the content-sniffing load path, the raster and vector
  branches behind it, the per-phase input allowlist, and the error taxonomy the
  CLI renders to stderr
max_lines: 70
generated: 2026-08-16
---

# Core pipeline

Every capability is the same pipeline with a different edge enabled:
**decode-or-rasterize → one in-memory buffer → display-or-encode.** As of Phase 2
both input edges exist and the display edge exists; nothing encodes to a file yet.

## The waist is `DynamicImage`

`src/load.rs:load` returns `image::DynamicImage`, not a fixed RGBA8 raster. It
keeps the source's channel layout and bit depth, where `to_rgba8()` would
quantize a 16-bit PNG to 8 on load — invisible for display, material for the
convert edge, which reads a written file back and compares it. Individual edges
convert to concrete buffers where they need to; the waist does not.

## Format comes from the bytes, never the extension

`src/load.rs:load` reads the file whole (`usvg` needs the bytes anyway) and
builds the reader over a cursor:

```rust
ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?
```

The construction is load-bearing. `ImageReader::open` seeds the format from the
path extension, and `with_guessed_format` is `format.or(self.format)` — so under
that construction a failed guess *keeps the extension's answer*, and a text file
named `.png` is reported as a corrupt PNG. Seeding nothing leaves a failed guess
nothing to fall back to.

The externally visible consequence, and the reason it is worth the extra line: a
PNG named `.txt` loads, and a text file named `.png` is `FormatUndetermined`
rather than `Decode`. `tests/gate.rs` asserts both, and mutating `load` to the
seeded construction fails exactly that one assertion.

## The vector branch is recognised, never fallen through to

`src/load.rs:Input` is `Raster(ImageFormat) | Svg`, because `image` has no SVG
variant and its signature table returns `None` for plain, XML-prolog,
BOM-prefixed and gzipped SVG alike. `src/load.rs:detect` is three positive steps:
the signature table, then `src/load.rs:looks_like_svg` — after a UTF-8 BOM and
leading ASCII whitespace the first byte must be `<` **and** a case-insensitive
`<svg` must appear within the first 1024 bytes — then `None`.

Positive is the whole point. SVG bytes produce the same `None` a text file named
`.png` produces, so a fall-through would refuse that file as an SVG parse error
and break a shipped assertion. Mutating the rule to `true` fails exactly
`tests/gate.rs`'s `gate3_a_text_file_named_png_is_undetermined_not_corrupt`.
`usvg` stays the final arbiter, so a false positive is a legible parse error and
never a wrong render. `.svgz` opens `1f 8b`, fails the rule, and is not supported.
`Input::Svg` dispatches to `src/svg.rs:rasterize` — see `rules/svg-rasterization.md`.

## Support is an allowlist, not whatever links in

`src/load.rs:ALLOWED` is `[Raster(Png), Raster(Jpeg), Svg]` — a check over
`Input`, not over `[ImageFormat; 2]`, which is what SVG joining the list costs.
With default features `image` also decodes GIF, BMP, TIFF, ICO, QOI and WebP, so
`src/load.rs:allowed` refuses a decodable-but-unallowed input by name
(`src/error.rs:format_name` renders the detected format) rather than silently
succeeding on a format no phase has gated. Sniffing is feature-independent —
`image`'s `MAGIC_BYTES` table carries no `cfg` — so the detected name is right
even for formats the build cannot decode.

## Errors are text, one variant per failure

`src/error.rs:TikrayError` implements `Display` and `std::error::Error`;
`src/main.rs` prints it to stderr and exits non-zero. Nothing panics or prints a
`Debug` dump at the user.
