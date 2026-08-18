---
id: tkr-001
title: tikray
status: accepted
last_updated: 2026-08-17
note: >
  A single Rust binary that shows an image inline in iTerm2 and writes it back
  out in another format — the decode/rasterize/encode core, the OSC 1337
  display path, the convert command, and the TUI shell over the same core.

phases:
  - name: "Phase 1 — view a raster image inline in iTerm2"
    reviewed: 2026-08-16
    shipped: 2026-08-16
    cut: null
    by: null
  - name: "Phase 2 — view an SVG, by rasterizing into the same buffer"
    reviewed: 2026-08-16
    shipped: 2026-08-16
    cut: null
    by: null
  - name: "Phase 3 — convert: write the buffer back out in another format"
    reviewed: 2026-08-16
    shipped: 2026-08-16
    cut: null
    by: null
  - name: "Phase 4 — the TUI shell over the same core"
    reviewed: 2026-08-17
    shipped: 2026-08-17
    cut: null
    by: null
  - name: "Phase 5 — convert from inside the TUI"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 6 — the bare path draws inline, and the preview sits in the middle"
    reviewed: null
    shipped: null
    cut: null
    by: null

extends: null
supersedes: null
superseded_by: null
related: []
reference: >
  chafa and timg (terminal image viewers) and ImageMagick's `convert` (format
  translation) are the two tools this replaces for one workflow. Out of scope
  from both: chafa's and timg's multi-protocol terminal support (Kitty, Sixel,
  ASCII-art fallback) and ImageMagick's operator set (crop, filter, composite,
  color-space work). Tikray takes the intersection, not the union.
---

# Tikray

## 1. Goal

**The observable is an image the user can see — drawn inline in an iTerm2 window,
or written out as a file in the requested format that opens and looks right.**

Tikray is one binary for two jobs that today need two tools: looking at an image
without leaving the terminal (`chafa`, `timg`) and turning it into a different
format (ImageMagick). It is for a developer working in iTerm2 who wants to see a
PNG, a JPEG or an SVG in place, and to save any of them as any of the others,
without a GUI and without remembering two command vocabularies.

Both halves of the observable are *seen*: the inline render is looked at
directly, and the converted file is judged by opening it. The intermediate the
whole program is built around — a decoded pixel buffer in memory — is
deliberately **not** the observable. Neither is the OSC 1337 escape sequence
that carries the picture to the terminal: that is the mechanism, and a phase
that emitted a well-formed escape sequence displaying nothing would have
produced no observable at all.

### 1.1 Non-goals

- **Other terminals.** No Kitty graphics protocol, no Sixel, no ASCII/half-block
  fallback. v1 targets iTerm2's inline image protocol only. Supporting a second
  protocol is a much larger design problem (capability detection, per-protocol
  sizing, degraded fidelity) and would swamp the first observable.
- **Editing.** No crop, resize-as-a-command, rotate, filter, or composite.
  Scaling exists only where it serves display fitting, and is not a user-facing
  operation in v1.
- **Animation.** No animated GIF or APNG playback; a multi-frame input is shown
  and converted as its first frame, and says so.
- **Batch pipelines.** No globbing, recursion, or parallel conversion of a
  directory in v1. One input, one output.
- **Being ImageMagick.** The comparison in `reference` is about which tool a
  user reaches for, not about operator coverage.

## 2. Design

### 2.1 One pixel buffer, three edges

Every capability in this spec is the same short pipeline with a different edge
enabled: **decode-or-rasterize → one in-memory image buffer → display-or-encode.**
Raster input decodes into it, SVG input rasterizes into it, the iTerm2 path
encodes it for display, and the convert path encodes it to a file. That is why
the SVG half is not a separate program: `resvg` produces a pixel buffer, and
from there SVG input is indistinguishable from PNG input.

**The buffer type is `image::DynamicImage`, not a fixed RGBA8 raster.**
`DynamicImage` keeps the source's channel layout and bit depth, where
`to_rgba8()` would quantize a 16-bit PNG to 8 on load. That is invisible for
display — the terminal gets 8-bit sRGB either way — and material for Phase 3,
whose gate reads the converted file back and compares it. Individual edges
convert to `RgbaImage` where they need to; the waist does not. Phase 1 is where
this type is fixed and written into `rules/core-pipeline.md`, which is why it is
settled here rather than at the phase that first cares.

The consequence worth stating up front, because it constrains every later phase:
**the buffer is the narrow waist, so anything that cannot be expressed as a
decoded image is not in this design.** Vector-to-vector passthrough (SVG in, SVG
out, unrasterized) is the case this excludes, and OQ-2 holds it open.

### 2.2 The library is the product; the CLI and TUI are two callers

The decode/rasterize/encode core is a library module, and both the CLI and the
TUI call it. This is not architecture for its own sake: Phase 4 adds a second
caller, and a core that only exists inside `main` would be rewritten at that
point rather than reused. The test surface follows the core, so the CLI stays
thin enough to be checked by running it.

### 2.3 iTerm2's inline image protocol, and why it needs no rendering logic

iTerm2 accepts `ESC ] 1337 ; File=<args> : <base64 payload> BEL`, where the
payload is a **complete encoded image file**, not raw pixels. So the display
path is: encode the buffer to PNG, base64 it, print the escape sequence. There
is no cell-quantization, dithering, or palette-approximation step anywhere in
Tikray — the terminal does the rendering. This is the single largest reason the
scope is affordable, and the single largest reason it is iTerm2-only.

**`inline=1` is mandatory and is the whole difference between the observable and
nothing.** The argument defaults to `0`, and iTerm2's documentation is explicit
about what that means: the file "will be downloaded with no visual
representation in the terminal session." A sequence that omits it is
well-formed, exits 0, and displays nothing — exactly the failure §1 names when
it refuses to make the escape sequence the observable. Phase 1 emits this
argument string, in this order:

```
inline=1;width=<W>px;height=<H>px;preserveAspectRatio=1
```

`width` and `height` accept `N` (character cells), `Npx`, `N%`, or `auto`;
§2.6 settles which form Tikray uses and how `<W>` and `<H>` are computed.
`preserveAspectRatio` already defaults to `1`, so stating it is belt-and-braces
rather than load-bearing — §2.6's arithmetic is what actually preserves the
ratio, and the exit gate asserts on the computed dimensions rather than on this
argument.

**Re-encoding to PNG is a choice, not a constraint.** iTerm2 accepts any image
format macOS can read, so a JPEG could be base64'd through byte-for-byte and
skip a decode/encode round-trip. Tikray re-encodes anyway, because from Phase 2
onward the thing being displayed is frequently a rasterized SVG that has no
source file to pass through, and one display path that always starts from the
§2.1 buffer is worth more than a fast path for one input type. Stated so a later
pass can weigh it rather than rediscover it as an oversight.

### 2.4 Rust, `image`, `resvg`

Versions are floors, pinned here because "the `image` crate" is not a fact an
implementer can act on — 0.24 and 0.25 differ materially in the reader API this
spec depends on:

- **`image = "0.25"`** (0.25.10 current) decodes and encodes PNG and JPEG, and
  brings GIF, BMP, TIFF, ICO, QOI and WebP along at no additional design cost.
  **The free ride is not uniform, and §2.8 is where it stops being a free ride:**
  WebP decode is complete but *encode is lossless-only*, so a format being
  present in `image` is not the same as it being an output Tikray can offer.
  Which formats are *supported* versus *incidentally working* is settled per
  phase by an explicit allowlist (§2.8), not left to whatever the dependency
  happens to link in.
- **`resvg = "0.48"` / `usvg = "0.48"`** (0.48.1 current) rasterize SVG in pure
  Rust, so there is no C toolchain dependency and the binary stays a single
  artifact. This is the deciding reason over librsvg/Cairo, and Phase 2's
  round 1 confirmed it by compiling `resvg` 0.48.1, `usvg` 0.48.1 and
  `tiny-skia` 0.12.0 with no C toolchain step — the claim is measured, not
  assumed. `tiny-skia` arrives through `resvg` and is named because §2.11's
  premultiplied-alpha boundary is its API, not resvg's.
- **`base64 = "0.23"`** (0.23.1 current) for the payload, and
  **`clap = "4"`** (4.6.6 current, `derive` feature) for the CLI. `clap` is
  worth a dependency at Phase 1 rather than hand-rolled argument parsing
  because Phase 3 adds a second subcommand with flags and Phase 4 a bare
  invocation, all of which it already handles.
- **`crossterm = "0.29"`** from Phase 1, not Phase 4 — §2.6 needs
  `terminal::window_size()` for the viewport, which is a query, not a UI.
  **`ratatui`** is Phase 4 only. Nothing before Phase 4 depends on it, which is
  what lets Phase 4 be cut without unwinding Phases 1–3.

### 2.5 Sequencing: the smallest visible thing first

Phase 1 is `tikray view <png|jpeg>` and nothing else. It is the shortest path
from no code to the observable — one decode, one encode, one escape sequence —
and it retires the project's largest unknown (does the inline protocol actually
display what we emit, under this terminal, at this size) before any effort is
spent on SVG, conversion, or a TUI. Every later phase adds one edge to the
pipeline §2.1 describes.

### 2.6 Display sizing: fit down, never up *(decision, recorded — resolves OQ-3)*

The protocol will size an image for us — `width=100%;height=100%` fills the
session "as much as possible without stretching" — and that is the wrong
default, because *filling* is not *fitting*: a 16×16 favicon fills the window
too, and comes out as a screenful of blurred squares. So Tikray computes the
dimensions itself and emits them in `px`.

Given the decoded image's native size `(w, h)` and a viewport `(W, H)` in
pixels:

```
scale = min(W / w, H / h, 1.0)          # the 1.0 clamp is the never-upscale rule
out_w = max(1, round(w * scale))
out_h = max(1, round(h * scale))
```

An image that already fits is emitted at native size, because `scale` clamps to
`1.0`. An image larger in either axis is scaled down until both fit. The ratio
is preserved by construction, since one `scale` drives both axes.

**The viewport comes from `crossterm::terminal::window_size()`**, whose
`WindowSize` carries `width` and `height` in pixels alongside `rows` and
`columns`. Treating `0` as "unreported" is what crossterm's own documentation
prescribes rather than a hedge on our part: the pixel fields "may not be
reliably implemented or default to 0", unix documents them as *unused*, and on
Windows they are not implemented at all. So **`width == 0` or `height == 0` is
the unreported case**, and it falls back to `width=auto;height=auto` — the
image's inherent size, which may scroll. That fallback never upscales either, so
the rule above holds in both branches.

**Assume the fallback is a live path, not a corner.** Given how those fields are
documented, a plausible outcome is that this machine reports zeroes and the
`auto` branch is the one users actually get. Phase 1's gate therefore exercises
it deliberately, and gate item 5 requires the measurement to be *recorded* —
because if it fires here, Phase 2 inherits the problem (an SVG has no inherent
size to rasterize at) and that is worth knowing one phase early.

