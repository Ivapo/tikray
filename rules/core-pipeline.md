---
title: core-pipeline
sources:
  - src/lib.rs
  - src/load.rs
  - src/svg.rs
  - src/convert.rs
  - src/error.rs
  - src/cli.rs
  - src/tui.rs
covers: >
  the DynamicImage waist, the content-sniffing load path, the raster and vector
  branches behind it, the per-phase input allowlist, the two output edges and
  the format matrix, the two callers over one core, and the error taxonomy the
  CLI renders to stderr
max_lines: 100
generated: 2026-08-17
---

# Core pipeline

Every capability is the same pipeline with a different edge enabled:
**decode-or-rasterize → one in-memory buffer → display-or-encode.** As of Phase 3
all four edges exist: raster decode and SVG rasterization in, the terminal and a
file out.

## Two callers, one core

`src/lib.rs` is a library with the CLI as one caller, not a binary with helpers,
and the second caller exists. `src/main.rs:run` reaches the one-shot surfaces
through `src/cli.rs:Command` **or through a bare path that turned out to be a
file** — the surface depends on the path's type, so a `Command` no longer
distinguishes them — and reaches the browser through `src/tui.rs:run`. Both go to
`src/load.rs:load` and the same waist; the TUI differs only in the last step,
sizing to a pane rather than the window, and `rules/tui.md` has the table of
which invocation lands where. Nothing below the waist knows which caller it is
serving — the claim the split was made for, rather than one asserted afterwards.

## The waist is `DynamicImage`

`src/load.rs:load` returns `image::DynamicImage`, not a fixed RGBA8 raster. It
keeps the source's channel layout and bit depth, where `to_rgba8()` would
quantize a 16-bit PNG to 8 on load — invisible for display, material for the
convert edge. `tests/gate_phase3.rs` is where that claim is finally cashed: a
16-bit PNG converted to PNG reads back `Rgb16` with `[65535, 1234, 7]` intact,
where a `to_rgba8()` first reads back `Rgba8` with `[65535, 1285, 0]` — and
**both report `Png` at the same dimensions**, so only colour type and pixel
separate them. Individual edges convert to concrete buffers where they need to;
the waist does not.

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

## Both output edges exist, and the matrix is not square

`src/display.rs:sequence` encodes the buffer for the terminal and
`src/convert.rs:encode` encodes it for a file. They share the waist and nothing
else, and what each supports is decided separately:

| | PNG | JPEG | SVG | GIF, BMP, TIFF, WebP, … |
|---|---|---|---|---|
| **in** | yes | yes | yes | refused, by name |
| **out** | yes | yes | **refused, by name** | refused, by name |

`src/convert.rs:Output` is the output allowlist as `src/load.rs:ALLOWED` is the
input one, and it is a two-variant enum rather than a runtime check so it cannot
name an ungated format at all. SVG is input-only — see `rules/convert.md`.

## Errors are text, one variant per failure

`src/error.rs:TikrayError` implements `Display` and `std::error::Error`;
`src/main.rs` prints it to stderr and exits non-zero. Nothing panics or prints a
`Debug` dump at the user.
