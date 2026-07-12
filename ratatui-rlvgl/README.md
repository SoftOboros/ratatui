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

Version 0.1 uses rlvgl's deterministic `FONT_6X10` rasterizer and a fixed
6×10-pixel grid. Ratatui named colors and indexed colors use the same xterm
256-color palette; RGB values pass through unchanged. Reversed, hidden,
underlined, crossed-out, bold, and dim modifiers have static graphical
mappings. Italic and blink modifiers are accepted but intentionally render as
their non-animated upright equivalents. Unicode box-drawing glyphs degrade to
ASCII `+`, `-`, and `|` with `FONT_6X10`; other unsupported non-ASCII glyphs
render as `?`. The backend targets fullscreen and fixed viewports; terminal
scrollback and inline `append_lines` behavior are outside the initial bridge.
