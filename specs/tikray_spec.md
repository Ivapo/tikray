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
    reviewed: null
    shipped: null
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
- **`resvg`/`usvg`** rasterize SVG in pure Rust, so there is no C toolchain
  dependency and the binary stays a single artifact. This is the deciding
  reason over librsvg/Cairo. Version pinned at Phase 2, which is the first
  phase that compiles it.
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
emitted `width=64px;height=48px`. So the `px` branch is the primary path, and
Phase 2 has a viewport to rasterize an SVG against. The paragraph above is kept
because its *reasoning* still holds — the `auto` branch stays reachable and is
exercised by every run without a controlling terminal, including `cargo test`'s
— but a Phase 2 implementer should not plan around it as the common case.

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

- **Scope:** `resvg`/`usvg` behind the same `load(path) -> DynamicImage` entry
  point Phase 1 defined (§2.1 — the waist is `DynamicImage`, not a fixed RGBA8
  raster), dispatching on detected input type rather than on file extension
  alone. Rasterization target size derives from the display
  sizing decision, since rasterizing an SVG at the wrong size is the one place
  where output quality actually depends on getting sizing right first.
- **Exit gate:** `tikray view <a.svg>` draws the SVG inline, sharp at the chosen
  display size (visibly not an upscaled small raster). An SVG that `usvg`
  rejects produces an actionable parse error, not a blank image.
- **Close-out:** updates `rules/core-pipeline.md` with the vector branch; adds
  `rules/svg-rasterization.md`.

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
