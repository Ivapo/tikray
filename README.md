# Tikray

View and convert images without leaving the terminal. One binary for two jobs
that today need two tools: looking at an image (`chafa`, `timg`) and turning it
into a different format (ImageMagick).

**Tikray** is Quechua for *to turn over / to translate* — it already spans both
meanings.

> **Status: Phase 3.** `tikray view` draws PNG, JPEG and SVG inline in iTerm2,
> and `tikray convert` writes any of them back out as PNG or JPEG. The TUI shell
> is specced but not built — see [`specs/INDEX.md`](specs/INDEX.md).

## Install

Needs a Rust toolchain, and **iTerm2** to see anything — the display half is
iTerm2-only by design (see [Non-goals](#non-goals)). There is no C toolchain
step: `resvg` rasterizes SVG in pure Rust, so this builds to a single binary.

```sh
cargo install --path .
```

## Usage

```sh
tikray view <path>            # draw a PNG, JPEG or SVG inline
tikray view --force <path>    # emit the escape sequence anyway (see below)

tikray convert <in> <out>              # format comes from <out>'s extension
tikray convert --format jpeg <in> <out>   # ...or from --format, which wins
tikray convert --overwrite <in> <out>  # replace <out> if it already exists
```

`view` is required, and deliberately so. A bare `tikray <path>` is reserved for
the TUI browser, so that `view` keeps meaning the one thing worth naming:
*print it and give me my prompt back, don't take my screen.*

There is something to point it at in [`samples/`](samples/) — a vector scene, the
same scene converted to PNG and JPEG, a 24×24 icon and a transparent one. Each
shows a different rule rather than being decoration; [`samples/README.md`](samples/README.md)
says which.

```sh
tikray view samples/landscape.svg
```

### What it supports today

| | |
|---|---|
| **Input formats** | PNG, JPEG, SVG |
| **Output formats** | PNG, JPEG |
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

### Converting

`tikray convert <in> <out>` writes any supported input back out as PNG or JPEG.
It needs no terminal — it writes a file, not escape bytes — so it works fine over
ssh, in a script, or piped.

The output format comes from `<out>`'s extension, or from `--format` where you
give one, which wins. Both are read case-insensitively. **An existing file is
never replaced without `--overwrite`**, and the refusal happens before anything
is read or written, so a run that cannot succeed leaves the destination exactly
as it was.

**JPEG has no alpha channel**, so a transparent PNG or SVG is composited onto
**white** before it is written — the same thing a browser or image viewer shows
you — and the run says so on stderr. This is worth knowing because the obvious
alternative is not "it refuses": the underlying library silently drops alpha
instead, which puts a transparent image on a *black* background with no error at
all. There are no quality or compression flags; the library defaults stand.

**Writing SVG is refused**, by name. Tikray converts through a pixel buffer, so a
raster in would have to be *traced* — a different and much larger tool — and an
SVG in would come back out a raster wearing an `.svg` extension. SVG is an input
format here, not an output one.

## Non-goals

Named here because they are refusals, not gaps waiting to be filled:

- **Other terminals.** No Kitty graphics protocol, no Sixel, no ASCII/half-block
  fallback. Supporting a second protocol means capability detection, per-protocol
  sizing and degraded fidelity — a much larger design problem.
- **Editing.** No crop, resize, rotate, filter or composite. Scaling exists only
  to fit the window, and is not a user-facing operation.
- **Animation.** A multi-frame input is treated as its first frame.
- **Batch pipelines.** No globbing or recursion. One input, one output.

## Development

This repo is developed spec-driven: `specs/` records *why* a decision was made
and what the plan is, `rules/` records *what is true right now*. Start at
[`specs/INDEX.md`](specs/INDEX.md) and [`CLAUDE.md`](CLAUDE.md).

```sh
cargo test     # the exit gate for the phases built so far
```

Each phase carries an exit gate someone else could check, and no phase is built
until its own review round converges — the record of those rounds is in
[`specs/reviews/`](specs/reviews/).

Most of a gate is `cargo test`, but the part that matters most is not: *does a
person looking at the screen see the right picture?* That half cannot be
asserted, so it is a script instead of a test.

```sh
bash scripts/gate8.sh    # Phase 3's item 8 — needs iTerm2, asks you questions
```

## License

MIT — see [LICENSE](LICENSE).
