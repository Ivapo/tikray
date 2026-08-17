---
id: tkr-001
title: tikray
status: accepted
last_updated: 2026-08-16
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
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 4 — the TUI shell over the same core"
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
| `tikray convert <in> <out>` | no display; writes a file | 3 |
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

## 3. Open questions

- **OQ-1** — Can OSC 1337 inline images coexist with a Ratatui full-screen
  alternate-screen TUI, or does the image have to be drawn outside Ratatui's
  buffer model (which repaints cells and would erase or clip it)? If they
  cannot, Phase 4's shape changes: either a split where Ratatui browses and the
  image is drawn on suspend, or no alternate screen at all.
  *(deferred by evidence — settle it with a spike at the top of Phase 4, not by
  argument now)* **§2.9's grammar rides on this**: `tikray <path>` is defined as
  the TUI, so if Phase 4 is cut or re-shaped the bare-path form falls back to
  `view`'s inline behaviour rather than being left undefined.
- **OQ-2** — What does "convert to SVG" mean? The seed document promises "any of
  the above into any of the others", but raster→SVG is not a format change: it
  is either vectorization/tracing (a different and much larger project) or
  wrapping the raster in an SVG container (technically an `.svg` file, arguably
  a lie). Options: refuse it with a clear error, wrap-and-document, or drop SVG
  from the output set entirely. *(needs-input — this narrows an advertised
  capability, so it is not a design call to make silently)*
- **OQ-3** — ~~Display sizing policy. Terminal cells are not pixels, and an
  image larger than the window has to be fitted. What is the default
  (fit-to-width, fit-to-window, native size with scroll), and does the user
  override it?~~ **RESOLVED — §2.6.** Fit down, never up: `scale = min(W/w,
  H/h, 1.0)`, emitted in `px`. The premise above was half wrong and is left
  visible rather than edited away — Tikray does no cell/pixel conversion at all,
  because `width`/`height` accept a `px` form and the viewport is read in pixels
  from `window_size()`. No user override in v1; the sizing rule is not a flag.
- **OQ-4** — Alpha and quality on export. JPEG has no alpha channel, so
  RGBA→JPEG must composite against something (white? black? a flag?) or refuse.
  Likewise JPEG quality and PNG compression level: defaults, or exposed?
  *(design call — Phase 3)*
- **OQ-5** — Does `view` need to survive tmux and ssh? tmux requires escape
  passthrough and iTerm2's protocol is commonly broken by it. Supporting it is
  small if designed in and awkward if retrofitted. *(needs-input — it is a
  question about how the user actually works)* **Blocks no phase; §2.7's
  `--force` is the stopgap.** It decides whether a later phase owns tmux
  passthrough, and it is the question that would reopen §2.7's rejection of
  Feature Reporting — so if the answer is "yes, tmux daily", that decision gets
  re-argued before Phase 4 rather than after.
- **OQ-6** — Multi-frame inputs (animated GIF, multi-page TIFF). §1.1 declares
  first-frame-only; the open part is whether the tool must *say* so, and where.
  *(design call — **Phase 3**, which is the first phase whose allowlist can
  admit one.)* §2.8's Phase 1 allowlist keeps GIF and TIFF out entirely, so this
  cannot fire there. The one residual case is APNG, which sniffs as PNG and so
  passes the allowlist: what `image`'s default decode path yields for one — first
  frame, default image, or an error — is unverified, and Phase 1 neither claims
  nor gates it.
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

- **Scope:** `tikray convert <in> <out>`, output format inferred from the
  destination extension with an explicit override flag. Encodes the same buffer
  Phases 1–2 fill, so every input type already supported is convertible on
  arrival. Resolves OQ-2 (SVG as an output target) and OQ-4 (alpha flattening
  and quality defaults) — both must be *answered in the spec* before this phase
  is cleared, not decided at the keyboard.
- **Exit gate:** for each supported (input, output) pair, converting produces a
  file that a second decoder reads back with the expected dimensions and
  format, and that `tikray view` displays as the same picture. Attempting an
  unsupported pair — whatever OQ-2 settles SVG-out to be — fails with the
  documented message rather than writing a broken file. Refusing to overwrite
  an existing output without a flag is part of the gate.
- **Close-out:** adds `rules/convert.md`; updates `rules/core-pipeline.md` with
  the encode edge and the format matrix.

### Phase 4 — the TUI shell over the same core
*Produces the observable: yes — images drawn inside a browsing UI. If OQ-1's
spike shows the protocol and Ratatui cannot share a screen, this phase is
re-specced or cut before it is built; it is last precisely so that answer costs
nothing already shipped.*

- **Scope:** a Ratatui + crossterm file browser that opens what it lands on
  through the Phase 1–3 core, plus a key to convert the selected file. Begins
  with the OQ-1 spike; its result is recorded in §2 as a decision before any UI
  work. **Owns both TUI entry points — bare `tikray` and `tikray <path>`
  (§2.9)** — including which of the two readings a `<file>` argument takes,
  which §2.9 reserves rather than settles.
- **Exit gate:** launching `tikray` with no arguments opens the browser,
  arrow-key navigation shows the highlighted image, and quitting restores the
  terminal to a clean state (no residual alternate screen, no swallowed cursor,
  no leaked escape state) after both a normal quit and a `SIGINT`.
  `tikray <path>` opens the same browser positioned at that path, and
  `tikray view <path>` still draws inline and exits — the two surfaces stay
  distinguishable, which is the whole of §2.9.
- **Close-out:** adds `rules/tui.md`; updates the `CLAUDE.md` stanza if the
  entry point changes.

<!--
The review record is a sibling file, not a section: it lives at
specs/reviews/tkr-001.md, append-only, one heading per round. See §7.
-->
