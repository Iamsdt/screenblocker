//! Renders the remaining time as a tray-icon image (the "panel countdown").
//! Uses a blocky 7-segment digit renderer drawn with tiny-skia, so it never
//! depends on system fonts being available.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

const SIZE: u32 = 64;

// Accent green (normal) / coral (final minute — break imminent).
const ACCENT: (u8, u8, u8) = (47, 158, 111);
const WARN: (u8, u8, u8) = (207, 83, 64);
// Muted grey (paused).
const MUTED: (u8, u8, u8) = (108, 114, 122);

/// Segment membership per digit: [a, b, c, d, e, f, g].
fn segments(d: u8) -> [bool; 7] {
    match d {
        0 => [true, true, true, true, true, true, false],
        1 => [false, true, true, false, false, false, false],
        2 => [true, true, false, true, true, false, true],
        3 => [true, true, true, true, false, false, true],
        4 => [false, true, true, false, false, true, true],
        5 => [true, false, true, true, false, true, true],
        6 => [true, false, true, true, true, true, true],
        7 => [true, true, true, false, false, false, false],
        8 => [true, true, true, true, true, true, true],
        9 => [true, true, true, true, false, true, true],
        _ => [false; 7],
    }
}

fn rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
}

fn push_digit(pb: &mut PathBuilder, digit: u8, cx: f32, cy: f32, dw: f32, dh: f32, t: f32) {
    let seg = segments(digit);
    let half = dh / 2.0;
    // [a, b, c, d, e, f, g]
    let rects: [(bool, f32, f32, f32, f32); 7] = [
        (seg[0], cx, cy, dw, t),                          // a  top
        (seg[1], cx + dw - t, cy, t, half),               // b  upper-right
        (seg[2], cx + dw - t, cy + half, t, half),        // c  lower-right
        (seg[3], cx, cy + dh - t, dw, t),                 // d  bottom
        (seg[4], cx, cy + half, t, half),                 // e  lower-left
        (seg[5], cx, cy, t, half),                        // f  upper-left
        (seg[6], cx, cy + half - t / 2.0, dw, t),         // g  middle
    ];
    for (on, x, y, w, h) in rects {
        if on {
            if let Some(r) = Rect::from_xywh(x, y, w, h) {
                pb.push_rect(r);
            }
        }
    }
}

/// Render the given 1–2 char numeric string to straight-alpha RGBA bytes.
pub fn render_rgba(text: &str, warn: bool) -> Vec<u8> {
    let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();

    // Pill background.
    let (r, g, b) = if warn { WARN } else { ACCENT };
    let mut bg = PathBuilder::new();
    rounded_rect(&mut bg, 2.0, 16.0, 60.0, 32.0, 11.0);
    if let Some(path) = bg.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    // Digits (white).
    let digits: Vec<u8> = text.chars().filter_map(|c| c.to_digit(10).map(|d| d as u8)).collect();
    if !digits.is_empty() {
        let n = digits.len() as f32;
        let (dw, dh, t, gap) = (15.0f32, 26.0f32, 4.5f32, 7.0f32);
        let block_w = n * dw + (n - 1.0) * gap;
        let start_x = (SIZE as f32 - block_w) / 2.0;
        let cy = 16.0 + (32.0 - dh) / 2.0;

        let mut dp = PathBuilder::new();
        for (i, d) in digits.iter().enumerate() {
            let cx = start_x + i as f32 * (dw + gap);
            push_digit(&mut dp, *d, cx, cy, dw, dh, t);
        }
        if let Some(path) = dp.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 255);
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    unpremultiply(pixmap.data())
}

/// Render a symbolic glyph (white shapes on a coloured pill) to straight-alpha
/// RGBA bytes. Used for the paused and meeting-waiting tray states.
fn render_glyph(pill: (u8, u8, u8), rects: &[(f32, f32, f32, f32)]) -> Vec<u8> {
    let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();

    let mut bg = PathBuilder::new();
    rounded_rect(&mut bg, 2.0, 16.0, 60.0, 32.0, 11.0);
    if let Some(path) = bg.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(pill.0, pill.1, pill.2, 255);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    let mut gp = PathBuilder::new();
    for &(x, y, w, h) in rects {
        // Slightly rounded corners so the marks match the pill's softness.
        rounded_rect(&mut gp, x, y, w, h, (w.min(h) / 3.0).min(3.0));
    }
    if let Some(path) = gp.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    unpremultiply(pixmap.data())
}

/// tiny-skia stores premultiplied alpha; convert to straight alpha for Tauri.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let unmul = |c: u8| ((c as u16 * 255 + (a as u16 / 2)) / a as u16).min(255) as u8;
            out.extend_from_slice(&[unmul(r), unmul(g), unmul(b), a]);
        }
    }
    out
}

/// Build a Tauri tray image for the given remaining seconds.
pub fn time_icon(remaining_secs: i64) -> tauri::image::Image<'static> {
    let (text, warn) = if remaining_secs < 60 {
        // Final minute: show seconds, coral pill.
        (remaining_secs.max(0).to_string(), true)
    } else {
        // Minutes remaining (rounded up), accent pill.
        (((remaining_secs + 59) / 60).to_string(), false)
    };
    let rgba = render_rgba(&text, warn);
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

/// Paused: two vertical bars on a muted grey pill.
pub fn pause_icon() -> tauri::image::Image<'static> {
    let bars = [(21.0, 23.0, 7.0, 18.0), (36.0, 23.0, 7.0, 18.0)];
    let rgba = render_glyph(MUTED, &bars);
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_expected_buffer_size() {
        let rgba = render_rgba("24", false);
        assert_eq!(rgba.len() as u32, SIZE * SIZE * 4);
    }

    #[test]
    fn single_and_double_digit_both_render() {
        assert_eq!(render_rgba("5", false).len() as u32, SIZE * SIZE * 4);
        assert_eq!(render_rgba("45", true).len() as u32, SIZE * SIZE * 4);
    }

    #[test]
    fn glyphs_render_expected_buffer_size() {
        let bars = [(21.0, 23.0, 7.0, 18.0), (36.0, 23.0, 7.0, 18.0)];
        assert_eq!(render_glyph(MUTED, &bars).len() as u32, SIZE * SIZE * 4);
    }
}
