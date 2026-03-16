//! Terminal rendering via egui's `Painter` API.
//!
//! The core rendering loop iterates over `Term::renderable_content().display_iter`,
//! painting cell backgrounds and characters with `rect_filled` and `galley`.
//!
//! **Paint cache:** When terminal content hasn't changed between frames (idle
//! terminal, mouse-only repaints), the expensive font-layout calls are skipped
//! and pre-built egui Shapes are replayed directly.

use std::sync::Arc;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::Term;
use conch_pty::EventProxy;
use egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Vec2};

use super::color::{convert_color, ResolvedColors};
use super::size_info::SizeInfo;

/// Convert an `[f32; 4]` RGBA color to egui's `Color32`.
#[inline]
fn rgba_to_color32(c: [f32; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}

/// Measure the monospace font's cell dimensions from egui's layout engine.
pub fn measure_cell_size(ctx: &egui::Context, font_size: f32) -> (f32, f32) {
    let font_id = FontId::new(font_size, FontFamily::Monospace);
    ctx.fonts(|fonts| {
        let g1 = fonts.layout_no_wrap("M".to_string(), font_id.clone(), Color32::WHITE);
        let g10 = fonts.layout_no_wrap("MMMMMMMMMM".to_string(), font_id, Color32::WHITE);
        let width = (g10.size().x - g1.size().x) / 9.0;
        let height = g1.size().y;
        (width, height)
    })
}

/// Convert a pixel position (relative to the window) to a terminal cell `(col, row)`.
pub fn pixel_to_cell(pos: Pos2, rect_min: Pos2, size_info: &SizeInfo) -> (usize, usize) {
    let x = (pos.x - rect_min.x - size_info.padding_x).max(0.0);
    let y = (pos.y - rect_min.y - size_info.padding_y).max(0.0);
    let col = (x / size_info.cell_width) as usize;
    let row = (y / size_info.cell_height) as usize;
    (col, row)
}

/// Check whether cell `(col, row)` falls within the normalized selection range.
#[inline]
fn is_in_selection(col: usize, row: usize, start: (usize, usize), end: (usize, usize)) -> bool {
    if row < start.1 || row > end.1 {
        return false;
    }
    if start.1 == end.1 {
        return col >= start.0 && col <= end.0;
    }
    if row == start.1 {
        return col >= start.0;
    }
    if row == end.1 {
        return col <= end.0;
    }
    true
}

/// Which underline style to draw (if any).
#[derive(Clone, Copy, PartialEq, Eq)]
enum UnderlineStyle {
    None,
    Single,
    Double,
    Dotted,
    Dashed,
    Undercurl,
}

/// Copied cell data for rendering after releasing the terminal lock.
#[derive(Clone, PartialEq)]
struct CellInfo {
    c: char,
    col: usize,
    row: usize,
    fg: [f32; 4],
    bg: [f32; 4],
    bold: bool,
    italic: bool,
    underline: UnderlineStyle,
    underline_color: Option<[f32; 4]>,
    strikeout: bool,
    wide: bool,
}

/// Cached frame data and pre-built paint shapes.
///
/// When the terminal content (cells + cursor) hasn't changed since the last
/// frame, the cached `Vec<Shape>` is replayed directly, skipping all
/// per-character `layout_no_wrap` calls.
pub struct TerminalFrameCache {
    cells: Vec<CellInfo>,
    cursor_pos: Option<(usize, usize, alacritty_terminal::vte::ansi::CursorShape)>,
    /// Pre-built shapes from the last full paint.
    paint_shapes: Vec<egui::Shape>,
    /// The rect these shapes were painted into (invalidate on resize).
    paint_rect: Option<Rect>,
}

impl Default for TerminalFrameCache {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            cursor_pos: None,
            paint_shapes: Vec::new(),
            paint_rect: None,
        }
    }
}

