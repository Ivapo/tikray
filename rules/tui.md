---
title: tui
sources:
  - src/tui.rs
  - src/cli.rs
  - src/main.rs
covers: >
  the pane the image is drawn behind and why it survives a repaint, the cell
  arithmetic that sizes it, the four things that decide there is no preview, the
  draw-then-place ordering, and the two interruptions that need different code
max_lines: 75
generated: 2026-08-17
---

# TUI

`src/tui.rs:run` is the second caller of the core, reaching the same buffer
through the same `src/load.rs:load`. Only the last step differs: a pane, not a
window.

## The image is not a widget

Ratatui diffs and writes **cells**, and has no notion of a region it must leave
alone. The image survives because the cells under it stay blank between frames,
so the diff never mentions them — an image behind a hole in the layout. It dies
when a widget claims those cells, and when too big for its region it does not
clip, it **spills**.

So `src/tui.rs:Browser`'s render leaves the image rectangle empty and puts its
one explanation line in a row above it. That separation is what lets
`src/tui.rs:blank` paint the rectangle directly: ratatui cannot erase an image
it does not know is there, and blanking a row it had just drawn into would
desync its buffer. `src/display.rs:display` is **not** used here and
`src/display.rs:sequence` is: `display` reads the whole-window viewport from
`src/term.rs:viewport`, and a pane is not the window.

## Sizing a pane

`src/term.rs:geometry` reads pixels and cells from **one** `window_size()` call
— two reads can straddle a resize and yield a cell size that never existed —
and `src/term.rs:cell_size` divides them: 2528×1584 over 158×44 is 16×36 per
cell, measured. The division truncates, the safe direction, since an
overestimate spills. `src/tui.rs:pane_viewport` multiplies back up, and
`src/tui.rs:pane_sequence` returns `Ok(None)` — *draw the explanation, emit
nothing* — whenever no image may be drawn. `None` is not `auto`, which in a
pane is exactly the spill.

## Four ways there is no preview, and one refusal

| Condition | What happens |
|---|---|
| `src/term.rs:cell_size` is `None` | browser runs, one line in the pane |
| not iTerm2 (`src/term.rs:detect_iterm2`) | browser runs, one line in the pane |
| a directory, or a file `load` refuses | browser runs, the reason in the pane |
| either pane axis is zero | nothing emitted, silently |
| **stdout is not a terminal** | **refused**, `TikrayError::NoScreen` |

Only the last is a refusal, and it names `view` and `convert` rather than
`--force`, which is `view`'s escape hatch and buys a browser nothing. A `load`
refusal is a line in the pane, never the end of the session.

## Draw, then place

`src/tui.rs:Session` owns the ordering and the invalidation rule, both of which
live outside ratatui's model: the image rectangle comes **out of** the draw
closure rather than being computed beside it, since bytes sized to a pane the
frame did not draw are how an image spills; `placed` records what is on screen,
so an unchanged frame writes nothing and a changed one blanks the old rectangle
first; and a resize drops that record by hand, because ratatui repaints its
whole buffer and destroys the image without touching it. The bytes go to the
same stdout ratatui writes to, after `draw` returns, so its flush has landed.

## The grammar, and two interruptions

`src/cli.rs:Cli` is an optional `src/cli.rs:Command` plus an optional path, and
`src/main.rs:run` sends a missing subcommand to `src/tui.rs:run`. A bare first
argument is a subcommand when it **exactly** matches one and a path otherwise,
so `tikray view` is a missing-argument error and `./view` is the escape hatch
for the file. It lives in the library so the grammar can be asserted without
launching a TUI. `tikray <path>` opens the **browser at that path's directory
with it highlighted**, not a single-image view — that is `view`'s job.

Quitting and any `?` are covered by `Session`'s `Drop`, a panic by the hook
`ratatui::try_init` installs. Neither covers a signal: raw mode clears `ISIG`,
so Ctrl-C arrives as a key event, while a real `kill -INT` default-terminates
without unwinding and runs no `Drop`. `src/tui.rs:Interrupt` registers a flag
for that one case, and the loop polls so it is seen.
