---
title: tui
sources:
  - src/tui.rs
  - src/cli.rs
  - src/main.rs
covers: >
  the pane the image is drawn behind and why it survives a repaint, the cell
  arithmetic that sizes and centres it, what the list shows and what it hides,
  the four things that decide there is no preview, the draw-then-place ordering,
  which surface each invocation reaches, and the two interruptions that need
  different code
max_lines: 140
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
one explanation line in a row above it. That separation lets `src/tui.rs:blank`
paint the rectangle directly: ratatui cannot erase an image it does not know is
there, and blanking a row it had just drawn into would desync its buffer.
`src/display.rs:display` is unusable here — it sizes to the whole window and
indents for a prompt that is not there — so this module calls
`src/display.rs:sequence`.

## Sizing and centring a pane

`src/term.rs:geometry` reads pixels and cells from **one** `window_size()` call —
two reads can straddle a resize and yield a cell size that never existed — and
`src/term.rs:cell_size` divides them: 2528×1584 over 158×44 is 16×36 per cell,
measured, truncating, since an overestimate spills. `src/tui.rs:pane_viewport`
multiplies back up, and `src/tui.rs:pane_sequence` returns `Ok(None)` — *draw the
explanation, emit nothing* — whenever no image may be drawn. `None` is not
`auto`, which in a pane is exactly the spill.

`src/tui.rs:centre_offset` halves the free cells and `src/tui.rs:pane_offset` is
the glue that runs `src/display.rs:fit` first. **The split is the whole point:**
the argument is the size the terminal is *told*, never the buffer's native size —
which is what the call site has in hand, so the wrong call is the one an
implementer reaches for, and it fails silently. Almost every real image is larger
than its pane, so a native pair saturates the offset to `(0, 0)` and every
picture sits in the corner with every assertion on `centre_offset` still green.
`tests/gate_phase6.rs` pins it on a 1200×800 buffer, whose fitted `(0, 4)` and
native `(0, 0)` differ. The footprint rounds **up** — not against a spill, which
`fit` already prevents, but because a 24×24 image is 0.67 of a row and the floor
calls that zero and centres it a row low.

## What the list shows

`src/tui.rs:entries` lists directories first, then files, each by name. Files are
kept only when `src/load.rs:previewable` accepts their first
`src/load.rs:SVG_SNIFF_LIMIT` bytes — **by content, never by extension**, which
costs an open and a 1024-byte read per entry per directory change.

Extension filtering is refused for a reason a gate holds: `tests/gate.rs` has
asserted since Phase 1 that a PNG named `.txt` loads, so an extension filter
would hide exactly that file while `load` still drew it, leaving the list
disagreeing with the loader. `tests/gate_phase7.rs` pins the same fixture from
the listing side.

`previewable` is the **allowlist**, not `src/load.rs:detect` alone — which admits
GIF, BMP, TIFF, ICO, QOI and WebP, all of which `load` then refuses by name. A
list offering files that cannot be drawn is worse than no list, because the
refusal arrives after the choice. It is still only a byte rule: an SVG `usvg`
rejects is listed and then fails to parse, which is §2.10's final-arbiter rule
seen from this side.

`a` toggles the filter off, the count of held-back rows sits in the footer beside
that key — not in the list's border title, which `src/tui.rs:compact` already
shows is too tight to trust — and the toggle **persists across directory
changes**, since one that reset on entering a directory would need re-applying at
the moment the user is searching. The highlight follows by name through
`src/tui.rs:index_of`, never by index: toggling changes how many rows precede it.
An unreadable entry is not previewable, so it is hidden rather than raised.

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

`src/tui.rs:Session` owns the ordering and the invalidation rule, both outside
ratatui's model: the image rectangle comes **out of** the draw closure rather
than being computed beside it, since bytes sized to a pane the frame did not draw
are how an image spills; `placed` records what is on screen, so an unchanged
frame writes nothing and a changed one blanks the old rectangle first; and a
resize drops that record by hand, because ratatui repaints its whole buffer and
destroys the image without touching it. The bytes go to the same stdout ratatui
writes to, after `draw` returns. `placed` holds the pane **and** the offset — the
offset is where the bytes go, the pane is what `blank` must erase.

## Which surface an invocation reaches

`src/cli.rs:Cli` is an optional `src/cli.rs:Command`, an optional path and
`--browse`; a bare first argument is a subcommand only when it **exactly**
matches one, so `tikray view` is a missing-argument error and `./view` is the
escape hatch. It lives in the library so the grammar can be asserted without
launching a TUI. `src/main.rs:run` then dispatches on what the path **is**:

| Invocation | Surface |
|---|---|
| `tikray` | browser, working directory |
| `tikray <dir>` | browser, there |
| `tikray <file>` | inline — `src/main.rs:view`, `force` off |
| `tikray --browse <file>` | browser, that file highlighted |

**The stat is the dispatch**, so a missing path is `Io` before either surface
starts. Routing a file through `src/main.rs:view` rather than a second inline
path keeps `src/term.rs:detect_iterm2` on the branch; without it
`tikray x.png > out.txt` fills a file with escape bytes. `src/tui.rs:Browser`
stats again for its own start state, deliberately. `--force` modifies a surface
so it stays on `view`; `--browse` selects one.

## Two interruptions

Quitting and any `?` are covered by `Session`'s `Drop`, a panic by the hook
`ratatui::try_init` installs. Neither covers a signal: raw mode clears `ISIG`, so
Ctrl-C arrives as a key event, while a real `kill -INT` runs no `Drop` at all.
`src/tui.rs:Interrupt` registers a flag for that case, and the loop polls for it.
