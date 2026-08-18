# Tikray

View and convert images without leaving the terminal. One binary for two jobs
that today need two tools: looking at an image (`chafa`, `timg`) and turning it
into a different format (ImageMagick).

**Tikray** is Quechua for *to turn over / to translate* — it already spans both
meanings.

> **Status: complete.** All ten phases of `tkr-001` are built.
> `tikray <file>` draws a PNG, JPEG or SVG inline in iTerm2, `tikray convert`
> writes any of them back out as PNG or JPEG, and a bare `tikray` opens a file
> browser that previews what you land on and converts from there — see
> [`specs/INDEX.md`](specs/INDEX.md).

## Install

Needs a Rust toolchain, and **iTerm2** to see anything — the display half is
iTerm2-only by design (see [Non-goals](#non-goals)). There is no C toolchain
step: `resvg` rasterizes SVG in pure Rust, so this builds to a single binary.

```sh
cargo install --path .
```

## Usage

```sh
tikray <file>                 # draw it inline, and give the prompt back
tikray                        # browse from here, previewing what you land on
tikray <dir>                  # browse from there
tikray --browse <file>        # browse that file's directory, it highlighted

tikray view <path>            # the same inline draw, spelled out
tikray view --force <path>    # emit the escape sequence anyway (see below)

tikray convert <in> <out>              # format comes from <out>'s extension
tikray convert --format jpeg <in> <out>   # ...or from --format, which wins
tikray convert --overwrite <in> <out>  # replace <out> if it already exists
```

**A bare path does what the path is:** a file is drawn, a directory is browsed.
That is the common case — *show me this image* — reachable without remembering a
verb.

`view` is not a redundant synonym, it is the form that takes options: `--force`
lives there and nowhere else. `--browse` goes the other way, forcing the browser
on a file, which is the one thing a bare path cannot say for itself.

A first argument is a subcommand only when it matches one exactly, so a file
actually named `view` is `tikray ./view`.

There is something to point it at in [`samples/`](samples/) — a vector scene, the
same scene converted to PNG and JPEG, a 24×24 icon and a transparent one. Each
shows a different rule rather than being decoration; [`samples/README.md`](samples/README.md)
says which.

```sh
tikray samples/landscape.svg
```

### What it supports today

| | |
|---|---|
| **Input formats** | PNG, JPEG, SVG |
| **Output formats** | PNG, JPEG |
| **Surfaces** | inline one-shot (`view`), a file (`convert`), a browser |
| **Terminal** | iTerm2 only to *see* anything; `convert` needs no terminal |

Input format is detected from the file's **contents**, not its extension: a PNG
named `.txt` displays fine, and a text file named `.png` is refused as an
undetermined format rather than as a corrupt image.

Anything else that *looks* like an image — GIF, BMP, TIFF, WebP — is refused by
name. That is deliberate: support is an explicit list, not whatever the
dependencies happen to link in. The build links only PNG and JPEG decoders, and
those formats are still named in the refusal because recognising a file and
decoding it are different things.

**SVG** is rasterized at its own natural size and then travels the same path as
any other image, so everything below applies to it unchanged. Two limits worth
knowing: the SVG must be **self-contained** — one that pulls in a raster by
relative path silently drops that element — and gzipped `.svgz` is not supported,
so it is refused as an undetermined format.

### Sizing

An image larger than the window is scaled down to fit, preserving its aspect
ratio. An image that already fits is drawn at native size — **Tikray never
upscales on its own**, so a 16×16 icon appears as a 16×16 icon rather than a
screenful of blurred squares. There is no flag for this; it is the rule
everywhere except one place, and only when you ask: `+` in the browser. Nothing
upscaled is ever written to a file.

The rule applies to SVG exactly as to raster, which is worth saying out loud: an
icon with `viewBox="0 0 24 24"` draws as a 24-pixel speck. In the browser you can
press `+` to make it bigger — but it is upscaled as pixels, not re-rendered, so
it goes blocky rather than sharp. That is still the one place vector input buys
you nothing today.

If the terminal does not report its size in pixels, Tikray falls back to the
image's inherent size, which may scroll.

An inline draw is indented two columns from the left edge, and **the indent comes
out of the available width** rather than being added to it — so an image as wide
as your window still fits on one screen. Redirect the output and you get the
escape sequence with no indent at all: `--force` exists so the sequence can be
captured, and leading spaces would corrupt it.

### `--force`

Tikray refuses to emit unless stdout is a terminal *and* the environment looks
like iTerm2 (`TERM_PROGRAM=iTerm.app` or `LC_TERMINAL=iTerm2`). The first check
stops `tikray view x.png > out.txt` from filling a file with escape bytes; the
second stops another terminal from printing them at you.

Both checks have known false negatives — plain tmux breaks them, as can an
unrecognized iTerm2-compatible terminal — so `--force` skips both and emits
anyway.

### Browsing

`tikray` with no arguments — or with a directory, or with `--browse` — opens a
file list with a preview pane beside it. `↑`/`↓` (or `j`/`k`) move, `⏎` (or `→`)
enters a directory, `←` goes back up, and `q` quits. (The convert keys below are
capitals because `j` is already *down*.) The highlighted image is
drawn **centred** in the pane, scaled to it by the same rule below.

**`+` and `-` zoom** the preview through three levels — fit, 2×, 4× — and `0`
returns to fit. Zooming crops to the centre of the image, because the terminal
cannot clip one for us; there is no panning, so at 4× you are looking at the
middle sixteenth. Moving to another file returns to fit.

**`P` writes a PNG and `J` a JPEG** beside the highlighted file, same name, new
extension — `photo.svg` becomes `photo.png`. The pane says what was written, and
says when alpha had to be flattened onto white, since a JPEG cannot carry it.
Writing over an existing file refuses and asks: press the same key again to
replace it. Converting a file onto itself is refused outright — it would
re-encode it over itself and change nothing.

**The list shows what tikray can draw**, and hides what your shell hides — images and directories — and the footer
says how many entries are hidden. `.` shows everything and `.` again puts the
filter back; the setting follows you into subdirectories. Hidden entries — the
dot-prefixed ones — are held back by the same key, directories included. What
counts as an image is decided by **reading each file**, not by its extension, so
a PNG named `.txt` is listed as the PNG it is.

**The preview is not a widget** — it is an image drawn behind a hole in the
layout, which is why the pane is deliberately empty and why the rest of the
screen never overlaps it. Where there is nothing to draw, the pane says why in
one line rather than going blank: a directory, a file that is not an image, a
terminal that is not iTerm2, or one that does not report its size in pixels.
**In every one of those the browser still runs** — the file list is useful on
its own. The one refusal is stdout not being a terminal, since then there is no
screen to browse in at all.

Converting from inside the browser is not built yet.

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
bash scripts/gate8.sh          # Phase 3's item 8 — needs iTerm2, asks you questions
bash scripts/gate-phase4.sh    # the human items for Phases 4-8 and 10
bash scripts/gate-phase9.sh    # Phase 9's, and it needs no terminal
```

## License

MIT — see [LICENSE](LICENSE).