/// Paint the terminal grid into the given UI region.
///
/// Returns the `Response` (for mouse interaction) and the computed `SizeInfo`.
pub fn show_terminal(
    ui: &mut egui::Ui,
    term: &Arc<FairMutex<Term<EventProxy>>>,
    cell_width: f32,
    cell_height: f32,
    colors: &ResolvedColors,
    font_size: f32,
    cursor_visible: bool,
    selection: Option<((usize, usize), (usize, usize))>,
    frame_cache: &mut TerminalFrameCache,
) -> (egui::Response, SizeInfo) {
    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
    let rect = response.rect;

    let size_info = SizeInfo::new(rect.width(), rect.height(), cell_width, cell_height);

    // Fill the entire allocation with the terminal background.
    painter.rect_filled(rect, 0.0, rgba_to_color32(colors.background));

    // ── Collect cell data under lock, then release ──────────────────────
    let (cells, cursor_pos) = {
        let Some(term) = term.try_lock_unfair() else {
            // Lock contended — replay cached shapes.
            for shape in &frame_cache.paint_shapes {
                painter.add(shape.clone());
            }
            return (response, size_info);
        };
        let content = term.renderable_content();

        let show_cursor = cursor_visible
            && selection.is_none()
            && content
                .mode
                .contains(alacritty_terminal::term::TermMode::SHOW_CURSOR);

        let cursor_pos = if show_cursor {
            Some((content.cursor.point.column.0, content.cursor.point.line.0 as usize, content.cursor.shape))
        } else {
            None
        };

        let mut cells = Vec::with_capacity(size_info.columns() * size_info.rows());
        for indexed in content.display_iter {
            let cell = indexed.cell;
            let point = indexed.point;
            let flags = cell.flags;

            if flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }

            let wide = flags.contains(CellFlags::WIDE_CHAR);

            let mut fg = convert_color(cell.fg, colors);
            let mut bg = convert_color(cell.bg, colors);

            if flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            let col = point.column.0;
            let row = point.line.0 as usize;

            if let Some((sel_start, sel_end)) = selection {
                if is_in_selection(col, row, sel_start, sel_end) {
                    if let (Some(sel_bg), Some(sel_fg)) = (colors.selection_bg, colors.selection_text) {
                        bg = sel_bg;
                        fg = sel_fg;
                    } else {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                }
            }

            let c = if flags.contains(CellFlags::HIDDEN) { ' ' } else { cell.c };

            if flags.contains(CellFlags::DIM) {
                fg = [fg[0] * 0.67, fg[1] * 0.67, fg[2] * 0.67, fg[3]];
            }

            let underline = if flags.contains(CellFlags::UNDERCURL) {
                UnderlineStyle::Undercurl
            } else if flags.contains(CellFlags::DOUBLE_UNDERLINE) {
                UnderlineStyle::Double
            } else if flags.contains(CellFlags::DOTTED_UNDERLINE) {
                UnderlineStyle::Dotted
            } else if flags.contains(CellFlags::DASHED_UNDERLINE) {
                UnderlineStyle::Dashed
            } else if flags.contains(CellFlags::UNDERLINE) {
                UnderlineStyle::Single
            } else {
                UnderlineStyle::None
            };

            let underline_color = cell.underline_color().map(|uc| convert_color(uc, colors));

            cells.push(CellInfo {
                c, col, row, fg, bg,
                bold: flags.contains(CellFlags::BOLD),
                italic: flags.contains(CellFlags::ITALIC),
                underline, underline_color,
                strikeout: flags.contains(CellFlags::STRIKEOUT),
                wide,
            });
        }

        (cells, cursor_pos)
    }; // ── lock released ──

    // ── Check if we can replay cached shapes ────────────────────────────
    let same_content = cells == frame_cache.cells
        && cursor_pos == frame_cache.cursor_pos
        && frame_cache.paint_rect == Some(rect);

    if same_content && !frame_cache.paint_shapes.is_empty() {
        // Content unchanged — replay pre-built shapes (no font layout).
        for shape in &frame_cache.paint_shapes {
            painter.add(shape.clone());
        }
        return (response, size_info);
    }

    // ── Content changed — full repaint + capture shapes ─────────────────
    frame_cache.cells = cells;
    frame_cache.cursor_pos = cursor_pos;
    frame_cache.paint_rect = Some(rect);
    frame_cache.paint_shapes.clear();

    paint_cells_cached(
        &painter,
        &frame_cache.cells,
        cursor_pos,
        &size_info,
        colors,
        font_size,
        cell_width,
        cell_height,
        rect,
        &mut frame_cache.paint_shapes,
    );

    (response, size_info)
}

