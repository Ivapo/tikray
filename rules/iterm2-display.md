---
title: iterm2-display
sources:
  - src/display.rs
  - src/term.rs
covers: >
  the OSC 1337 argument string, the fit-down-never-up sizing arithmetic and the
  one door out of it, the two-cell indent and where it comes out of, the
  viewport and cell-geometry queries, and iTerm2 detection with its --force
  override
max_lines: 110
generated: 2026-08-17
---

# iTerm2 display

The protocol carries a **complete encoded image file**, not raw pixels, so the
terminal renders it. There is no cell quantization, dithering or palette
approximation anywhere in Tikray — why the scope is affordable, and why it is
iTerm2-only.

## The sequence

```
ESC ] 1337 ; File=inline=1;width=<W>px;height=<H>px;preserveAspectRatio=1 : <base64 PNG> BEL
```

`src/display.rs:sequence` builds this and is pure — buffer plus viewport in,
bytes out — which is the seam that makes the display path testable with no
terminal. `inline=1` is mandatory and is the whole difference between a drawn
image and nothing: it defaults to `0`, and iTerm2 then downloads the file with no
visual representation in the session — well-formed, exit 0, nothing shown.
`preserveAspectRatio=1` also defaults to `1`, so it is belt-and-braces; `fit`'s
arithmetic is what preserves the ratio. The payload is encoded at **native**
size, with the computed dimensions as arguments — Tikray never resamples.

## Sizing: fit down, never up

`src/display.rs:fit` is the whole policy, a pure function of four integers:

```
scale = min(W / w, H / h, 1.0)
out   = (max(1, round(w * scale)), max(1, round(h * scale)))
```

The `1.0` clamp is the never-upscale rule — computed rather than delegated to
`width=100%;height=100%` because *filling is not fitting*, and a 16×16 favicon
fills a window as a screenful of blurred squares. The `max(1, …)` floor is not
defensive: `(10,3)` into `(1,100)` computes `round(0.3) = 0`, and a zero
dimension is not legal. `None` in means the viewport was unreported; `None` out
means emit `width=auto;height=auto`, which never upscales either.

**The `1.0` clamp is a default, not an invariant.** `src/display.rs:scale` is
that arithmetic exposed as a raw factor, so it has one implementation, and
`src/display.rs:sequence_at` emits an exact size **without** consulting `fit` —
the door zoom opens (`rules/tui.md`). Everything automatic still clamps; nothing
upscales unless a person pressed a key, and no upscaled pixel is ever written to
a file, because `src/tui.rs:convert_to` re-loads from disk. `sequence_at` shares
`sequence`'s argument string exactly, which is what lets a zoomed emission be
compared against an unzoomed one.

## The indent comes out of the viewport

`src/display.rs:indented` puts `src/display.rs:INDENT` — **2** — spaces in front
of the sequence, and `src/display.rs:indent` takes those two cells' worth of
pixels **out of the viewport** before `fit` sees it. They are one decision and
one function returns both: spaces without the shrink push a window-width image
past the right edge, where iTerm2 wraps or scrolls it, and the shrink without the
spaces silently narrows the image for no visible reason.

Two conditions drop it, and in both the picture wins: no cell size (the indent is
in cells, the viewport in pixels, so there is no conversion) and a viewport too
narrow to spare it. **A piped stdout is never indented** — `--force` exists so
the *sequence* can be captured to a file, and two spaces prepended to that byte
stream are corruption. The predicate is `std::io::stdout().is_terminal()`, not a
bound on `display`'s `out`, which need not be stdout. Mechanically the test
yields `cell: None`, so there is no second code path.

That last rule is load-bearing beyond taste: `tests/gate.rs:gate4_force_emits_anyway`
asserts the binary's piped stdout starts at `\x1b]1337;File=`, and an
unconditional indent breaks it **only where `window_size()` resolves** — from a
real terminal and not from CI. `tests/gate_phase7.rs` restates it so the coupling
is visible where it was made.

## Viewport, cell geometry, and detection

`src/term.rs:viewport` reads `crossterm::terminal::window_size()` and treats an
error, or a `0` in either pixel axis, as unreported — what crossterm documents
rather than a hedge: the pixel fields may default to 0, unix documents them as
unused, and Windows does not implement them.

`src/term.rs:geometry` is the same query for a caller that needs **cells** as
well, and returns both pairs from one call because two reads can straddle a
resize. `src/term.rs:cell_size` divides them, and is pure for the reason `fit`
is: the four integers are injected, so the gates run with no terminal. A zero in
any of them, or a quotient that truncates to zero, is the same "unreported" the
viewport rule uses.

`src/display.rs:display` reads `geometry` **once** and derives both the viewport
and the cell size from that one pair, rather than calling `viewport` beside it:
two reads can straddle a resize, which is the reason `geometry` returns both at
all. `viewport`'s zero-axis rule is reapplied by hand at that call site.
`src/term.rs:detect_iterm2` requires both a tty on stdout and one of
`TERM_PROGRAM=iTerm.app` or `LC_TERMINAL=iTerm2`; the two survive different
things (the latter an ssh hop, neither plain tmux). `--force` skips both, because
a detection rule with no override turns a known false negative into an unusable
tool. `src/display.rs:display` is the only thing that writes to the stream.
