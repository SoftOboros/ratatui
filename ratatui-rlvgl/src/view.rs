//! rlvgl widget view and input translation.

use alloc::boxed::Box;
use core::cell::RefCell;

use ratatui_core::style::Modifier;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::event::{Event, Key};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::widget::{Rect, Widget};
use unicode_width::UnicodeWidthStr;

use crate::backend::{CellMetrics, RatatuiSurface};
use crate::color::{dim, resolve};

type InputCallback = Box<dyn FnMut(RlvglInput)>;

#[derive(Clone, Copy)]
struct PaintContext {
    bounds: Rect,
    metrics: CellMetrics,
    defaults: crate::SurfaceDefaults,
}

/// Backend-neutral key vocabulary emitted by [`RatatuiView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    /// Escape.
    Escape,
    /// Enter/return.
    Enter,
    /// Space.
    Space,
    /// Backspace.
    Backspace,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Function key number.
    Function(u8),
    /// Printable character.
    Character(char),
    /// Unclassified platform key code.
    Other(u32),
}

/// Pointer transition translated from an rlvgl event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    /// Raw pointer down.
    Down,
    /// Raw pointer up.
    Up,
    /// Raw pointer movement.
    Move,
    /// Debounced press began.
    Press,
    /// Debounced press released.
    Release,
}

/// Input delivered by an [`RatatuiView`] to its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RlvglInput {
    /// Logical UI tick.
    Tick,
    /// Key press.
    KeyDown(InputKey),
    /// Key release.
    KeyUp(InputKey),
    /// Pointer activity inside the view.
    Pointer {
        /// Transition kind.
        kind: PointerKind,
        /// Local pixel coordinates relative to the view.
        pixel: (u16, u16),
        /// Derived Ratatui cell coordinates.
        cell: (u16, u16),
    },
}

/// rlvgl widget that paints a shared Ratatui cell surface.
pub struct RatatuiView {
    bounds: Rect,
    surface: RatatuiSurface,
    input: RefCell<Option<InputCallback>>,
}

impl RatatuiView {
    /// Create a view at `bounds` for `surface`.
    pub fn new(bounds: Rect, surface: RatatuiSurface) -> Self {
        Self {
            bounds,
            surface,
            input: RefCell::new(None),
        }
    }

    /// Replace the optional input callback.
    pub fn set_on_input(&self, callback: Option<InputCallback>) {
        *self.input.borrow_mut() = callback;
    }

    /// Access the shared surface.
    pub const fn surface(&self) -> &RatatuiSurface {
        &self.surface
    }

    /// Convert the surface's dirty-cell union to screen pixel coordinates.
    pub fn dirty_pixel_rect(&self) -> Option<Rect> {
        let metrics = self.surface.metrics();
        let cells = self.surface.dirty_cells()?;
        Some(Rect {
            x: self
                .bounds
                .x
                .saturating_add(i32::from(cells.x).saturating_mul(i32::from(metrics.width()))),
            y: self
                .bounds
                .y
                .saturating_add(i32::from(cells.y).saturating_mul(i32::from(metrics.height()))),
            width: i32::from(cells.width).saturating_mul(i32::from(metrics.width())),
            height: i32::from(cells.height).saturating_mul(i32::from(metrics.height())),
        })
    }

    fn emit(&self, input: RlvglInput) -> bool {
        let mut callback = self.input.borrow_mut();
        if let Some(callback) = callback.as_mut() {
            callback(input);
            true
        } else {
            false
        }
    }

    fn pointer(&self, kind: PointerKind, x: i32, y: i32) -> bool {
        let Some(local_x) = x.checked_sub(self.bounds.x) else {
            return false;
        };
        let Some(local_y) = y.checked_sub(self.bounds.y) else {
            return false;
        };
        if local_x < 0
            || local_y < 0
            || local_x >= self.bounds.width
            || local_y >= self.bounds.height
        {
            return false;
        }
        let metrics = self.surface.metrics();
        let local_x = local_x.min(i32::from(u16::MAX)) as u16;
        let local_y = local_y.min(i32::from(u16::MAX)) as u16;
        self.emit(RlvglInput::Pointer {
            kind,
            pixel: (local_x, local_y),
            cell: (local_x / metrics.width(), local_y / metrics.height()),
        })
    }
}