/// Paint all cells and collect the resulting shapes into `out_shapes`.
fn paint_cells_cached(
    painter: &Painter,
    cells: &[CellInfo],
    cursor_pos: Option<(usize, usize, alacritty_terminal::vte::ansi::CursorShape)>,
    size_info: &SizeInfo,
    colors: &ResolvedColors,
    font_size: f32,
    cell_width: f32,
    cell_height: f32,
    rect: Rect,
    out_shapes: &mut Vec<egui::Shape>,
) {
    let font_regular = FontId::new(font_size, FontFamily::Monospace);

    for ci in cells {
        let (x, y) = size_info.cell_position(ci.col, ci.row);
        let char_cell_width = if ci.wide { cell_width * 2.0 } else { cell_width };

        if ci.bg != colors.background {
            let cell_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + x, rect.min.y + y),
                Vec2::new(char_cell_width, cell_height),
            );
            let shape = egui::Shape::rect_filled(cell_rect, 0.0, rgba_to_color32(ci.bg));
            painter.add(shape.clone());
            out_shapes.push(shape);
        }

        let fg_color = rgba_to_color32(ci.fg);

        if ci.c != ' ' && ci.c != '\0' {
            paint_char_cached(
                painter, ci.c,
                Pos2::new(rect.min.x + x, rect.min.y + y),
                &font_regular, fg_color, ci.bold, ci.italic,
                char_cell_width, cell_height, out_shapes,
            );
        }

        if ci.underline != UnderlineStyle::None {
            let ul_color = ci.underline_color.map(rgba_to_color32).unwrap_or(fg_color);
            let y_base = rect.min.y + y + cell_height - 1.0;
            let x_start = rect.min.x + x;
            let x_end = x_start + char_cell_width;
            draw_underline_cached(painter, ci.underline, x_start, x_end, y_base, char_cell_width, ul_color, out_shapes);
        }

        if ci.strikeout {
            let y_mid = rect.min.y + y + cell_height * 0.5;
            let shape = egui::Shape::line_segment(
                [Pos2::new(rect.min.x + x, y_mid), Pos2::new(rect.min.x + x + char_cell_width, y_mid)],
                egui::Stroke::new(1.0, fg_color),
            );
            painter.add(shape.clone());
            out_shapes.push(shape);
        }
    }

    // Cursor.
    if let Some((col, row, shape)) = cursor_pos {
        let (cx, cy) = size_info.cell_position(col, row);
        let cursor_c = colors.cursor_color.unwrap_or(colors.foreground);
        let color = rgba_to_color32(cursor_c);
        let cursor_shape = match shape {
            alacritty_terminal::vte::ansi::CursorShape::Block => {
                egui::Shape::rect_filled(
                    Rect::from_min_size(Pos2::new(rect.min.x + cx, rect.min.y + cy), Vec2::new(cell_width, cell_height)),
                    0.0, color,
                )
            }
            alacritty_terminal::vte::ansi::CursorShape::Underline => {
                let thickness = (cell_height * 0.1).max(1.0);
                egui::Shape::rect_filled(
                    Rect::from_min_size(Pos2::new(rect.min.x + cx, rect.min.y + cy + cell_height - thickness), Vec2::new(cell_width, thickness)),
                    0.0, color,
                )
            }
            alacritty_terminal::vte::ansi::CursorShape::Beam => {
                let thickness = (cell_width * 0.12).max(1.0);
                egui::Shape::rect_filled(
                    Rect::from_min_size(Pos2::new(rect.min.x + cx, rect.min.y + cy), Vec2::new(thickness, cell_height)),
                    0.0, color,
                )
            }
            alacritty_terminal::vte::ansi::CursorShape::HollowBlock => {
                egui::Shape::rect_stroke(
                    Rect::from_min_size(Pos2::new(rect.min.x + cx, rect.min.y + cy), Vec2::new(cell_width, cell_height)),
                    0.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside,
                )
            }
            alacritty_terminal::vte::ansi::CursorShape::Hidden => egui::Shape::Noop,
        };
        painter.add(cursor_shape.clone());
        out_shapes.push(cursor_shape);
    }
}

/// Render a character and capture the shape.
fn paint_char_cached(
    painter: &Painter,
    c: char,
    pos: Pos2,
    font_id: &FontId,
    color: Color32,
    bold: bool,
    italic: bool,
    _cell_width: f32,
    cell_height: f32,
    out_shapes: &mut Vec<egui::Shape>,
) {
    let galley = painter.layout_no_wrap(c.to_string(), font_id.clone(), color);

    let mut offset_x = 0.0;
    if italic {
        offset_x += cell_height * 0.06;
    }

    let text_pos = Pos2::new(pos.x + offset_x, pos.y);
    let shape = egui::Shape::galley(text_pos, galley, color);
    painter.add(shape.clone());
    out_shapes.push(shape);

    if bold {
        let galley2 = painter.layout_no_wrap(c.to_string(), font_id.clone(), color);
        let shape2 = egui::Shape::galley(Pos2::new(text_pos.x + 1.0, text_pos.y), galley2, color);
        painter.add(shape2.clone());
        out_shapes.push(shape2);
    }
}

