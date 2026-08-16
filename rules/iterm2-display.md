---
title: iterm2-display
sources:
  - src/display.rs
  - src/term.rs
covers: >
  the OSC 1337 argument string, the fit-down-never-up sizing arithmetic, the
  viewport query, and iTerm2 detection with its --force override
max_lines: 50
generated: 2026-08-16
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

## Viewport and detection

`src/term.rs:viewport` reads `crossterm::terminal::window_size()` and treats an
error, or a `0` in either pixel axis, as unreported — what crossterm documents
rather than a hedge: the pixel fields may default to 0, unix documents them as
unused, and Windows does not implement them.
`src/term.rs:detect_iterm2` requires both a tty on stdout and one of
`TERM_PROGRAM=iTerm.app` or `LC_TERMINAL=iTerm2`; the two survive different
things (the latter an ssh hop, neither plain tmux). `--force` skips both, because
a detection rule with no override turns a known false negative into an unusable
tool. `src/display.rs:display` is the only thing that writes to the stream.
