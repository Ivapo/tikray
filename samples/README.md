# Samples

Something to point tikray at. Each one is here because it shows a *different*
thing about how tikray behaves — none of them is decoration.

The raster files were produced by tikray itself, from the SVG next to them, so
what you are looking at is what the tool actually writes:

```sh
tikray convert samples/landscape.svg samples/landscape.png
tikray convert samples/landscape.svg samples/landscape.jpg
tikray convert samples/translucent.svg samples/translucent.png
```

| File | What it shows |
|---|---|
| `landscape.svg` | Vector at document scale (1200×800), with gradients. Rasterized at its own size, then treated exactly like any raster. |
| `landscape.png` | The same scene, lossless. Compare it against the `.jpg`. |
| `landscape.jpg` | The same scene at JPEG's default quality — the gradients in the sky are where you will see the difference first. |
| `icon.svg` | A 24×24 viewBox and no `width`/`height`. **Draws as a 24-pixel speck**, not scaled up to fill the window. |
| `translucent.svg` | Overlapping translucent shapes on a fully transparent page. |
| `translucent.png` | Converted, **alpha preserved** — over a dark terminal it shows through. |

## Try them

```sh
tikray samples/landscape.png     # a raster, scaled down to fit your window
tikray samples/landscape.svg     # the vector source, same picture
tikray samples/icon.svg          # tiny, and stays tiny
tikray samples/                  # all of them, in the browser
```

**The two worth doing side by side**, because each is a rule you would otherwise
have to read about:

```sh
# Never upscale. A 24x24 icon is drawn at 24x24, in a window some hundreds of
# pixels wide. Consistency with raster, and the one place vector buys nothing.
tikray samples/icon.svg

# Alpha cannot survive JPEG, so it is composited onto white -- and tikray says
# so on stderr rather than doing it silently. Convert the same source both ways
# and open the pair: the PNG shows through, the JPEG is on white.
tikray convert samples/translucent.svg /tmp/t.png
tikray convert samples/translucent.svg /tmp/t.jpg
```

That second pair is worth the thirty seconds. The library default underneath
tikray does not composite and does not refuse — it *drops* alpha, which puts a
transparent image on a **black** background with no error at all. The white you
get instead is a decision, not a default.

## Two refusals you can reproduce here

```sh
tikray convert samples/landscape.png out.svg   # SVG is input-only
tikray convert samples/landscape.png out.gif   # GIF is not a gated output
```

Both name what went wrong. Support is an explicit list rather than whatever the
dependencies happen to link in, so a format tikray *can* technically write but
has not gated is refused by name instead of quietly working.