/// Draw underline and capture shapes.
fn draw_underline_cached(
    painter: &Painter,
    style: UnderlineStyle,
    x_start: f32,
    x_end: f32,
    y_base: f32,
    cell_width: f32,
    color: Color32,
    out_shapes: &mut Vec<egui::Shape>,
) {
    let stroke = egui::Stroke::new(1.0, color);
    match style {
        UnderlineStyle::Single => {
            let s = egui::Shape::line_segment([Pos2::new(x_start, y_base), Pos2::new(x_end, y_base)], stroke);
            painter.add(s.clone()); out_shapes.push(s);
        }
        UnderlineStyle::Double => {
            let s1 = egui::Shape::line_segment([Pos2::new(x_start, y_base), Pos2::new(x_end, y_base)], stroke);
            let s2 = egui::Shape::line_segment([Pos2::new(x_start, y_base - 2.0), Pos2::new(x_end, y_base - 2.0)], stroke);
            painter.add(s1.clone()); out_shapes.push(s1);
            painter.add(s2.clone()); out_shapes.push(s2);
        }
        UnderlineStyle::Dotted => {
            let dot_spacing = 3.0;
            let mut x = x_start;
            while x < x_end {
                let s = egui::Shape::circle_filled(Pos2::new(x, y_base), 0.5, color);
                painter.add(s.clone()); out_shapes.push(s);
                x += dot_spacing;
            }
        }
        UnderlineStyle::Dashed => {
            let dash_len = cell_width * 0.3;
            let gap_len = cell_width * 0.15;
            let mut x = x_start;
            while x < x_end {
                let dash_end = (x + dash_len).min(x_end);
                let s = egui::Shape::line_segment([Pos2::new(x, y_base), Pos2::new(dash_end, y_base)], stroke);
                painter.add(s.clone()); out_shapes.push(s);
                x += dash_len + gap_len;
            }
        }
        UnderlineStyle::Undercurl => {
            let amplitude = 1.5;
            let half_period = cell_width * 0.25;
            let mut points = Vec::new();
            let mut x = x_start;
            while x <= x_end {
                let t = (x - x_start) / half_period;
                let dy = amplitude * (t * std::f32::consts::PI).sin();
                points.push(Pos2::new(x, y_base + dy));
                x += 1.0;
            }
            if points.len() >= 2 {
                for pair in points.windows(2) {
                    let s = egui::Shape::line_segment([pair[0], pair[1]], stroke);
                    painter.add(s.clone()); out_shapes.push(s);
                }
            }
        }
        UnderlineStyle::None => {}
    }
}

// ── Selection / word helpers (used by mouse.rs) ──

/// Find word boundaries around the given cell position.
pub fn word_selection_at(
    term: &Arc<FairMutex<Term<EventProxy>>>,
    col: usize,
    row: usize,
) -> ((usize, usize), (usize, usize)) {
    let Some(term) = term.try_lock_unfair() else {
        return ((col, row), (col, row));
    };

    let content = term.renderable_content();
    let mut start = col;
    for ci in content.display_iter {
        if ci.point.line.0 as usize == row && ci.point.column.0 < col {
            let c = ci.cell.c;
            if c == ' ' || c == '\0' {
                start = ci.point.column.0 + 1;
            }
        }
    }

    let content2 = term.renderable_content();
    let mut end = col;
    for ci in content2.display_iter {
        if ci.point.line.0 as usize == row && ci.point.column.0 >= col {
            let c = ci.cell.c;
            if c == ' ' || c == '\0' {
                break;
            }
            end = ci.point.column.0;
        }
    }

    ((start, row), (end, row))
}

/// Select an entire line.
pub fn line_selection_at(
    term: &Arc<FairMutex<Term<EventProxy>>>,
    row: usize,
) -> ((usize, usize), (usize, usize)) {
    let Some(term) = term.try_lock_unfair() else {
        return ((0, row), (0, row));
    };
    let content = term.renderable_content();
    let mut max_col = 0;
    for ci in content.display_iter {
        if ci.point.line.0 as usize == row {
            max_col = ci.point.column.0;
        }
    }
    ((0, row), (max_col, row))
}

/// Extract the selected text from the terminal grid.
pub fn get_selected_text(
    term: &Arc<FairMutex<Term<EventProxy>>>,
    start: (usize, usize),
    end: (usize, usize),
) -> String {
    let Some(term) = term.try_lock_unfair() else {
        return String::new();
    };

    let content = term.renderable_content();
    let mut result = String::new();
    let mut last_row: Option<usize> = None;

    for indexed in content.display_iter {
        let col = indexed.point.column.0;
        let row = indexed.point.line.0 as usize;

        if is_in_selection(col, row, start, end) {
            if let Some(prev_row) = last_row {
                if row > prev_row {
                    result.push('\n');
                }
            }
            let c = indexed.cell.c;
            if c != '\0' {
                result.push(c);
            }
            last_row = Some(row);
        }
    }

    result
}
