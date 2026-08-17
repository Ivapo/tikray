# Tikray

View and convert images without leaving the terminal. One binary for two jobs
that today need two tools: looking at an image (`chafa`, `timg`) and turning it
into a different format (ImageMagick).

**Tikray** is Quechua for *to turn over / to translate* — it already spans both
meanings.

> **Status: Phase 2.** `tikray view` draws PNG, JPEG and SVG inline in iTerm2.
> `tikray convert` and the TUI shell are specced but not built — see
> [`specs/INDEX.md`](specs/INDEX.md).

## Install

```sh
cargo install --path .
```

## Usage

```sh
tikray view <path>            # draw a PNG, JPEG or SVG inline
tikray view --force <path>    # emit the escape sequence anyway (see below)
```

`view` is required, and deliberately so. A bare `tikray <path>` is reserved for
the TUI browser, so that `view` keeps meaning the one thing worth naming:
*print it and give me my prompt back, don't take my screen.*

### What it supports today

| | |
|---|---|
| **Input formats** | PNG, JPEG, SVG |
| **Terminal** | iTerm2 only |

Input format is detected from the file's **contents**, not its extension: a PNG
named `.txt` displays fine, and a text file named `.png` is refused as an
undetermined format rather than as a corrupt image.

Anything else `image` can decode — GIF, BMP, TIFF, WebP — is refused by name.
That is deliberate: support is an explicit list, not whatever the dependencies
happen to link in.

**SVG** is rasterized at its own natural size and then travels the same path as
any other image, so everything below applies to it unchanged. Two limits worth
knowing: the SVG must be **self-contained** — one that pulls in a raster by
relative path silently drops that element — and gzipped `.svgz` is not supported,
so it is refused as an undetermined format.

### Sizing

An image larger than the window is scaled down to fit, preserving its aspect
ratio. An image that already fits is drawn at native size — **Tikray never
upscales**, so a 16×16 icon appears as a 16×16 icon rather than a screenful of
blurred squares. There is no flag for this; it is the rule.

The rule applies to SVG exactly as to raster, which is worth saying out loud: an
icon with `viewBox="0 0 24 24"` draws as a 24-pixel speck, not re-rendered sharp
at window size. That is consistency with the never-upscale rule rather than an
oversight, and it is the one place vector input buys you nothing today.

If the terminal does not report its size in pixels, Tikray falls back to the
image's inherent size, which may scroll.

### `--force`

Tikray refuses to emit unless stdout is a terminal *and* the environment looks
like iTerm2 (`TERM_PROGRAM=iTerm.app` or `LC_TERMINAL=iTerm2`). The first check
stops `tikray view x.png > out.txt` from filling a file with escape bytes; the
second stops another terminal from printing them at you.

Both checks have known false negatives — plain tmux breaks them, as can an
unrecognized iTerm2-compatible terminal — so `--force` skips both and emits
anyway.

## Development

This repo is developed spec-driven; see [`CLAUDE.md`](CLAUDE.md).

```sh
cargo test     # the exit gate for the phases built so far
```