**CORRECTED 2026-08-16 — it did not fire.** Phase 1's gate item 5 measured
`window_size()` reporting real pixels in iTerm2 on this machine: a 64×48 source
emitted `width=64px;height=48px`. So the `px` branch is the primary path. (This
note originally added "and Phase 2 has a viewport to rasterize an SVG against";
§2.11 has since removed the need — Phase 2 rasterizes at the SVG's own size and
never consults the viewport.) The paragraph above is kept
because its *reasoning* still holds — the `auto` branch stays reachable and is
exercised by every run without a controlling terminal, including `cargo test`'s
— but a Phase 2 implementer should not plan around it as the common case.

**Also CORRECTED 2026-08-16: the parenthetical above — "an SVG has no inherent
size to rasterize at" — is simply false**, and Phase 2's round 1 measured it.
`usvg` resolves a size for *every* parseable input: `width`/`height` when
present, the `viewBox`'s dimensions when only a viewBox, the content bounding box
when neither, and `Options::default_size` (100×100) for an empty root. So an SVG
always arrives at §2.6 with a native size, exactly like a raster, and this
subsection's arithmetic needs no vector special case (§2.11).

The whole of this subsection is a pure function of four integers, which is what
makes Phase 1's gate machine-checkable at all (§4, Phase 1).

### 2.7 Detecting iTerm2 by environment, not by asking *(decision, recorded — resolves the round-1 blocker)*

Before emitting anything, Tikray requires **both**: stdout is a terminal
(`std::io::IsTerminal`), and one of `TERM_PROGRAM == "iTerm.app"` or
`LC_TERMINAL == "iTerm2"` is set. Failing either is a non-zero exit with a
message naming which check failed. The tty half is what stops
`tikray view x.png > out.txt` from filling a file with escape bytes; the env
half is what stops a non-iTerm2 terminal from printing them to a human.

Both variables are checked because they fail in different places: `TERM_PROGRAM`
is set by the local terminal, and `LC_TERMINAL` is the one that survives an ssh
hop. Neither survives plain tmux.

**Rejected: iTerm2's Feature Reporting protocol**, which is the mechanism
iTerm2's own documentation points at for detecting inline-image support. It is
strictly more correct — it asks the terminal rather than inferring from
environment — and it costs a tty put into raw mode, a control sequence written,
a reply parsed, and a timeout for every terminal that will never answer. That
is a tty state machine, which is its own plan-mode pass, and it fails in the
same tmux case the environment variables fail in. Recorded as rejected rather
than omitted, so a later phase can adopt it as an upgrade with the trade-off
already written down.

**`--force` is the escape hatch**: it skips both checks and emits anyway. It
exists because the checks have known false negatives (tmux, an unrecognized
iTerm2-compatible terminal), and a detection rule with no override turns a
false negative into an unusable tool.