impl Widget for RatatuiView {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let state = self.surface.borrow();
        if !state.published {
            return;
        }
        let metrics = state.metrics;
        let context = PaintContext {
            bounds: self.bounds,
            metrics,
            defaults: state.defaults,
        };
        let mut clipped = ClipRenderer::new(renderer, self.bounds);
        let width = state.buffer.area.width;
        let height = state.buffer.area.height;
        let mut y = 0;
        while y < height {
            let mut x = 0;
            while x < width {
                let Some(cell) = state.buffer.cell((x, y)) else {
                    break;
                };
                let span = UnicodeWidthStr::width(cell.symbol())
                    .max(1)
                    .min(usize::from(width - x));
                draw_cell(&mut clipped, context, x, y, span as u16, cell);
                x = x.saturating_add(span as u16);
            }
            y += 1;
        }
        if state.cursor_visible && state.cursor.x < width && state.cursor.y < height {
            let x = self.bounds.x.saturating_add(
                i32::from(state.cursor.x).saturating_mul(i32::from(metrics.width())),
            );
            let y = self.bounds.y.saturating_add(
                i32::from(state.cursor.y).saturating_mul(i32::from(metrics.height())),
            );
            clipped.fill_rect(
                Rect {
                    x,
                    y: y.saturating_add(i32::from(metrics.height()))
                        .saturating_sub(2),
                    width: i32::from(metrics.width()),
                    height: 2,
                },
                state.defaults.foreground,
            );
        }
        drop(state);
        self.surface.mark_clean();
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Tick => self.emit(RlvglInput::Tick),
            Event::KeyDown { key } => self.emit(RlvglInput::KeyDown(map_key(key))),
            Event::KeyUp { key } => self.emit(RlvglInput::KeyUp(map_key(key))),
            Event::PointerDown { x, y } => self.pointer(PointerKind::Down, *x, *y),
            Event::PointerUp { x, y } => self.pointer(PointerKind::Up, *x, *y),
            Event::PointerMove { x, y } => self.pointer(PointerKind::Move, *x, *y),
            Event::PressDown { x, y } => self.pointer(PointerKind::Press, *x, *y),
            Event::PressRelease { x, y } => self.pointer(PointerKind::Release, *x, *y),
            _ => false,
        }
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

fn draw_cell(
    renderer: &mut dyn Renderer,
    context: PaintContext,
    x: u16,
    y: u16,
    span: u16,
    cell: &ratatui_core::buffer::Cell,
) {
    let PaintContext {
        bounds,
        metrics,
        defaults,
    } = context;
    let px = bounds
        .x
        .saturating_add(i32::from(x).saturating_mul(i32::from(metrics.width())));
    let py = bounds
        .y
        .saturating_add(i32::from(y).saturating_mul(i32::from(metrics.height())));
    let mut foreground = resolve(cell.fg, defaults.foreground);
    let mut background = resolve(cell.bg, defaults.background);
    if cell.modifier.contains(Modifier::REVERSED) {
        core::mem::swap(&mut foreground, &mut background);
    }
    if cell.modifier.contains(Modifier::DIM) {
        foreground = dim(foreground);
    }
    let cell_rect = Rect {
        x: px,
        y: py,
        width: i32::from(metrics.width()).saturating_mul(i32::from(span)),
        height: i32::from(metrics.height()),
    };
    let glyph_y = py
        .saturating_add(i32::from(metrics.baseline()))
        .saturating_sub(FONT_6X10.scaled_height());
    renderer.fill_rect(cell_rect, background);
    let mut clipped = ClipRenderer::new(renderer, cell_rect);
    if !cell.modifier.contains(Modifier::HIDDEN) {
        for (index, ch) in cell
            .symbol()
            .chars()
            .take(usize::from(span))
            .map(bitmap_font_fallback)
            .enumerate()
        {
            FONT_6X10.draw_char(
                &mut clipped,
                px.saturating_add((index as i32).saturating_mul(i32::from(metrics.width()))),
                glyph_y,
                ch,
                foreground,
            );
        }
        if cell.modifier.contains(Modifier::BOLD) {
            for (index, ch) in cell
                .symbol()
                .chars()
                .take(usize::from(span))
                .map(bitmap_font_fallback)
                .enumerate()
            {
                FONT_6X10.draw_char(
                    &mut clipped,
                    px.saturating_add(1)
                        .saturating_add((index as i32).saturating_mul(i32::from(metrics.width()))),
                    glyph_y,
                    ch,
                    foreground,
                );
            }
        }
    }
    if cell.modifier.contains(Modifier::UNDERLINED) {
        clipped.fill_rect(
            Rect {
                x: px,
                y: py
                    .saturating_add(i32::from(metrics.height()))
                    .saturating_sub(1),
                width: cell_rect.width,
                height: 1,
            },
            foreground,
        );
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        clipped.fill_rect(
            Rect {
                x: px,
                y: py.saturating_add(i32::from(metrics.height() / 2)),
                width: cell_rect.width,
                height: 1,
            },
            foreground,
        );
    }
}

