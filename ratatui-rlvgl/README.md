# ratatui-rlvgl

`ratatui-rlvgl` connects Ratatui's retained cell model to an rlvgl widget
tree. It provides a `no_std + alloc` Ratatui backend and an rlvgl widget that
paints the backend's shared cell surface through any rlvgl renderer.

```rust,ignore
let (backend, surface) = RlvglBackend::new(80, 24, CellMetrics::font_6x10())?;
let terminal = ratatui_core::terminal::Terminal::new(backend)?;
let view = RatatuiView::new(bounds, surface);
```

The backend performs no device I/O. A Ratatui terminal writes cell updates to
the shared surface; `RatatuiView` draws that surface when it participates in
an rlvgl widget tree. The view can be the main content widget or a child pane
inside a native graphical screen.

`RatatuiView` defaults to the RATATUI-00 curated-Unicode, anti-aliased
`PackedFont` terminal family (`ratatui_rlvgl::fonts`): four variants
(regular/bold/oblique/bold-oblique) packed from `DejaVu Sans Mono`, covering
ASCII, the full box-drawing block (U+2500–U+257F), block elements
(U+2580–U+259F), the full arrow block (U+2190–U+21FF), and a small
status-symbol set (`●○■□✓✗▪▸`). `Modifier::BOLD` and `Modifier::ITALIC`
select a real font-variant outline instead of a synthetic redraw or an
upright no-op. `Modifier::SLOW_BLINK`/`Modifier::RAPID_BLINK` are driven by
an explicit, caller-triggered `RatatuiSurface::advance_blink_phase()` call —
never a wall clock — which suppresses the glyph (background only) on the
"off" phase; wire it to whatever tick source the host application already
owns. Glyphs outside the curated repertoire (Braille, CJK, and everything
else not enumerated above) still degrade through the existing ASCII
substitution (`+`, `-`, `|`) or `?` fallback. Ratatui named colors and
indexed colors use the same xterm 256-color palette; RGB values pass through
unchanged. Reversed, hidden, underlined, crossed-out, and dim modifiers have
static graphical mappings and compose with whichever font variant is
selected.

Call `RatatuiView::set_font_family(RatatuiTerminalFont::Bitmap6x10)` to opt
back into rlvgl's deterministic 1-bit `FONT_6X10` rasterizer and a fixed
12×20-pixel grid (`CellMetrics::font_6x10()`) — the lowest-flash-cost
profile, and a regression-proof baseline: in this mode every box-drawing
glyph still degrades to ASCII, bold is still a redrawn-one-pixel-right
approximation, and italic/blink still render as static, non-animated
upright equivalents, matching the pre-RATATUI-00 behavior byte-for-byte.
Pair the default `Packed` family with `CellMetrics::packed_terminal()`
(14×21); pair `Bitmap6x10` with `CellMetrics::font_6x10()` (12×20).

The backend targets fullscreen and fixed viewports only —
`Viewport::Inline` and the `scrolling-regions` feature's scroll-region
methods are an intentional non-goal, not a gap: an embedded display has no
"scrollback above the viewport" surface for inline mode to be correct about.
See `RATATUI-00-CONCEPTS.md` §9 for the ratified rationale.
