---
title: convert
sources:
  - src/convert.rs
  - src/main.rs
covers: >
  the output allowlist as a two-variant type, how the target format is resolved
  and why not through the image crate, the composite-onto-white arithmetic and
  the two channels that announce it, and the order run takes its two cheap
  refusals in
max_lines: 75
generated: 2026-08-17
---

# Convert

`src/convert.rs:encode` turns the buffer every input fills into the bytes of a
file. Nothing in it knows which edge filled that buffer, so every input
`src/load.rs:load` accepts is convertible on arrival rather than one at a time.

It is the **pure seam** — buffer plus target in, bytes out, no filesystem — which
is what lets `tests/gate_phase3.rs` assert on encoded bytes without writing one,
and why the stderr note below is emitted by `src/main.rs:run` instead.

## The output allowlist is a type

`src/convert.rs:Output` is `Png | Jpeg`. A two-variant enum cannot name a format
no phase has gated, so the per-phase allowlist obligation is discharged
structurally rather than in prose. It earns the type because the encode edge is
*wider* than the decode edge: an RGBA8 buffer encodes cleanly to nine formats
(PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, QOI, TGA), so "whatever `image` will
write" is not two.

`src/convert.rs:resolve` reads `--format` when given, else the destination's
extension, both lowercased. **Not `ImageFormat::from_path`**, whose refusal for
`.svg` reads "the file extension `svg` was not recognized as an image format" —
false, since tikray reads SVG. Mutating `resolve` to it lands `out.svg` on
`OutputUndetermined` and fails two assertions. **SVG is input-only**, and
`src/error.rs:TikrayError::OutputSvg` carries both readings in one message
because the destination is `.svg` either way and only the source differs: a
raster in would have to be traced, and an SVG in would come back out a raster
wearing an `.svg` extension.

## Alpha is composited onto white, and it says so

`DynamicImage::write_to(_, Jpeg)` on an RGBA8 buffer **succeeds** by calling
`to_rgb8()`, dropping alpha with no compositing at all: a transparent pixel lands
on **black** (`[0,0,0,0]` → `[0,1,0]`), exit zero, right dimensions, right
format, no error. So `src/convert.rs:flatten` composites first, in integers —
`src/convert.rs:over_white` is `(c·a + 255·(255−a) + 127) / 255` per channel —
and `src/main.rs:run` prints one line to stderr naming it, exit zero. The note
fires on the buffer *having* an alpha channel, not on any pixel actually being
transparent: coarser, and not subtly gettable-wrong. Quality and compression stay
at the library defaults, with no flags.

The PNG branch hands the buffer over untouched, for the reason
`rules/core-pipeline.md` gives the waist.

## Refusals come before the work

`src/main.rs:run` orders the convert arm **resolve, overwrite guard, load,
encode, write**, so a run that cannot possibly succeed does no work and touches
no file. Its one visible consequence is deliberate: `convert missing.png
existing.png` reports `OutputExists`, not `Io`. There is no tty check on this
path — convert writes a file, not escape bytes.

`run` dispatches four invocations across three surfaces now, and this arm is the
one that writes a file. **The stderr note belongs to it alone.** The browser
converts too — `src/tui.rs:convert_to` is `encode`'s second caller — and it
cannot print: inside the alternate screen an `eprintln!` paints over the
display. It returns the same fact as a flag instead, and the pane says it.

Two channels, **one rule**: both fire on the buffer *having* an alpha channel,
never on a pixel being transparent. `src/convert.rs:flatten` is called by
`encode` and by nothing else — a pre-flattened buffer reports `has_alpha() ==
false`, so calling both would be redundant rather than wrong, and the TUI reports
what `encode` did rather than doing it twice.