/// Preserve Ratatui's box layout when the selected embedded bitmap font only
/// contains ASCII. Richer rlvgl fonts can replace this policy in a future
/// configurable cell renderer without changing the backend contract.
const fn bitmap_font_fallback(character: char) -> char {
    match character {
        '─' | '━' | '═' | '╌' | '╍' | '┄' | '┅' | '┈' | '┉' => '-',
        '│' | '┃' | '║' | '╎' | '╏' | '┆' | '┇' | '┊' | '┋' => '|',
        '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '╔' | '╗' | '╚' | '╝' | '╠' | '╣'
        | '╦' | '╩' | '╬' => '+',
        character if character.is_ascii() => character,
        _ => '?',
    }
}

const fn map_key(key: &Key) -> InputKey {
    match key {
        Key::Escape => InputKey::Escape,
        Key::Enter => InputKey::Enter,
        Key::Space => InputKey::Space,
        Key::Backspace => InputKey::Backspace,
        Key::ArrowUp => InputKey::Up,
        Key::ArrowDown => InputKey::Down,
        Key::ArrowLeft => InputKey::Left,
        Key::ArrowRight => InputKey::Right,
        Key::Function(number) => InputKey::Function(*number),
        Key::Character(character) => InputKey::Character(*character),
        Key::Other(code) => InputKey::Other(*code),
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use ratatui_core::backend::Backend;
    use ratatui_core::buffer::Cell;
    use rlvgl_core::renderer::Renderer;
    use rlvgl_core::widget::Color;

    use super::*;
    use crate::{CellRect, RlvglBackend, SurfaceDefaults};

    #[derive(Default)]
    struct RecordingRenderer {
        fills: alloc::vec::Vec<(Rect, Color)>,
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fills.push((rect, color));
        }

        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn view_draws_background_and_glyph_pixels() {
        let (mut backend, surface) = RlvglBackend::new(2, 1, CellMetrics::font_6x10()).unwrap();
        let cell = Cell::new("A");
        backend.draw([(0, 0, &cell)].into_iter()).unwrap();
        backend.flush().unwrap();
        let view = RatatuiView::new(
            Rect {
                x: 10,
                y: 20,
                width: 24,
                height: 20,
            },
            surface,
        );
        let mut renderer = RecordingRenderer::default();
        view.draw(&mut renderer);
        assert!(renderer.fills.iter().any(|(rect, _)| *rect
            == Rect {
                x: 10,
                y: 20,
                width: 12,
                height: 20
            }));
        assert!(renderer.fills.len() > 2, "glyph pixels should be drawn");
        let foreground = SurfaceDefaults::default().foreground;
        assert!(
            renderer
                .fills
                .iter()
                .any(|(rect, color)| *color == foreground && rect.y < 30),
            "glyph must occupy the upper half of its 20-pixel cell"
        );
    }

    #[test]
    fn dirty_cells_convert_to_view_pixels() {
        let (mut backend, surface) = RlvglBackend::new(4, 3, CellMetrics::font_6x10()).unwrap();
        surface.mark_clean();
        let cell = Cell::new("X");
        backend.draw([(2, 1, &cell)].into_iter()).unwrap();
        backend.flush().unwrap();
        assert_eq!(
            surface.dirty_cells(),
            Some(CellRect {
                x: 2,
                y: 1,
                width: 1,
                height: 1
            })
        );
        let view = RatatuiView::new(
            Rect {
                x: 5,
                y: 7,
                width: 48,
                height: 60,
            },
            surface,
        );
        assert_eq!(
            view.dirty_pixel_rect(),
            Some(Rect {
                x: 29,
                y: 27,
                width: 12,
                height: 20
            })
        );
    }

    #[test]
    fn translates_pointer_to_local_pixels_and_cells() {
        let (_, surface) = RlvglBackend::new(4, 3, CellMetrics::font_6x10()).unwrap();
        let mut view = RatatuiView::new(
            Rect {
                x: 10,
                y: 20,
                width: 48,
                height: 60,
            },
            surface,
        );
        let seen = Rc::new(RefCell::new(None));
        let seen_callback = seen.clone();
        view.set_on_input(Some(Box::new(move |input| {
            *seen_callback.borrow_mut() = Some(input);
        })));
        assert!(view.handle_event(&Event::PressRelease { x: 35, y: 51 }));
        assert_eq!(
            *seen.borrow(),
            Some(RlvglInput::Pointer {
                kind: PointerKind::Release,
                pixel: (25, 31),
                cell: (2, 1)
            })
        );
        assert!(!view.handle_event(&Event::PressRelease { x: 70, y: 31 }));
    }

    #[test]
    fn maps_unicode_box_drawing_to_ascii_bitmap_glyphs() {
        assert_eq!(bitmap_font_fallback('─'), '-');
        assert_eq!(bitmap_font_fallback('│'), '|');
        assert_eq!(bitmap_font_fallback('┼'), '+');
        assert_eq!(bitmap_font_fallback('λ'), '?');
    }
}