**CORRECTED 2026-08-17 — `--force` does not rescue the tmux case, and naming it
among the false negatives implied it does.** The two halves of tmux are separate
failures and only one is ours: detection fails because neither variable survives
(that half `--force` genuinely fixes), and *tmux then swallows the escape
sequence itself* unless `allow-passthrough` is on — an option that has existed
since tmux 3.3 and is **off by default**. Checked on this machine: tmux 3.6a with
no `allow-passthrough` in `~/.tmux.conf`, so it is off. Under tmux, `--force`
therefore buys an emitted sequence and still no picture. Left unverified
end-to-end deliberately, because OQ-5's answer makes it low-value — but named,
because "the stopgap is `--force`" was a claim the escape hatch cannot honour.
Whoever picks tmux up owns **two** problems: detection, and passthrough (wrapping
the sequence in tmux's DCS form). Only the first is what §2.7 is about.

### 2.8 Input formats: detect by content, allowlist by phase *(decision, recorded)*

**Detection is content-based from Phase 1**, and *which* content-based
construction is load-bearing rather than incidental. Three candidates, two of
which are wrong:

| Construction | What it actually does |
|---|---|
| `image::open(p)` | Format from the path extension. Never sniffs. |
| `ImageReader::open(p)?.with_guessed_format()?` | **Also wrong.** `open` seeds the format from the extension, and `with_guessed_format` is `self.format = format.or(self.format)` — a failed guess *keeps the extension's answer*. |
| `ImageReader::new(BufReader::new(File::open(p)?)).with_guessed_format()?` | Format from the bytes alone. Nothing seeded, nothing to fall back to. |

Tikray uses the third. The second is the trap this table exists to mark: it
reads as content-detecting, is what one reaches for by name, and silently
degrades to extension-detection on exactly the inputs where the difference
matters — a text file named `.png` is reported as a corrupt PNG ("Invalid PNG
signature") rather than as not an image. Round 2 verified all three against
`image` 0.25.10 by compiling them; this is a measured behaviour, not a reading
of the documentation.

The reason to pay for content detection at all is Phase 2, which requires
dispatch "on detected input type rather than on file extension alone" behind the
*same* entry point — so the cheap choice is one Phase 2 would have to undo. What
it buys at Phase 1 is a legible error taxonomy: a PNG named `.txt` renders, and
a text file named `.png` is refused as **"the image format could not be
determined"** rather than as a corrupt PNG. Gate item 3 asserts both, and
asserts them because they are the two cases that distinguish the right
construction from the wrong one.

**Support is an explicit allowlist, per phase — Phase 1's is PNG and JPEG.**
With default features `image` also decodes GIF, BMP, TIFF, ICO, QOI and WebP
without being asked, so "unsupported format" is a thing Tikray must decide
rather than a thing that happens to it. A decodable-but-not-allowed input is
refused with a message naming the detected format, so the failure is legible
rather than a silent success on a format no phase has gated. This is §2.4's
"supported versus incidentally working" claim being made honestly, in the one
place that can enforce it.

### 2.9 CLI grammar: the TUI is the default surface, `view` is the opt-in one-shot *(decision, recorded)*

Four invocations. The split is by **surface**, not by verb:

| Invocation | Surface | Phase |
|---|---|---|
| `tikray view [--force] <path>` | inline, one-shot, returns to the prompt | 1 |
| `tikray convert [--format <fmt>] [--overwrite] <in> <out>` | no display; writes a file | 3 |
| `tikray` | TUI browser | 4 |
| `tikray <path>` | TUI browser, starting there | 4 |

**A bare path opens the TUI rather than drawing inline.** The alternative —
`tikray <path>` as a synonym for `tikray view <path>` — was considered and
rejected on two counts. It would make *adding an argument flip the entire output
mode*, where `tikray` and `tikray <path>` both reaching the browser is the
`vim` / `vim file` shape a path argument already implies. And it would make
`view` pure noise, a synonym existing only for symmetry with `convert`; under
this grammar `view` carries the one distinction that matters — *print it and give
me my prompt back, don't take my screen* — which is the smaller vocabulary §1
asks for, not the larger one.

**`tikray <path>` therefore does not exist before Phase 4**, and Phase 1
requiring the `view` subcommand is not a stopgap. Shipping a bare-path form
earlier would mean it meant "inline" then and "TUI" later: shipped behaviour
contradicted by a later phase, which is the one shape the methodology says is
never a phase.

**Convert stays a subcommand, not a `--convert` flag.** It takes two positionals
plus a format override and an overwrite guard (Phase 3's gate names the latter).
As a flag, `--convert` would silently govern how many positionals are legal and
what each one means, which the parser cannot validate and which degrades exactly
the error messages a user needs. §2.4 bought `clap` for subcommands; this is
that purchase being used.

**This decision depends on OQ-1.** If the spike shows OSC 1337 and a Ratatui
alternate screen cannot share a terminal, Phase 4 changes shape or is cut — and
`tikray <path>` goes with it, the fallback being that it inherits `view`'s inline
behaviour after all. Recorded here so that outcome is a known branch rather than
a rediscovery.

Whether `tikray <file>` opens the browser at that file's directory with it
highlighted, or a single-image view, is **reserved for Phase 4** — named so it is
not forgotten, not designed before there is a consumer.

**CORRECTED 2026-08-17 — the table's fourth row and the paragraph rejecting it
are both reversed by Phase 6, and the reversal came from the human after using
what this section designed.** `tikray <path>` now dispatches on the path's
*type*: **a file draws inline, a directory browses**. The argument above is kept
because it was honestly made and half of it still holds — the *reasoning* is what
this document is for — but it is no longer what the tool does, and a reader who
stops here will be wrong about the grammar.

Which half held, and which did not, is the part worth carrying forward. **The
`vim` / `vim file` analogy was the weak step.** `vim` has no second surface to
dispatch to: every invocation is the editor, so the analogy establishes that a
path argument is *natural*, not that it must select the same surface. Tikray does
have two surfaces, and the human's report is that the common case is "show me
this image" — for which `view` was the ceremony, not the affordance. **The
"`view` becomes pure noise" objection was the strong step, and Phase 6 answers it
rather than dismissing it**: `view` keeps the flags. `--force` lives there and
nowhere else, so the subcommand is the form that takes options and the bare form
is the zero-ceremony one — a real division of labour rather than a synonym.

What the reversal would have cost is spelled `--browse` instead, because §1
judges this project by what a user sees and the affordance Phase 4 shipped —
*browse, starting at this file* — is one a person had already used. Phase 6
keeps it rather than recording it as a loss.

### 2.10 SVG is not an `ImageFormat`, so detection grows a type *(decision, recorded — resolves Phase 2's round-1 blocker)*

§2.8 settled detection for raster input and cannot be stretched to cover SVG.
Round 1 measured why, and the measurement is the whole argument:

- `ImageReader::new(…).with_guessed_format()` returns `format() == None` for
  plain SVG, XML-prolog SVG, BOM-prefixed SVG and gzipped `.svgz` alike;
- `image::ImageFormat` has **no SVG variant at all**, so neither
  `src/load.rs:ALLOWED` (`[ImageFormat; 2]`) nor
  `TikrayError::FormatNotAllowed { format: ImageFormat }` can express one.

**The trap this creates is sharper than the gap.** SVG bytes today produce
`FormatUndetermined` — *the identical outcome Phase 1's shipped gate item 3
asserts for a text file named `.png`*. So the obvious implementation ("if
`image` cannot sniff it, hand the bytes to `usvg`") turns that shipped assertion
into an SVG parse error, and Phase 1's gate goes red for a reason that looks like
a Phase 2 bug. The dispatch must therefore be positive — SVG is *recognised*, not
*fallen through to*.

So Phase 2 introduces the input type the design has been missing:

```rust
pub enum Input { Raster(ImageFormat), Svg }
pub fn detect(head: &[u8]) -> Option<Input>
```

**Detection order is raster first, then SVG, then nothing**, and each step is
positive:

1. `image`'s signature table (§2.8's third construction, over the same bytes —
   nothing is seeded from the path, so §2.8's property is preserved verbatim).
2. Otherwise **the SVG rule**: after skipping a UTF-8 BOM and leading ASCII
   whitespace, the first byte must be `<`, **and** the case-insensitive
   substring `<svg` must occur within the first 1024 bytes. Both conditions,
   because either alone is too loose.
3. Otherwise `None` → `FormatUndetermined`, unchanged.

The rule is stated in bytes rather than described, because it is the thing gate
item 1 asserts and the thing that must keep `not_an_image.png` — which begins
`this is not an image` — returning `None`. `usvg` remains the final arbiter: a
false positive (an HTML file carrying an inline `<svg`) becomes a legible parse
error, never a wrong render.

**Phase 2's allowlist is PNG, JPEG and SVG**, discharging §2.8's per-phase
obligation. `ALLOWED` becomes a check over `Input` rather than over
`[ImageFormat; 2]`.

**Non-goals, named so they are refusals rather than surprises:** gzipped `.svgz`
(sniffs as `None`, so it lands on `FormatUndetermined` — a legible refusal
message for it is a candidate for a later phase, not this one) and UTF-16-encoded
SVG. Neither is claimed and neither is gated.

### 2.11 Rasterize at the SVG's own size; §2.6 then applies unchanged *(decision, recorded — resolves Phase 2's round-1 blockers)*

The seed design said Phase 2's "rasterization target size derives from the
display sizing decision". **That is deleted, because under §2.6 it buys nothing
and costs the architecture.** Round 1 established both halves:

- **It buys nothing.** `scale = min(W/w, H/h, 1.0)` never exceeds `1.0`, so the
  fit size is never *larger* than native — and rasterizing at a size you are
  never allowed to exceed cannot be sharper than rasterizing at native. Measured:
  a `viewBox="0 0 24 24"` icon resolves to 24×24, `fit((24,24), (1600,1200))`
  returns `(24,24)`, and it draws as 24 device pixels either way. The seed's
  claim that this is "the one place where output quality actually depends on
  getting sizing right first" is false under the rule §2.6 records.
- **It costs the architecture.** `src/load.rs:load` takes no viewport, and the
  viewport is not discovered until `src/display.rs:display` calls
  `src/term.rs:viewport` — two steps later in `src/main.rs:run`. Deriving the
  raster size from the viewport forces either a signature change or a `load`
  that reads the environment, making the pipeline's first stage
  environment-dependent.

**So: rasterize at `usvg`'s resolved size, and `load`'s signature does not
change.** From that point SVG is indistinguishable from PNG, which is §2.1's
claim actually being cashed rather than asserted. Three mechanical consequences,
each measured at round 1 and each gated:

- **Size comes from `usvg::Size::to_int_size()`**, which rounds — `width="10mm"
  height="5mm"` resolves to `37.795 × 18.898` and becomes `38 × 19`. Naming the
  rounding mode matters because it changes the emitted `px` arguments.
- **`tiny_skia::Pixmap` is premultiplied, and the obvious conversion corrupts
  alpha.** `RgbaImage::from_raw(w, h, pixmap.take())` — the one-liner an
  implementer reaches for — carries 50%-opacity red through as `[128, 0, 0, 128]`,
  compositing over white as `(191,127,127)` instead of `(255,127,127)`. Every
  pixel must go through `PremultipliedColorU8::demultiply()`. This is the same
  shape as the round-2 finding on §2.8: silent, plausible, and invisible to a
  gate that only checks dimensions — so gate item 3 asserts the pixel value.
- **`usvg::Options::default()` has an empty font database**, so `<text>` renders
  **zero** non-transparent pixels with no error — exactly the blank image the
  gate forbids. Tikray calls `load_system_fonts()`. The startup cost (≈800 faces)
  is accepted for v1 over correctness; scanning the source bytes for `<text`
  first would avoid it in the common case and is recorded here as available, not
  taken.

**Non-goal, named rather than discovered:** `usvg::Options::resources_dir`
defaults to `None`, so an SVG referencing a raster by relative `href` silently
drops that element. Phase 2 does not set it and does not claim otherwise; a
self-contained SVG is what is supported. Setting it from the input file's parent
is a one-line change available to any later phase that wants it.

**`--force` and the `auto` branch are unaffected.** An SVG loaded with no
reported viewport emits `width=auto;height=auto` around a PNG of the SVG's
natural size, which is well-defined precisely because §2.6's corrected note is
right that usvg always resolves one.

### 2.12 Output is a type too, and SVG is refused by name *(decision, recorded — resolves OQ-2 and Phase 3's round-1 blocker)*

§2.10 grew an `Input` type because `image::ImageFormat` cannot express SVG. **The
output side has the identical gap, and the identical trap.** Round 1 measured it:

```
ImageFormat::from_path("out.svg")
  → Err(Unsupported(UnsupportedError { format: PathExtension("svg"), … }))
```

whose rendered message is *"the file extension `svg` was not recognized as an
image format"* — **false**, since Tikray reads SVG perfectly well, and not the
"documented message" the phase's gate demands. So the obvious mechanism cannot
express the refusal, exactly as §2.10 found on the input side.

```rust
pub enum Output { Png, Jpeg }
pub fn resolve(dest: &Path, over: Option<&str>) -> Result<Output, TikrayError>
```

**The allowlist is the type.** §2.8 makes "support is an explicit allowlist, per
phase" a standing obligation, and Phase 3 discharges it structurally rather than
in prose: a two-variant enum cannot name a format no phase has gated. That is
worth doing here because the encode edge is *wider* than the decode edge —
measured, an RGBA8 buffer encodes happily to **Png, Jpeg, Gif, WebP, Bmp, Tiff,
Ico, Qoi and Tga**, so "whatever `image` will write" is nine formats, not two.

`resolve` reads `--format` when given, else the destination's extension, both
case-insensitively (`ImageFormat::from_path("OUT.PNG")` → `Png`, confirmed):

1. `png` → `Output::Png`; `jpg` or `jpeg` → `Output::Jpeg`.
2. `svg` or `svgz` → **refused by name** (below).
3. Any other extension → refused, naming it.
4. No extension and no `--format` → refused as undetermined.

**OQ-2 resolves as: SVG is input-only in v1, and both readings are refused.**
Raster→SVG is not a format change — it is tracing, which is a different and much
larger project — and SVG→SVG passthrough is the case §2.1 already excludes by
making the pixel buffer the narrow waist. One error variant carries both
readings, because the destination is `.svg` either way and only the source
differs:

```
cannot write SVG: tikray converts through a pixel buffer, so a raster in would
have to be traced (a different tool), and an SVG in would come back out a raster
wearing an .svg extension — supported output formats are PNG and JPEG
```

*Wrap-and-document was considered and rejected*: an `.svg` containing a base64
`<image>` element opens anywhere and scales like the raster it is, which makes
the extension a claim the file does not honour. Recorded as rejected rather than
omitted, so a later phase can adopt it with the trade-off already written down.

**Output errors are their own variants, never `FormatNotAllowed`.** That one's
message ends *"supported **input** formats are PNG, JPEG and SVG"*, so reusing it
for an output refusal would report about the wrong side of the pipeline.

**Neither flag takes a short form.** `-f` is the obvious abbreviation for both
`--format` and an overwrite guard, and `view` already ships `--force` with an
entirely different meaning (§2.7's detection bypass). One letter meaning three
things in one binary is a vocabulary collision, so the guard is spelled
`--overwrite` and nothing is abbreviated.

### 2.13 Alpha is composited over white, and quality is not a flag *(decision, recorded — resolves OQ-4)*

**The library's default here is silently and materially wrong**, which is why
this is a decision rather than a shrug. Measured against `image` 0.25.10:
`DynamicImage::write_to(_, Jpeg)` on an RGBA8 buffer **succeeds** by calling
`to_rgb8()`, which drops the alpha channel with no compositing at all:

| pixel | naive `write_to(Jpeg)` | composited over white |
|---|---|---|
| `[255, 0, 0, 128]` (50% red) | `[254, 1, 0]` | `[255, 127, 127]` |
| `[0, 0, 0, 0]` (transparent) | **`[0, 1, 0]` — black** | `[255, 255, 255]` |

So every transparent PNG or SVG converted to JPEG comes out on a **black**
background, exit 0, correct dimensions, correct format, no error. This is the
third instance of the shape §2.8 and §2.11 already record — silent, plausible,
and invisible to a gate that checks dimensions — and it is the one the observable
is most exposed to, because §1 judges the converted file by opening it.

**So Tikray composites explicitly, before encoding, whenever the target is JPEG
and the buffer carries an alpha channel:** `out = src·α + 255·(1−α)` per channel.
White, because it is what browsers and image viewers do, so the result matches
what the user last saw the file look like.

**And it says so** — one line on stderr naming the flattening, exit zero. Silence
would make this the fourth member of that list rather than the fix for it. The
note fires on the source buffer *having* an alpha channel, not on any pixel
actually being transparent: a coarser rule, and one an implementer cannot get
subtly wrong. **`src/main.rs:run` emits it, not `encode`** — `encode` is the phase's
pure seam, and a function that writes to stderr is not one.

**Quality and compression stay at the library defaults, with no flags** (JPEG
quality 75, via `JpegEncoder::new`). §1.1 makes editing a non-goal, and a
`--quality` flag is the thin end of exactly the operator set this project refuses
to grow. Recorded as available, not taken — a later phase can add it against a
real complaint rather than an imagined one.

**JPEG is not byte-exact, so no gate may key to a literal through it.** Measured:
`[255, 127, 127]` written and re-read comes back `[251, 125, 139]`, and
`[255, 255, 255]` comes back `[255, 255, 243]`. Phase 3's gate therefore asserts
the *distinction* — a flattened pixel is near-white, a dropped-alpha one is
near-black, some 250 apart — rather than an equality that would be pinning the
encoder's rounding.

### 2.14 The image is not in Ratatui's model; it survives by not being painted over *(decision, recorded — resolves OQ-1)*

Run 2026-08-17 in iTerm2, against `ratatui` 0.30.2 calling
`src/display.rs:sequence` through a path dependency — **the shipped display path,
not a reimplementation of it**, because the question is whether *tikray's*
sequence survives, not whether some sequence does. Nine repaints, one keypress
each:

| Repaint | Image |
|---|---|
| emitted into a bordered pane, sized to it | drew, inside the border |
| an identical frame (diff writes nothing) | survived |
| a counter changed **outside** the pane | survived |
| a widget drawing text **inside** the pane | **overwritten**, where the widget's cells landed |
| re-emit | restored |
| `terminal.clear()` + full redraw | gone |
| re-emit | restored |
| emitted at **natural size**, ignoring the pane | **spilled across both panes** |
| full redraw | recovered |

**The mechanism is the finding, not the yes.** Ratatui diffs and writes *cells*;
it has no notion of a region it must leave alone. The image survives because the
cells under it stay blank between frames and the diff therefore says nothing
about them — it is not an image widget, it is an image behind a hole in the
layout. Row 4 is what proves this: it died when a widget claimed those cells, not
merely because a frame was drawn. So **Phase 4's rule is: reserve a pane, render
nothing into it, and re-emit whenever the frame is invalidated** — by `clear()`,
by a resize, or by any frame that covers those cells.

**`src/display.rs:display` is not reusable inside the TUI; `sequence` is.** Row 8
measured why: at natural size the image spilled over the border and across the
neighbouring pane, because OSC 1337 draws at the cursor and iTerm2 clips it to
nothing. `display` reads the whole-window viewport from `src/term.rs:viewport`,
and a pane is not the window. Phase 4 calls `sequence` with pane-relative pixels
instead. That is Phase 1's pure seam paying off a second time — the same property
that made the gate runnable without a terminal is what makes the function
reusable under a different consumer.

**Cell geometry is arithmetic §2.6 never needed**, because it sizes to the whole
window and so never divided. Measured here: `window_size()` reports 2528×1584 px
over 158×44 cells → **16×36 px per cell**, and a pane of `w×h` cells is the
viewport `16w × 36h`. Two notes an implementer wants. The division is integer and
**truncates, which is the safe direction** — an underestimate leaves the image
slightly inside its pane, where an overestimate reproduces row 8 at small scale.
And it divided evenly on this machine, which will not always be true, so the
truncation is a live path rather than a formality. (16×36 for one cell is the
device-pixel signature OQ-7 recorded, seen from the other side.)

**Reserved for Phase 4, named so it is not discovered:** what the TUI does when
`window_size()` reports no pixels. §2.6's `auto` fallback is *unusable* here —
with no cell geometry there is no pane size, and `width=auto` is precisely row 8.
So that terminal either draws no image in the TUI or accepts the spill, and which
one is a Phase 4 decision. `view` is unaffected; it owns the whole window, which
is what makes `auto` fine there and not here.

**How the bytes are placed, since `sequence` returns no positioning and Ratatui's
`Frame` offers no raw-write escape.** The spike moved the cursor with
crossterm's `MoveTo(area.x, area.y)` on the same stdout the backend writes to,
wrote `sequence`'s bytes, and flushed — *after* `terminal.draw` returned, so the
backend's own flush had already landed. This is what "reserve a pane, render
nothing into it" costs in practice: the image is written outside Ratatui's
writer, and the ordering (draw, then place) is load-bearing rather than
incidental.

**Two corrections to how this subsection was first written**, both checked
against the crate rather than assumed. `ratatui-crossterm`'s `default` feature
**already selects `crossterm_0_29`**, so a plain `ratatui = "0.30"` produces one
crossterm instance on its own — the explicit feature the spike named is
belt-and-braces, not the thing that avoided a duplicate. And the spike was
throwaway: it left **no artifact in the repo**, so its nine rows are not
re-runnable by a second person. Its two load-bearing numbers do re-derive
(2528/158 = 16, 1584/44 = 36), which is why the claim stands, but that is a
weaker guarantee than Phases 1–3's gates carry and is recorded rather than
papered over.

## 3. Open questions

- **OQ-1** — ~~Can OSC 1337 inline images coexist with a Ratatui full-screen
  alternate-screen TUI, or does the image have to be drawn outside Ratatui's
  buffer model (which repaints cells and would erase or clip it)? If they
  cannot, Phase 4's shape changes: either a split where Ratatui browses and the
  image is drawn on suspend, or no alternate screen at all.
  *(deferred by evidence — settle it with a spike at the top of Phase 4, not by
  argument now)*~~ **RESOLVED 2026-08-17 by the spike — §2.14. They coexist**,
  and neither fallback is needed. The question's own framing conflated two
  things the spike separates: Ratatui repaints *cells*, not regions, so the
  image is never "erased by a repaint" as such — it survives exactly as long as
  no widget claims the cells under it, and it does not clip, it **spills**.
  **§2.9's grammar rides on this**: `tikray <path>` is defined as
  the TUI, so if Phase 4 is cut or re-shaped the bare-path form falls back to
  `view`'s inline behaviour rather than being left undefined. It is not cut, so
  the grammar stands as written.
- **OQ-2** — ~~What does "convert to SVG" mean? The seed document promises "any of
  the above into any of the others", but raster→SVG is not a format change: it
  is either vectorization/tracing (a different and much larger project) or
  wrapping the raster in an SVG container (technically an `.svg` file, arguably
  a lie). Options: refuse it with a clear error, wrap-and-document, or drop SVG
  from the output set entirely.~~ **RESOLVED 2026-08-16 — §2.12.** Refused, by
  name, both readings: **SVG is input-only in v1.** The seed's "any of the above
  into any of the others" is narrowed, which is why this was `needs-input` and
  was put to a person rather than decided at the keyboard. Wrap-and-document is
  recorded as rejected with its trade-off, not omitted. §2.1 had already
  foreclosed half of it — vector-to-vector passthrough is the case the waist
  excludes — so what needed an answer was raster→SVG, and the answer is no.
- **OQ-3** — ~~Display sizing policy. Terminal cells are not pixels, and an
  image larger than the window has to be fitted. What is the default
  (fit-to-width, fit-to-window, native size with scroll), and does the user
  override it?~~ **RESOLVED — §2.6.** Fit down, never up: `scale = min(W/w,
  H/h, 1.0)`, emitted in `px`. The premise above was half wrong and is left
  visible rather than edited away — Tikray does no cell/pixel conversion at all,
  because `width`/`height` accept a `px` form and the viewport is read in pixels
  from `window_size()`. No user override in v1; the sizing rule is not a flag.
- **OQ-4** — ~~Alpha and quality on export. JPEG has no alpha channel, so
  RGBA→JPEG must composite against something (white? black? a flag?) or refuse.
  Likewise JPEG quality and PNG compression level: defaults, or exposed?~~
  **RESOLVED 2026-08-16 — §2.13.** Composited over **white**, explicitly, with a
  line on stderr saying so; quality and compression stay at library defaults with
  no flags. The question understated itself: the library does not refuse and does
  not composite — it drops alpha, so a transparent pixel lands on **black**
  (measured `[0, 0, 0, 0]` → `[0, 1, 0]`). "Or refuse" was never the live
  alternative to "composite"; *silently wrong* was.
- **OQ-5** — ~~Does `view` need to survive tmux and ssh? tmux requires escape
  passthrough and iTerm2's protocol is commonly broken by it. Supporting it is
  small if designed in and awkward if retrofitted. *(needs-input — it is a
  question about how the user actually works)* **Blocks no phase; §2.7's
  `--force` is the stopgap.** It decides whether a later phase owns tmux
  passthrough, and it is the question that would reopen §2.7's rejection of
  Feature Reporting — so if the answer is "yes, tmux daily", that decision gets
  re-argued before Phase 4 rather than after.~~ **RESOLVED 2026-08-17 — put to a
  person, who answered: tmux rarely, and not daily.** So **no phase owns tmux
  passthrough**, and §2.7's rejection of Feature Reporting **stands un-reopened**
  — Phase 4 proceeds without re-arguing it, which is the branch this question
  existed to decide.

  Two notes, because the question was less symmetric than it looked. **The ssh
  half was already discharged** and did not need this answer: §2.7 checks
  `LC_TERMINAL` precisely because it is the variable that survives an ssh hop, so
  `view` over ssh works today. And **the tmux half's stated stopgap does not
  exist** — see §2.7's 2026-08-17 correction. "Rarely" is not "never", so this is
  recorded as *not designed for*, not as a non-goal: an occasional tmux user gets
  no picture rather than a degraded one, and the fix is two problems rather than
  the one §2.7 addresses.
- **OQ-6** — Multi-frame inputs (animated GIF, multi-page TIFF). §1.1 declares
  first-frame-only; the open part is whether the tool must *say* so, and where.
  ~~*(design call — **Phase 3**, which is the first phase whose allowlist can
  admit one.)*~~ **REASSIGNED 2026-08-16 — Phase 3's round 1 falsified the
  premise.** Phase 3 adds an *output* allowlist and leaves the input allowlist
  where Phase 2 put it, so no multi-frame input can reach it either: this is a
  design call for **whichever later phase grows the input set**, and there is
  none currently specced. §2.8's Phase 1 allowlist keeps GIF and TIFF out
  entirely, so this cannot fire there. The one residual case is APNG, which
  sniffs as PNG and so passes the allowlist: what `image`'s default decode path
  yields for one — first frame, default image, or an error — is unverified, and
  no phase through 3 claims or gates it.
- **OQ-7** — ~~Are `window_size()`'s pixel fields **device pixels or display
  points?**~~ **RESOLVED — measured at Phase 1's gate, 2026-08-16: device
  pixels.** An image wider than the window occupied approximately the full
  window width in iTerm2 on a Retina Mac, which is the signature §2.6's
  arithmetic requires. No divisor is needed, and Phase 2 does not inherit the
  problem. The original reasoning is kept because the *method* is the part worth
  preserving: this was invisible to every machine-checkable item in the gate,
  and only a human looking at the screen could have caught it.
  §2.6 compares image pixels against them as though they were the
  same unit. iTerm2's protocol documentation notes that Retina displays were
  "properly supported" from 3.2.0, "previously they would be double-size (one
  display 'point' per image pixel rather than one display pixel per image
  pixel)" — so if the viewport arrives in points on a Retina Mac, every image
  renders at roughly half its intended size. **It passes every machine-checkable
  item in Phase 1's gate**: the result is still whole, still undistorted, still
  never upscaled. Only a human looking at it can catch it, which is why gate
  item 5 now carries the clause. *(design call — **Phase 1 measures it**; the
  fix, if needed, is a divisor in §2.6 and nothing else. Phase 2 inherits it,
  since rasterizing an SVG at half the intended size wastes the one advantage
  vector input has.)*

- **OQ-8** — **Should vector input be allowed to render *larger* than its natural
  size?** §2.11 applies §2.6's never-upscale clamp to SVG exactly as to raster,
  so a `viewBox="0 0 24 24"` icon draws as a 24-pixel speck — consistent with a
  24×24 PNG, and arguably the whole advantage of vector input thrown away, since
  an SVG is the one input that *could* be re-rendered sharp at any size. The
  clamp was chosen deliberately (it is OQ-3's recorded resolution, and §1.1 makes
  scaling a non-goal in v1), so this is not an oversight; it is the cost of that
  consistency, now visible. Any fix is a user-facing scaling operation — a
  `--size`/`--scale` flag, or a rule that vector fits *up* to the viewport —
  which §1.1 excludes from v1 by name. *(design call — **deferred past v1**.
  Blocks no phase; Phase 2 ships the clamp and gate item 7 records what it looks
  like, so the decision is made against something seen rather than imagined.)*

## 4. Implementation phases

Strictly sequential. Each states the observable it produces and carries an exit
gate someone else could check.

### Phase 1 — view a raster image inline in iTerm2
*Produces the observable: yes — a PNG or JPEG drawn in the terminal window is
the observable's first half, and this is the smallest surface that reaches it.*

- **Scope:** a Rust binary crate, `tikray`, with these files and entry points:

  | File | Entry points |
  |---|---|
  | `src/main.rs` | `clap` parse, `view` subcommand dispatch, error → stderr + non-zero exit |
  | `src/error.rs` | `pub enum TikrayError` + `Display` — one variant per failure in the gate |
  | `src/load.rs` | `pub fn load(path: &Path) -> Result<DynamicImage, TikrayError>` |
  | `src/term.rs` | `pub fn detect_iterm2(force: bool) -> Result<(), TikrayError>`, `pub fn viewport() -> Option<(u32, u32)>` |
  | `src/display.rs` | `pub fn fit(native: (u32,u32), viewport: Option<(u32,u32)>) -> Option<(u32,u32)>`, `pub fn sequence(img: &DynamicImage, viewport: Option<(u32,u32)>) -> Result<Vec<u8>, TikrayError>`, `pub fn display(img: &DynamicImage, out: &mut impl Write) -> Result<(), TikrayError>` |

  One subcommand, `tikray view [--force] <path>`. `load` sniffs by content and
  enforces the PNG/JPEG allowlist (§2.8). `fit` is §2.6's arithmetic and nothing
  else — `None` in means the viewport was unreported, `None` out means emit
  `auto`. `sequence` is a pure function from buffer plus viewport to the
  complete escape-sequence bytes (§2.3), which is the seam that makes this phase
  testable without a terminal; `display` is the only part that touches stdout.
  Errors for file-missing, not-an-image, format-not-allowed, not-a-tty and
  not-iTerm2 are text a user can act on, not a panic or a `Debug` dump.

- **Exit gate:** four assertions runnable by `cargo test` with no terminal, and
  one a human checks in iTerm2 — the machine half is what catches a wrong
  implementation, and the human half is the only thing that can confirm the
  observable.

  1. **`fit` reproduces §2.6.** Native `(800, 600)` into viewport `(1600, 1200)`
     returns `(800, 600)` — clamped, *not* `(1600, 1200)`. Native `(4000, 1000)`
     into `(1000, 1000)` returns `(1000, 250)`: `scale = min(0.25, 1.0, 1.0)`.
     Native `(10, 3)` into `(1, 100)` returns `(1, 1)`: `scale = 0.1`, so the
     height computes `round(0.3) = 0` and the `max(1, …)` floor is what rescues
     it — a zero dimension is not a legal argument value.
     Viewport `None` returns `None`. A viewport reporting `0` in either axis is
     read as unreported and yields `None` (§2.6).
  2. **`sequence` emits the protocol.** Output starts with the bytes
     `\x1b]1337;File=`, its argument segment contains `inline=1` and exactly the
     `width=<W>px;height=<H>px` that `fit` returned for that input (or
     `width=auto;height=auto`), and it ends with `\x07`. The base64 payload
     decodes to a PNG whose dimensions equal the source buffer's. **The
     `inline=1` assertion is not ceremony** — without it the sequence is still
     well-formed and displays nothing, so it is the one byte the machine half
     exists to pin.
  3. **`load` honours the allowlist.** PNG and JPEG fixtures decode to their
     known dimensions. A PNG fixture renamed `.txt` still loads (content
     sniffing, §2.8). A text file renamed `.png` is refused as **format-could-
     not-be-determined, not as a corrupt PNG** — that distinction is the whole
     evidence that §2.8's third construction was used and not its second. A
     GIF fixture is refused with a message naming GIF as the detected format.
  4. **Refusal is not silent.** With stdout not a tty, and with both
     `TERM_PROGRAM` and `LC_TERMINAL` cleared, `view` exits non-zero and writes
     **zero bytes** to stdout. With `--force` under the same conditions it exits
     zero and writes a sequence — asserted **prefix-only** (`\x1b]1337;File=`,
     `inline=1`, `\x07`), never on the sizing arguments. `window_size()` reads
     `/dev/tty` and only falls back to stdout, so *if* the viewport resolves on
     the machine running this test, a piped run still emits `px`, while an
     environment with no controlling terminal emits `auto`. Which of the two
     this machine is, is item 5's measurement — the point here is only that the
     two can differ, so a test asserting sizing arguments would pass in one
     environment and fail in the other. Item 1 is where sizing is pinned, with
     the viewport injected rather than discovered.

     *(That two-descriptor split is deliberate but worth seeing: §2.7 gates on
     stdout being a tty, §2.6 reads the viewport from `/dev/tty`. They can
     disagree — piped stdout under a real terminal — and `--force` is the only
     way to reach that state, which is why the gate names it here.)*
  5. **Human, in iTerm2:** `tikray view <a.png>` and `tikray view <a.jpg>` each
     draw the correct picture inline. An image larger than the window appears
     whole and undistorted inside it; **an image much smaller than the window —
     a 16×16 icon — appears small, not blown up to fill it.** Two measurements
     get recorded in the review log while a human is already sitting there,
     because both are free to take at this moment and expensive to reconstruct
     later:
     - whether `window_size()` reports zero pixels here — if so, the §2.6
       fallback is the primary path, and Phase 2's rasterization target size
       inherits the problem;
     - whether an image wider than the window **occupies approximately the full
       window width, not about half of it** (OQ-7 — the Retina points-vs-pixels
       question). Half-width is the signature of the units being wrong, and it
       is invisible to every other item in this gate.

- **Close-out:** seeds `rules/core-pipeline.md` (the `DynamicImage` waist, the
  load path, the allowlist) and `rules/iterm2-display.md` (the argument string,
  §2.6's arithmetic, the detection rule and `--force`). Commits the crate
  skeleton, the four modules, the `view` subcommand, the test fixtures and the
  tests above.

### Phase 2 — view an SVG, by rasterizing into the same buffer
*Produces the observable: yes — the same drawn image, from vector input.*

- **Scope:** `resvg`/`usvg` behind the same
  `src/load.rs:load` entry point Phase 1 shipped — **its signature does not
  change** (§2.11). Dispatch is on detected input type via the new `Input` enum
  (§2.10), never on the path extension.

  | File | Entry points |
  |---|---|
  | `src/load.rs` | `pub enum Input { Raster(ImageFormat), Svg }`, `pub fn detect(head: &[u8]) -> Option<Input>`; `load` reads the file once, sniffs, and dispatches |
  | `src/svg.rs` | `pub fn rasterize(path: &Path, bytes: &[u8]) -> Result<DynamicImage, TikrayError>` |
  | `src/error.rs` | `+ SvgParse { path, source }`, `+ Rasterize { path, reason }` |
  | `src/lib.rs` | `pub mod svg;` |

  `load` now reads the whole file into memory before sniffing, because `usvg`
  needs the bytes anyway. **The §2.8 property is preserved and must stay
  preserved:** the reader is built as `ImageReader::new(Cursor::new(&bytes))`,
  seeding nothing from the path, so a text file named `.png` is still
  `FormatUndetermined` and a PNG named `.txt` still loads. `rasterize` is a
  deterministic function of `bytes` — the `path` is read only to label errors,
  following `src/error.rs:TikrayError::io`'s precedent, since `SvgParse` and
  `Rasterize` both carry one — which is the seam that makes this phase testable
  without a terminal, exactly as `sequence` was for Phase 1.

- **Exit gate:** six machine-checkable items plus one a human checks. The blast
  radius is the shared `load` entry point, so item 6 is not optional.

  1. **`detect` reproduces §2.10.** `rgb.png` → `Some(Raster(Png))`; `rgb.jpg` →
     `Some(Raster(Jpeg))`; a bare `<svg>` fixture, one with an XML prolog, and one
     with a UTF-8 BOM each → `Some(Svg)`; a `.svgz` → `None`; and
     **`not_an_image.png` → `None`** — Phase 1's shipped assertion restated at the
     new seam, which is the one §2.10 exists to protect.
  2. **`load` rasterizes into the waist at `usvg`'s resolved size.**
     `viewBox="0 0 24 24"` with no `width`/`height` → **24×24**;
     `width="10mm" height="5mm"` → **38×19** (`to_int_size()` rounds
     37.795×18.898); neither attribute nor viewBox, containing a lone 10×10 rect
     → **10×10**.
  3. **Alpha survives the premultiplied boundary.** A 50%-opacity red rect
     rasterizes to a pixel equal to `[255, 0, 0, 128]`, **not** `[128, 0, 0, 128]`
     (§2.11). Dimensions alone cannot catch this, which is why it is a pixel
     assertion.
  4. **Text renders.** A 24px `<text>Hello</text>` SVG rasterizes to a **non-zero**
     count of non-transparent pixels — zero is what an empty font database
     produces, silently. *This item is knowingly environment-dependent*: it
     passes because the machine running it has system fonts, and would fail in a
     fontless container where `load_system_fonts()` finds none. That is the
     property Phase 1's gate item 4 was reshaped to avoid, accepted here because
     the alternative — bundling a font — is a heavier decision than the assertion
     is worth, and because a fontless build genuinely cannot render text.
  5. **A malformed SVG is refused, not blanked.** Bytes that `detect` calls `Svg`
     but `usvg` rejects produce `SvgParse`, and the CLI exits non-zero writing
     **zero bytes** to stdout. ("Not a blank image" is unreachable by
     construction — `src/main.rs:run` returns before `display` writes — so the
     assertion is on the refusal, not on the absence of a draw.)
  6. **Phase 1's gate still passes, unmodified.** All 16 assertions in
     `tests/gate.rs` green with no edits to that file. Phase 2 changes the shared
     entry point; this is what proves it changed nothing behind it.
  7. **Human, in iTerm2:** `tikray view <a.svg>` draws the SVG. One whose natural
     size exceeds the window appears whole and undistorted. **A 24×24-viewBox icon
     appears at 24 px — small.** That is §2.6's clamp applying to vector exactly
     as to raster, and recording how it looks is what makes OQ-8 a decision taken
     against something seen.

- **Close-out:** adds `rules/svg-rasterization.md`. Updates
  `rules/core-pipeline.md` with the vector branch **and its frontmatter** — the
  new `src/svg.rs` must be added to that file's `sources:`, or `/sync-rules`
  regenerates it without ever reading the module; it sits at 48/50 lines, so
  `max_lines` needs raising in the same edit. Updates the README's supported-input
  table, **and `src/error.rs:TikrayError`'s `FormatNotAllowed` message**, which
  ends "supported input formats are PNG and JPEG" and becomes false the moment the
  allowlist grows. `tests/gate.rs` needs no edit for it — the shipped assertion
  checks only that the message names GIF — which is what keeps gate item 6
  satisfiable.

### Phase 3 — convert: write the buffer back out in another format
*Produces the observable: yes — the observable's second half, a file that opens
and looks right.*

- **Scope:** `tikray convert [--format <fmt>] [--overwrite] <in> <out>`. Encodes
  the same buffer Phases 1–2 fill, so every input type already supported is
  convertible on arrival — verified, not assumed: `DynamicImage::write_to` routes
  through `make_compatible_img`, so Rgb8, Rgba8 and Rgb16 all encode to PNG and
  JPEG without error. **The input allowlist does not change**; this phase adds an
  *output* one (§2.12). OQ-2 and OQ-4 are resolved in §2.12 and §2.13 — they were
  answered in the spec before this phase was cleared, which its round 1 enforced.

  | File | Entry points |
  |---|---|
  | `src/convert.rs` | `pub enum Output { Png, Jpeg }`; `pub fn resolve(dest: &Path, over: Option<&str>) -> Result<Output, TikrayError>`; `pub fn flatten(img: &DynamicImage) -> RgbImage`; `pub fn encode(img: &DynamicImage, target: Output) -> Result<Vec<u8>, TikrayError>` |
  | `src/error.rs` | `+ OutputUndetermined { path }`, `+ OutputNotAllowed { path, name }`, `+ OutputSvg { path }`, `+ OutputExists { path }`, `+ Write { path, source }` |
  | `src/main.rs` | a `Convert { input, output, format, overwrite }` arm in `run` |
  | `src/lib.rs` | `pub mod convert;` |

  **`encode` is the pure seam**, as `src/display.rs:sequence` was for Phase 1 and
  `src/svg.rs:rasterize` for Phase 2: buffer plus target in, bytes out, no
  filesystem — which is what lets the gate assert on encoded bytes without
  writing files. `flatten` is pure too, and separate, because §2.13's compositing
  is the one arithmetic worth asserting on its own. `run`'s order is **resolve the
  output, check the overwrite guard, load, encode, write**: both refusals are
  cheap and come before the decode, so a run that cannot possibly succeed does no
  work and touches no file.

- **Exit gate:** seven items runnable by `cargo test` with no terminal, and one a
  human checks. The blast radius is a new edge on the shared waist plus a second
  subcommand, so item 7 is not optional.

  1. **`resolve` reproduces §2.12.** `out.png` → `Png`; `out.PNG` → `Png`
     (case-insensitive, confirmed); `out.jpg` and `out.jpeg` → `Jpeg`;
     `--format jpeg` over a `.png` destination → `Jpeg`, so the override beats the
     extension. `out.svg` → `OutputSvg`, **not** the `image` crate's
     "not recognized as an image format" — that distinction is the evidence
     §2.12's own type was used rather than `ImageFormat::from_path`. `out.gif` →
     `OutputNotAllowed` naming GIF. `out`, with no `--format` → `OutputUndetermined`.
  2. **Alpha is composited, not dropped (§2.13).** `flatten` maps
     `[255, 0, 0, 128]` → `[255, 127, 127]` and `[0, 0, 0, 0]` → `[255, 255, 255]`.
     End to end, a transparent-PNG-to-JPEG conversion reads back with **every
     channel above 200** at that pixel. The threshold is deliberate and is the
     item's whole design: JPEG is not byte-exact — measured, white returns as
     `[255, 255, 243]` — while the bug it guards against returns `[0, 1, 0]`, so
     the two are ~250 apart and no equality assertion is needed to separate them.
     A literal here would be pinning the encoder's rounding instead.
     **The success path's stderr is asserted here too**, not left to the human
     item: `convert alpha.png out.jpg` exits **zero** and writes a non-empty
     stderr naming the flattening (§2.13). Item 6 asserts stderr on the refusal
     paths; this is the one the decision is actually about, and unlike "the
     picture looks right" it is trivially machine-checkable.
  3. **The waist is not quantized on the way out.** A 16-bit PNG fixture converted
     to PNG reads back as **`Rgb16`** with the pixel `[65535, 1234, 7]` intact.
     Under a `to_rgba8()` first — the one-liner an implementer reaches for at this
     edge — it reads back `Rgba8` with `[65535, 1285, 0]` **compared in 16-bit
     space** (the stored pixel is the 8-bit `[255, 5, 0, 255]`; 1234/257 = 5, and
     5·257 = 1285 back up), and **both files report `format = Png` and
     `dims = (2, 2)`**. So this item asserts colour type and
     pixel, never dimensions and format, which is exactly the assertion §2.1 has
     owed since Phase 1: it defends `DynamicImage` over RGBA8 by saying bit depth
     is "material for Phase 3, whose gate reads the converted file back and
     compares it", and a gate keyed to dimensions cannot cash that claim.
  4. **Every allowed (input, output) pair round-trips.** Inputs `rgb.png`,
     `rgb.jpg` and `icon24.svg` × outputs PNG and JPEG: the written file is
     re-read by content sniffing (§2.8's construction, not by extension) at the
     expected format and dimensions, and `src/load.rs:load` accepts it. That last
     clause is what makes "and `tikray view` displays it" checkable **without a
     terminal** — once `load` accepts the file, `view`'s only remaining step is
     `sequence`, which Phase 1 item 2 already gates.
  5. **The overwrite guard.** Converting onto an existing path fails with
     `OutputExists` and leaves that file's bytes **byte-for-byte unchanged**;
     with `--overwrite` it exits zero and the bytes change. The unchanged-bytes
     half is the one worth asserting — a guard that errors *after* truncating
     would pass a weaker check.
  6. **Refusal writes no file and says why.** `convert x.png out.svg`,
     `out.gif` and `out` each exit non-zero, write to stderr, and leave **no
     file at the destination**. The SVG message names both readings (§2.12).
  7. **Phases 1 and 2's gates still pass, unmodified.** All 27 assertions —
     16 in `tests/gate.rs`, 11 in `tests/gate_phase2.rs` — green with no edits to
     either file. Phase 3 adds an edge to the shared waist; this is what proves it
     changed nothing behind it, and it is Phase 2's item 6 generalized.
  8. **Human, in iTerm2:** convert a PNG, a JPEG and an SVG to each of PNG and
     JPEG, open the results in a normal image viewer, and `tikray view` them —
     they show the same picture, right way up, right colours. A transparent
     source converted to JPEG comes out on **white**, not black, and the run says
     on stderr that alpha was flattened. This is the observable's second half, and
     no assertion above can confirm it: item 2 proves one pixel was composited,
     not that a person opening the file sees the picture they expected.

  New fixtures, **both pinned to an exact layout, because item 2's threshold is
  layout-sensitive**: `deep16.png` is 2×2 16-bit RGB carrying `[65535, 1234, 7]`
  at (0,0), and `alpha.png` is 2×2 RGBA laid out
  `[255,0,0,128], [0,0,0,0], [255,0,0,128], [0,0,0,0]` — the same fixture every
  number in §2.13's table was measured on. The pin is not tidiness: round 2
  measured a *correct* flatten→JPEG on an 8×8 opaque-red-with-one-clear-corner
  layout returning a min channel of **194**, which fails item 2's `> 200` bar. The
  failure is only in the false-red direction, but a gate item that depends on a
  fixture nobody wrote down is one an implementer can reproduce as broken.
  (`alpha.svg` covers the vector path; `alpha.png` is the decode one.) Tests land
  in `tests/gate_phase3.rs`; `tests/gate.rs` and `tests/gate_phase2.rs` are not
  edited, which is item 7.

- **Close-out:** adds `rules/convert.md`. Updates `rules/core-pipeline.md` with
  the encode edge and the format matrix **and its frontmatter** — `src/convert.rs`
  must join that file's `sources:`, or `/sync-rules` regenerates it without ever
  reading the encode path, and it sits at **68/70 lines**, so `max_lines` needs
  raising in the same edit. Updates `README.md` (the "Status: Phase 2" banner, the
  Usage block, which lists only `view`, and the supported-formats table, which a
  convert command makes incomplete) and `tikray.md`'s status line. `CLAUDE.md`
  needs no change — §2.9's grammar is unaltered.

### Phase 4 — the TUI shell over the same core
*Produces the observable: yes — images drawn inside a browsing UI.*

**The OQ-1 spike this phase used to open with is done (§2.14), and the cut
branch it guarded can no longer fire.** The phase originally read "begins with
the OQ-1 spike" and "if the protocol and Ratatui cannot share a screen, this
phase is re-specced or cut". They can share a screen, measured, so the spike is
not step 1 and the phase is not conditional. §2.14 is the input this scope is
written against.

- **Scope:** a Ratatui + crossterm file browser that opens what it lands on
  through the Phase 1–3 core. **Owns both TUI entry points — bare `tikray` and
  `tikray <path>` (§2.9).** Converting from inside the TUI is **Phase 5**, not
  this phase (see there for why).

  | File | Entry points |
  |---|---|
  | `src/cli.rs` | `pub struct Cli` + `pub enum Command`, **moved out of `src/main.rs`** so gate item 3 can parse argv without running a TUI |
  | `src/tui.rs` | `pub fn run(start: Option<&Path>) -> Result<(), TikrayError>`; `pub fn pane_viewport(pane: (u16,u16), cell: Option<(u32,u32)>) -> Option<(u32,u32)>`; `pub fn pane_sequence(img: &DynamicImage, pane: (u16,u16), cell: Option<(u32,u32)>) -> Result<Option<Vec<u8>>, TikrayError>` — `Ok(None)` means *draw the explanation and emit nothing* |
  | `src/term.rs` | `+ pub fn geometry() -> Option<((u32,u32), (u16,u16))>` — the reader; `+ pub fn cell_size(px: (u32,u32), cells: (u16,u16)) -> Option<(u32,u32)>` — the arithmetic, pure |
  | `src/error.rs` | `+ Tui { source: std::io::Error }` |
  | `src/main.rs` | dispatch only: no subcommand → `tui::run(None)`; a bare path → `tui::run(Some(p))` |
  | `src/lib.rs` | `pub mod cli; pub mod tui;` |
  | `Cargo.toml` | `ratatui = "0.30"` (0.30.2 current; its `crossterm_0_29` default already matches §2.4's crossterm — §2.14), `signal-hook = "0.3"` (0.3.18 current) |

  **`src/display.rs:display` is not used by this phase and `sequence` is** — it
  reads the whole-window viewport, and a pane is not the window (§2.14). The TUI
  calls `sequence` with pane pixels and places the bytes itself.

  **`term::geometry` and `term::cell_size` are two functions on purpose**, and
  the split is Phase 1's, not a new one: `src/term.rs:viewport` reads the
  terminal and takes no arguments, `src/display.rs:fit` is pure with the value
  injected, and that is the only reason Phase 1's gate runs headless. A single
  zero-argument `cell_size()` would have to call `window_size()` itself, which
  returns `Err` under `cargo test`, and gate item 1 could then never reach its
  assertion. `geometry` returns **both** pairs from **one** `window_size()` call
  rather than exposing a second reader beside `viewport`, because two separate
  reads can straddle a resize and yield a cell size that never existed.

  **Three decisions this scope settles, because an implementer cannot guess
  them:**

  1. **The clap tree.** `src/main.rs:Cli` is `command: Command`, non-optional
     with no positional, so `tikray` and `tikray x.png` both exit **2** today —
     verified. It becomes `command: Option<Command>` plus `path:
     Option<PathBuf>`. The collision this creates is a file named `view` or
     `convert`: **a bare first argument is a subcommand when it exactly matches
     one, and a path otherwise**, with `./view` as the escape hatch. Stated
     because it is a rule, not a default.
  2. **What `tikray <path>` opens** — §2.9 reserved this rather than settling
     it. **The browser at that path's directory, with the path highlighted**;
     given a directory, the browser there with its first entry highlighted. Not
     a single-image view, because that is `view`'s job and §2.9's whole point is
     that the two surfaces stay distinguishable.

     **CORRECTED 2026-08-17 — superseded by Phase 6**, which dispatches on the
     path's type: a file draws inline, a directory browses. This decision shipped
     and was used before it was reversed, which is the only reason the reversal
     is well-founded — see §2.9's note and Phase 6. The directory half survives
     verbatim; only the file half changes.
  3. **What happens when `cell_size()` is `None`** — §2.14 reserved this. **The
     TUI runs and draws no image**, showing one line in the pane saying why.
     Not `auto`: that is §2.14's row 8, which spills across the layout, and a
     browser that wrecks its own screen is worse than one without previews.
     Refusing to launch is worse still — the file list is useful on its own.

- **Exit gate:** five items runnable by `cargo test` with no terminal, and two a
  human checks. The blast radius is a second caller of the core (§2.2) plus a
  dependency that co-owns crossterm with `src/term.rs`, so item 4 is not
  optional.

  1. **`cell_size` reproduces §2.14's arithmetic.**
     `cell_size((2528, 1584), (158, 44))` → `Some((16, 36))`. A zero in **any**
     of the four → `None`, which is §2.6's unreported rule extended to the two
     new fields. The four integers are arguments, which is what lets this run
     with no terminal; `geometry` is the half that reads one and is exercised by
     item 6.
  2. **`pane_viewport` turns cells into pixels.** A 40×20 pane at `(16, 36)` →
     `(640, 720)`, and `sequence` on a 1200×800 buffer with that viewport emits
     exactly `width=640px;height=427px` — `fit`'s clamp applied to a pane rather
     than a window, which is the one place Phase 4 touches shipped arithmetic.
     `cell` of `None` → `None`, which is item 5's input. **And a zero in either
     pane axis → `None` too**: a pane shrunk to nothing under a bordered layout
     otherwise multiplies out to `(0, …)`, `src/display.rs:fit` returns `None`
     for a zero axis, and `sequence` then emits `width=auto;height=auto` — which
     is §2.14's row 8 reached from the other side, and the one path by which the
     spill decision 3 forbids can still get in. Asserted rather than inherited.
  3. **The clap tree parses all four invocations**, via `Cli::try_parse_from`
     with no TUI: `["tikray"]` → no subcommand, no path; `["tikray", "a.png"]` →
     path, no subcommand; `["tikray", "view", "a.png"]` → `View`; `["tikray",
     "convert", "a.png", "b.jpg"]` → `Convert`. **And `["tikray", "view"]` is an
     error, not a bare path named `view`** — that is decision 1 being pinned
     rather than described.
  4. **Phases 1–3's gates still pass, unmodified.** All 39 assertions — 16 in
     `tests/gate.rs`, 11 in `tests/gate_phase2.rs`, 12 in `tests/gate_phase3.rs`
     — green with no edits to any of the three. This is Phase 3's item 7
     generalized again, and it matters more here: this phase moves `Cli` out of
     `src/main.rs` and adds a crate that co-owns crossterm.
  5. **The no-pixel fallback emits nothing.** `pane_sequence(img, pane, None)`
     returns `Ok(None)` — **zero escape bytes** — so the pane draws decision 3's
     explanation instead. With a `cell` of `Some((16, 36))` the same call returns
     `Ok(Some(bytes))` carrying item 2's arguments, so one function covers both
     branches. Asserted on it as a pure function because the failure it guards is
     §2.14's row 8, and a spilled image is not something a test can see.
  6. **Human, in iTerm2:** `tikray` opens the browser; arrow-key navigation
     redraws the highlighted image; `tikray <path>` opens at that path's
     directory with it highlighted; `tikray view <path>` still draws inline and
     exits. Quitting restores the terminal — no residual alternate screen, no
     swallowed cursor, no leaked escape state. **And the named visual property,
     which is the whole reason this item is human: the image sits inside its
     pane and does not cross the border**, including while the window is
     resized. §2.14's headline hazard is a spill, item 5 says outright that no
     test can see one, so this is the only place it is caught — and Phases 1–3
     each named their human item's property this specifically rather than
     asking whether it "looks right".
  7. **Human: the terminal survives both interruptions, which are two different
     mechanisms.** crossterm 0.29's raw mode goes through `cfmakeraw`, which
     clears `ISIG` — so **Ctrl-C inside the TUI arrives as a `KeyEvent`, not a
     signal**, and is handled as quit. A real `kill -INT <pid>` *is* a signal,
     default-terminates without unwinding, and so runs no `Drop`: that is what
     `signal-hook` is pinned for. Both are checked, because a gate saying only
     "after a `SIGINT`" does not distinguish them and the two need different
     code. A `Drop` guard plus a panic hook covers normal quit and panic.

  Tests land in `tests/gate_phase4.rs`; the three existing gate files are not
  edited, which is item 4. Items 6–7 ship as `scripts/gate-phase4.sh`, following
  `scripts/gate8.sh` — a gate someone else could check is the point of a gate.

- **Close-out:** adds `rules/tui.md`. **Three existing rules change, and two need
  frontmatter edits in the same pass or `/sync-rules` regenerates them against
  sources they never declared:**

  | Rule | Why it changes | Frontmatter |
  |---|---|---|
  | `rules/iterm2-display.md` | `sources` are `src/display.rs` **and `src/term.rs`** — both touched | at **50/50**, so `max_lines` must rise |
  | `rules/core-pipeline.md` | the second caller §2.2 exists for; `src/lib.rs` in `sources` | at **88/90**, so `max_lines` must rise, **and `src/tui.rs` + `src/cli.rs` join `sources`** |
  | `rules/convert.md` | its `sources` include **`src/main.rs`**, which this phase restructures — `Cli` moves out and two dispatch arms are added, and its `covers` names "the order `run` takes its two cheap refusals in" | at 52/55, no raise needed |

  Updates `README.md` — the "Status: Phase 3" banner and the Usage block, which
  lists only `view` and `convert` — and `tikray.md`'s status line and its
  unticked "TUI shell (Ratatui)" item. **`CLAUDE.md` needs no change** unless
  decision 1 alters the grammar §2.9 records, which it does not.

### Phase 5 — convert from inside the TUI
*Produces the observable: yes — the observable's second half, reached from the
browsing surface.*

**Split out of Phase 4 by its round 1, for a reason worth keeping.** Phase 4 read
"plus a key to convert the selected file", one clause, gated by nothing — and it
hides a decision the rest of the spec actively contradicts. **§2.13 requires the
alpha-flattening notice, and puts it in `src/main.rs:run` as an `eprintln!`
because `encode` is the pure seam. Inside an alternate screen, stderr writes
straight over the TUI.** Phase 3's gate item 2 asserts that line, so it cannot
quietly be dropped; it has to go somewhere else, and *where* is a decision, not a
bullet. Phase 4 was also already the largest phase in the spec against §3's "one
phase = one plan-mode pass".

- **Scope:** a key in the browser that converts the highlighted file. Settles,
  because none of it is inferable: **where the output lands** (destination path
  rule), **how the target format is chosen** (a prompt, or a fixed pair of
  keys), **what replaces `--overwrite`** now that there is no flag, and **where
  §2.13's flattening notice is surfaced** now that stderr is unusable. It calls
  `src/convert.rs:resolve` and `src/convert.rs:encode` directly rather than
  `src/main.rs:run`, since `run` is the CLI caller and owns the `eprintln!`.
- **Exit gate**, three items its own round is expected to sharpen and add to —
  **not** a placeholder, because §3 requires every phase to carry one:
  1. The flattening notice reaches a human **without leaving the TUI**, on the
     same condition §2.13 states — the buffer *having* an alpha channel, not any
     pixel being transparent. `eprintln!` is unavailable inside the alternate
     screen, so this is the item the phase exists to answer.
  2. The overwrite guard still refuses, leaving the destination **byte-for-byte
     unchanged** — Phase 3's item 5, restated at a surface with no `--overwrite`
     flag to carry it.
  3. Phases 1–4's gates still pass, unmodified.
- **Close-out:** updates `rules/tui.md` and `rules/convert.md`; updates
  `README.md`'s TUI section with the key and the destination rule.

### Phase 6 — the bare path draws inline, and the preview sits in the middle
*Produces the observable: yes — both halves of it. The first change is about how
the user reaches an image drawn inline, and the second is about where in the
pane that image appears.*

**Appended 2026-08-17, after Phase 4 shipped and was used.** Two changes the
human asked for having run the thing, which is the only standing that reverses an
argued decision. Numbered after the last existing phase per §6.1; **it is
expected to ship before Phase 5**, since it corrects what Phase 4 shipped an hour
earlier and Phase 5 has not started — an out-of-order `shipped` wants a one-line
reason in the review record, and this is it.

They are one phase rather than two because both are small, both touch
`src/tui.rs` or its caller, and neither is worth its own review round. They are
independent, so the gate keeps them separate.

- **Scope, part 1 — the grammar dispatches on the path's type (§2.9's corrected
  note).**

  | Invocation | Surface | Changed? |
  |---|---|---|
  | `tikray` | browser, in the working directory | no |
  | `tikray <dir>` | browser, in that directory | no |
  | `tikray <file>` | **inline, one-shot** | **yes — was the browser** |
  | `tikray --browse <file>` | **browser, that file highlighted** | **new spelling of Phase 4's behaviour** |
  | `tikray view [--force] <path>` | inline, one-shot | no |
  | `tikray convert …` | writes a file | no |

  **`src/cli.rs` gains one `bool` field**, plus a rewritten module doc — the one
  it ships argues the reversed grammar verbatim, `vim` analogy included, and a
  file that still makes the old case is worse than one that never made it, since
  `/sync-rules` reads it. Phase 4's tree already parses a bare path into
  `Cli::path`; what changes is `src/main.rs:run`'s dispatch of that value, from
  "always the browser" to "stat it, then choose". So **Phase 4's gate item 3
  stays true unmodified** — all five of its parse cases, including `./view` —
  because those assertions read `command` and `path` by field, and a new flag
  beside them changes neither. That is the item to watch, not a formality:
  parsing is not what this phase changes, and if item 3 goes red the two have
  been conflated.

  Four things an implementer cannot guess, settled here:

  1. **The file branch *is* the `View` arm with `force: false`**, reached
     without the subcommand — not a second inline path beside it. It therefore
     calls `src/term.rs:detect_iterm2` before anything reaches stdout, exactly as
     §2.7 requires, and stating that is not pedantry: an implementer who wires
     the branch straight to `src/display.rs:display` makes `tikray x.png >
     out.txt` fill a file with escape bytes, which is the failure §2.7's tty half
     exists to stop and which no assertion in Phase 4's gate would catch.
  2. **The stat is the dispatch, so its failure is the dispatch's failure.** A
     path that does not exist is `TikrayError::Io` from that call, before either
     surface starts. **Under a pipe that is a change, not a relocation**, and the
     difference is worth naming: today `src/tui.rs:run` checks the terminal
     before `src/tui.rs:Browser::open` reads anything, so `tikray <missing>`
     with stdout redirected reports `NoScreen`; after this phase it reports
     `Io`, because the stat now comes first. A path that is neither file nor
     directory (a socket, a fifo) takes the **file** branch and is refused by
     `load` as an undetermined format, because that is the branch with a message
     about what the bytes are.

     `Browser::open` keeps its own `metadata` call, so the browse branch stats
     twice. That is deliberate rather than overlooked: the two stats answer
     different questions — which surface runs, and what the browser starts on —
     and threading one answer into the other couples a dispatch arm to a
     constructor for no gain.
  3. **`--browse` selects a surface; `--force` modifies one. That is why they
     sit on different invocations**, and it is the answer to §2.9's "`view`
     becomes pure noise" — an objection this phase has to answer rather than
     dismiss, since it was the strong half of the argument being reversed.
     `--force` changes *how the inline draw behaves* (§2.7's detection bypass),
     so it belongs to the invocation that draws inline and stays on `view`
     alone. `--browse` chooses *which surface runs at all*, which is the one
     thing the bare form has to be able to say now that its meaning depends on
     what the path turns out to be. So `view` still owns the options, and the
     bare form owns exactly one surface selector.
  4. **`--browse` overrides the stat rather than consulting it.** With a
     directory it is a no-op, and with a missing path it still fails at the stat
     — the browser needs a directory to list either way. Given a file it opens
     that file's directory with the file highlighted, which is Phase 4's
     decision 2 preserved verbatim rather than reimplemented. Given no path at
     all it is the bare `tikray`, not an error: a flag that selects the default
     surface has nothing to complain about.

- **Scope, part 2 — the image is centred in its pane.**

  | File | Entry points |
  |---|---|
  | `src/tui.rs` | `+ pub fn centre_offset(image: (u32,u32), pane: (u16,u16), cell: (u32,u32)) -> (u16,u16)` — the offset in **cells**, pure |
  | `src/tui.rs` | `+ pub fn pane_offset(img: &DynamicImage, pane: (u16,u16), cell: Option<(u32,u32)>) -> (u16,u16)` — the glue, mirroring `pane_sequence` |
  | `src/cli.rs` | `+ browse: bool` on `Cli`, part 1's surface selector |
  | `src/main.rs` | part 1's type-dispatch, in `run` |

  **`src/tui.rs:pane_sequence`'s signature does not change**, deliberately:
  Phase 4's gate item 5 asserts on it, and a phase that edits a shipped gate to
  make room for itself has removed the gate's evidence. The offset is a second
  pure function over the same integers, and `src/tui.rs:Session` applies it at
  the `MoveTo`.

  **`centre_offset`'s `image` argument is the pair `src/display.rs:fit`
  returned — the size the terminal is told — and never the buffer's native
  size.** This is the phase's one real trap, and it is the shape §2.8, §2.11 and
  §2.13 each record: silent, plausible, and invisible to a gate that checks the
  pure function alone. At the call site the only image in hand is
  `Session`'s decoded buffer, so `centre_offset((img.width(), img.height()), …)`
  is what an implementer reaches for — and since almost every real image is
  larger than its pane, the footprint exceeds the pane, the offset saturates to
  `(0, 0)`, and **every picture stays in the corner while all four of gate item
  1's assertions pass green**. `pane_offset` exists to make that unreachable
  rather than merely warned about: it takes the buffer, runs `pane_viewport` and
  `fit` exactly as `pane_sequence` does, and hands `centre_offset` the fitted
  pair. Gate item 3 asserts it on a buffer whose native and fitted sizes differ,
  which is the only assertion that can tell the two implementations apart.

  Two mechanical points:

  - **The image's footprint in cells is computed by rounding *up*.** A 427px-tall
    image in 36px cells occupies 12 rows, not 11.86. What forces the ceiling is
    **gate item 1's `(19, 9)`**, not a spill: a 24×24 image is 0.67 rows, which
    the floor calls 0 and centres as though it occupied nothing, giving `(19,
    10)` and sitting a row low. *(An earlier draft justified this as spill
    prevention. That was wrong and is corrected rather than quietly dropped: the
    round-1 reviewer searched every `cell` in 1..39 against every `pane` in 1..59
    and found **zero** cases where the floor pushes an image past its pane,
    because `fit` already guarantees `ceil(h/cell) ≤ pane`. Ceiling is still the
    conservative rule; the reason it is required is smaller and more exact than
    the reason first given.)*
  - **The offset is saturating.** An image whose footprint exceeds its pane
    yields `(0, 0)` rather than an underflow. Reached through `pane_offset` that
    is unreachable, since `fit` clamps first — but `centre_offset` is public and
    pure, and "should be unreachable" is how the zero-axis path in Phase 4's gate
    item 2 got in.

  `src/tui.rs:blank` keeps clearing the **whole** image area rather than the
  centred rectangle: what has to be erased is wherever the *previous* image was,
  and after this change that is no longer the pane's corner.

- **Exit gate:** six items runnable by `cargo test` with no terminal, and one a
  human checks. The blast radius is one dispatch arm, one flag and one offset, so
  the regression item is the whole suite rather than a reading of it.

  1. **`centre_offset` reproduces part 2's arithmetic.** In a 40×20-cell pane at
     16×36: a 640×427 image → `(0, 4)` — it fills the width, and 427px is 12 rows
     of 36, leaving 8 to split. A 24×24 image → `(19, 9)`: two cells wide and one
     tall, in the same pane. An image exactly filling the pane (640×720) → `(0,
     0)`. An image *larger* than its pane (800×900) → `(0, 0)` by saturation, not
     an underflow.
  2. **The rounding is up, asserted where the two rules disagree.** A 640×433
     image in the same pane → **`(0, 3)`**, *not* `(0, 4)`: 433/36 is 12.03, so
     the footprint is 13 rows, the free space 7, and its half 3. The floor rule
     would say 12 rows, 8 free, offset 4 — so this literal is the one place the
     two rules produce different answers on the same input, which is what makes
     it worth an assertion. Item 1's `(19, 9)` is the other half of the pin:
     the floor gives `(19, 10)` there. *(Both literals were re-derived at round 1
     under both rules; an earlier draft had this item asserting the floor's
     answer while its own prose derived the ceiling's.)*
  3. **`pane_offset` fits before it centres.** A **1200×800** buffer in a
     40×20-cell pane at 16×36 → `(0, 4)`: `fit` returns 640×427 for that pane,
     and 427px is 12 rows. Passing the buffer's native size instead returns
     `(0, 0)`, so this single assertion separates the correct implementation from
     the one part 2 says an implementer reaches for — and it is the only item
     that can, since item 1 exercises `centre_offset` on pairs that are already
     fitted.
  4. **`--browse` parses, and parses beside everything Phase 4 pinned.**
     `["tikray", "--browse", "a.png"]` → `browse` set with the path, no
     subcommand; `["tikray", "--browse"]` → `browse` set with no path, which is
     decision 3's "the bare `tikray`, not an error"; and
     `["tikray", "view", "--browse", "a.png"]` → **an error**, since `--browse`
     is not `view`'s flag and a surface selector on the invocation that already
     names its surface means nothing.
  5. **The dispatch itself is asserted, headlessly, through the binary.** The
     phase's headline change otherwise rests entirely on a human item, and it
     does not have to: the three branches give **three different refusals** with
     no terminal, which is exactly what makes them distinguishable to a test.
     Following `tests/gate.rs:run_without_iterm2`, with stdout on a pipe and both
     terminal variables cleared — `tikray <a PNG fixture>` exits non-zero
     reporting **not a tty** (the inline surface's refusal, §2.7, which is also
     item-1-of-decision-1's evidence that the branch runs detection at all);
     `tikray <a directory>` exits non-zero reporting **no screen** (the browser's
     refusal); and `tikray <a missing path>` reports **neither**, failing at the
     stat before either surface starts. Three assertions, no terminal, and they
     fail if the dispatch is wired backwards.
  6. **Phases 1–4's gates still pass, unmodified.** All 52 assertions — 16, 11,
     12 and 13 — green with no edits to any of the four files. **Item 3 of
     Phase 4's gate is the load-bearing one**: it asserts what `["tikray",
     "a.png"]` *parses* to, this phase changes what that value *dispatches* to,
     and the two must not be confused. If this item cannot pass, the parse and
     the dispatch have been conflated.

     One shipped test name goes stale and is **left that way**:
     `gate3_a_bare_path_is_the_tui_starting_there` describes a dispatch this
     phase reverses, though every assertion under it still holds because they are
     about parsing. Renaming it would be an edit to a shipped gate file, which
     this item forbids for a better reason than tidiness. Item 5 is where the new
     truth is asserted under a name that means it.
  7. **Human, in iTerm2:** `tikray <file>` draws the image inline and returns to
     the prompt; `tikray <dir>` and bare `tikray` open the browser;
     `tikray --browse <file>` opens the browser at that file's directory **with
     the file highlighted**, which is the assertion that the restored affordance
     is the same one and not a near miss; `tikray view <file>` is unchanged. In
     the browser, **the image sits in the middle of its pane — horizontally and
     vertically — and still does not cross the border at any window size**,
     including while resizing. The border half is Phase 4's item 6 restated,
     because this phase moves the very arithmetic that item was protecting, and
     a centred image that spills is a worse regression than an uncentred one.

  Tests land in `tests/gate_phase6.rs`; the four existing gate files are not
  edited, which is item 6. Item 7 **amends** `scripts/gate-phase4.sh` rather than
  adding a script beside it, and the distinction that makes this legal is worth
  stating: an assertion file is *evidence* and a gate script is a *procedure*.
  Phase 4's evidence — `tests/gate_phase4.rs` and its exit-gate entry in the
  review record — is untouched and stays true. Its script asks a question whose
  right answer this phase changes ("did it open the browser on samples/ with
  landscape.svg highlighted?"), and a procedure that tracks the code is the same
  contract `rules/` files carry. A script left stale would fail for the wrong
  reason, which is worse than one that was amended in the open.

- **Close-out:** four documents, and two of them are the kind a close-out
  usually misses:

  | Artifact | Why it changes |
  |---|---|
  | `rules/tui.md` | the dispatch table, `--browse`, the centring arithmetic. At **74/75 lines**, so `max_lines` rises in the same edit — Phase 4's exit-gate record already names it the first thing to trim |
  | `rules/core-pipeline.md` | it declares `src/cli.rs` in `sources` and states that `src/main.rs:run` dispatches `src/cli.rs:Command` for the one-shot surfaces and `src/tui.rs:run` for the browser — **false** once a bare file reaches the inline surface with no `Command` at all |
  | `rules/convert.md` | its note that `run` dispatches three surfaces; it dispatches four |
  | `README.md` | the Usage block and the Browsing section, both of which state the old grammar |

  `src/cli.rs`'s **module doc** is part of the scope rather than the close-out,
  because leaving it would ship a file arguing against what it implements — but
  it is named in both places on purpose: `/sync-rules` reads that file to
  regenerate `rules/core-pipeline.md`, so a stale doc comment there propagates
  into a rule on the next pass.

  **`CLAUDE.md` needs no change**; it names no invocation.

<!--
The review record is a sibling file, not a section: it lives at
specs/reviews/tkr-001.md, append-only, one heading per round. See §7.
-->
