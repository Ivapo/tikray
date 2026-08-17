# Tikray

## Idea

An iTerm2-focused CLI/TUI application that both **views** and **converts**
raster and vector images — PNG, JPEG, and SVG at minimum. One tool instead of
reaching for a separate viewer (`chafa`, `timg`) and converter (ImageMagick).

- **View:** render PNG/JPEG/SVG inline in iTerm2.
- **Convert:** export/save any of the above as PNG or JPEG. (The seed said "any
  of the others"; SVG turned out to be input-only — writing one would mean
  tracing, which is a different tool.)

## Name

**Tikray** — Quechua for "to turn over / to translate." Chosen because it
already spans both meanings the app needs (flip/see an image, and
convert/translate it into another format), which is unusually apt for a
combined viewer+converter.

Availability check (2026-08-16):

| Registry | Status |
|---|---|
| crates.io | Free |
| npm | Free |
| Homebrew formula | Free |
| PyPI | Taken (unrelated Python data-transformation engine) |
| GitHub repo name | Taken at `PaoloDiazG/tikray` (unrelated, small project) — doesn't block a repo under our own account |
| GitHub username/org | `TikRay` exists — no clean top-level `github.com/tikray` org |

Crate name (the one that matters, since this ships via Rust/crates.io) is
clear.

## Tech stack

- **Language:** Rust
- **TUI framework:** [Ratatui](https://ratatui.rs/) + `crossterm`
- **Raster decode/encode:** `image` crate — handles PNG/JPEG (and other
  formats like WebP/GIF/BMP for free) without writing codecs by hand
- **SVG rendering:** `resvg`/`usvg` (pure Rust, no C dependency) to
  rasterize SVG into a pixel buffer, which then goes through the same
  `image` crate encode path as any other format
- **Display in iTerm2:** iTerm2's inline image protocol (OSC 1337,
  `File=...`) — base64-encode the image bytes and print the escape
  sequence; no terminal-graphics rendering logic needed for the "view" half

### Why Rust/Ratatui over alternatives

- Single dependency-free binary, and `image` + `resvg` cover decode/encode/
  SVG-rasterization without reinventing codecs
- Scope is intentionally iTerm2-only for v1 — supporting other terminals
  (Kitty graphics protocol, Sixel, ASCII-art fallback) is a much bigger
  undertaking and out of scope for now

## Rough scope (not yet started)

- [ ] Decode PNG/JPEG via `image`, rasterize SVG via `resvg`
- [ ] Display decoded image inline via iTerm2 OSC 1337 protocol
- [ ] Export/save any input format to PNG or JPEG
- [x] TUI shell (Ratatui) for browsing/opening files, not just a one-shot CLI
- [ ] CLI mode for scripting (`tikray view file.svg`, `tikray convert file.svg file.png`)

## Status

Phases 1-4 of `tkr-001` built: `tikray view` draws PNG, JPEG and SVG inline in
iTerm2, `tikray convert` writes any of them back out as PNG or JPEG, and a bare
`tikray` browses files with a live preview. Phase 5 — converting from inside
the browser — is specced and not built.
Per-phase state lives in `specs/INDEX.md`; current behaviour lives in `rules/`.
