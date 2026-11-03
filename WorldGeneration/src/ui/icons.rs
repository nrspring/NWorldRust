//! The icons on the toolbar.

use super::*;

/// Draws a closed outline through `points`.
pub(super) fn draw_outline(points: &[Vec2], color: Color) {
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        draw_line(p.x, p.y, q.x, q.y, 1.6, color);
    }
}

/// An open folder: "open".
pub(super) fn draw_open_icon(r: Rect, color: Color) {
    let c = r.center();
    draw_outline(
        &[
            c + vec2(-9.0, 8.0),
            c + vec2(-9.0, -7.0),
            c + vec2(-3.0, -7.0),
            c + vec2(-1.0, -4.0),
            c + vec2(9.0, -4.0),
            c + vec2(9.0, 8.0),
        ],
        color,
    );
}

/// A floppy disk: "save".
pub(super) fn draw_save_icon(r: Rect, color: Color) {
    let c = r.center();
    draw_outline(
        &[
            c + vec2(-9.0, -9.0),
            c + vec2(5.0, -9.0),
            c + vec2(9.0, -5.0),
            c + vec2(9.0, 9.0),
            c + vec2(-9.0, 9.0),
        ],
        color,
    );
    // The metal shutter at the top and the label at the bottom.
    draw_outline(
        &[c + vec2(-5.0, -9.0), c + vec2(-5.0, -3.5), c + vec2(3.0, -3.5), c + vec2(3.0, -9.0)],
        color,
    );
    draw_outline(
        &[c + vec2(-6.0, 9.0), c + vec2(-6.0, 2.0), c + vec2(6.0, 2.0), c + vec2(6.0, 9.0)],
        color,
    );
}

/// A curved arrow pointing back: "undo".
pub(super) fn draw_undo_icon(r: Rect, color: Color) {
    let c = r.center() + vec2(0.0, 2.0);
    let radius = 7.5;
    let (start, end) = (-2.4f32, 1.35f32);
    let point = |a: f32| c + vec2(a.cos(), a.sin()) * radius;
    let steps = 18;
    for k in 0..steps {
        let a0 = start + (end - start) * k as f32 / steps as f32;
        let a1 = start + (end - start) * (k + 1) as f32 / steps as f32;
        let (p0, p1) = (point(a0), point(a1));
        draw_line(p0.x, p0.y, p1.x, p1.y, 1.8, color);
    }
    // Arrowhead at the start of the arc, pointing the way the arc runs backwards.
    let tip = point(start);
    let heading = vec2(start.sin(), -start.cos());
    let tip = tip + heading * 2.0;
    for wing in [2.55f32, -2.55] {
        let (s, co) = wing.sin_cos();
        let dir = vec2(heading.x * co - heading.y * s, heading.x * s + heading.y * co);
        let end = tip + dir * 5.0;
        draw_line(tip.x, tip.y, end.x, end.y, 1.8, color);
    }
}

/// A die showing five pips: "random".
pub(super) fn draw_dice(c: Vec2, color: Color) {
    let r = Rect::new(c.x - 8.0, c.y - 8.0, 16.0, 16.0);
    style::rounded_rect_lines(r, 3.5, 1.5, color);
    for (dx, dy) in [(-4.0, -4.0), (4.0, -4.0), (0.0, 0.0), (-4.0, 4.0), (4.0, 4.0)] {
        draw_circle(c.x + dx, c.y + dy, 1.4, color);
    }
}

pub(super) fn draw_tool_icon(tool: Tool, r: Rect, color: Color) {
    let c = r.center();
    let t = 1.6;
    match tool {
        Tool::NewMap => {
            // A folded map with a plus badge: "new map".
            let (left, right) = (c.x - 10.0, c.x + 10.0);
            let (top, bottom) = (c.y - 8.0, c.y + 8.0);
            let (f1, f2) = (c.x - 3.3, c.x + 3.3);
            let outline = [
                (vec2(left, top + 2.0), vec2(f1, top)),
                (vec2(f1, top), vec2(f2, top + 2.0)),
                (vec2(f2, top + 2.0), vec2(right, top)),
                (vec2(right, top), vec2(right, bottom - 2.0)),
                (vec2(right, bottom - 2.0), vec2(f2, bottom)),
                (vec2(f2, bottom), vec2(f1, bottom - 2.0)),
                (vec2(f1, bottom - 2.0), vec2(left, bottom)),
                (vec2(left, bottom), vec2(left, top + 2.0)),
                (vec2(f1, top), vec2(f1, bottom - 2.0)),
                (vec2(f2, top + 2.0), vec2(f2, bottom)),
            ];
            for (a, b) in outline {
                draw_line(a.x, a.y, b.x, b.y, t, color);
            }
            let badge = vec2(right, bottom);
            draw_circle(badge.x, badge.y, 7.0, style::SURFACE);
            draw_circle(badge.x, badge.y, 5.5, style::ACCENT);
            draw_line(badge.x - 3.0, badge.y, badge.x + 3.0, badge.y, 1.5, WHITE);
            draw_line(badge.x, badge.y - 3.0, badge.x, badge.y + 3.0, 1.5, WHITE);
        }
        Tool::Mountains => {
            // A tall peak with a snow cap, and a smaller one beside it.
            let ground = c.y + 9.0;
            let (peak_x, peak_y) = (c.x - 2.0, c.y - 10.0);
            draw_line(c.x - 12.0, ground, peak_x, peak_y, t, color);
            draw_line(peak_x, peak_y, c.x + 7.0, ground, t, color);
            draw_line(c.x + 3.0, c.y - 1.0, c.x + 11.0, ground, t, color);
            draw_line(c.x - 12.0, ground, c.x + 11.0, ground, t, color);
            // The snow line, zigzagging across the peak.
            let line = [vec2(peak_x - 3.7, peak_y + 6.0), vec2(peak_x - 1.5, peak_y + 4.0), vec2(peak_x + 0.5, peak_y + 6.0), vec2(peak_x + 2.5, peak_y + 4.0), vec2(peak_x + 3.7, peak_y + 5.5)];
            for pair in line.windows(2) {
                draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 1.3, color);
            }
        }
        Tool::Lakes => {
            // A pond with two ripples across it.
            draw_ellipse_lines(c.x, c.y, 11.0, 7.5, 0.0, t, color);
            for (dx, dy, w) in [(-3.5, -1.5, 3.0), (2.0, 2.5, 3.5)] {
                draw_line(c.x + dx - w, c.y + dy, c.x + dx + w, c.y + dy, 1.2, color);
            }
        }
        Tool::Forest => {
            // Two conifers, a small one behind and left of a bigger one: a trunk and three
            // stacked tiers of foliage each.
            let ground = c.y + 10.0;
            for (dx, scale) in [(-6.0f32, 0.7f32), (3.0, 1.0)] {
                let x = c.x + dx;
                draw_line(x, ground - 2.0 * scale, x, ground, t, color);
                for (apex, bottom, half) in [(-15.0f32, -9.0f32, 4.0f32), (-11.0, -5.0, 5.5), (-7.0, -2.0, 7.0)] {
                    let (top, base) = (ground + apex * scale, ground + bottom * scale);
                    draw_line(x, top, x - half * scale, base, 1.3, color);
                    draw_line(x, top, x + half * scale, base, 1.3, color);
                    draw_line(x - half * scale, base, x + half * scale, base, 1.3, color);
                }
            }
        }
        Tool::Stone => {
            // A heap of rocks: a big one with a smaller one on each side.
            draw_outline(
                &[c + vec2(-6.0, 9.0), c + vec2(-7.5, 1.0), c + vec2(-2.0, -7.0), c + vec2(5.0, -6.0), c + vec2(8.0, 2.0), c + vec2(6.5, 9.0)],
                color,
            );
            draw_outline(&[c + vec2(-12.0, 9.0), c + vec2(-11.5, 4.0), c + vec2(-7.5, 3.0), c + vec2(-6.0, 9.0)], color);
            draw_outline(&[c + vec2(6.5, 9.0), c + vec2(8.0, 4.5), c + vec2(12.0, 5.0), c + vec2(12.5, 9.0)], color);
            // A crack across the big one.
            draw_line(c.x - 2.0, c.y - 7.0, c.x + 1.0, c.y + 1.0, 1.1, color);
        }
        Tool::Iron => {
            // Two stacked ingots, each wider at the foot.
            draw_outline(&[c + vec2(-12.0, 9.0), c + vec2(12.0, 9.0), c + vec2(9.5, 2.0), c + vec2(-9.5, 2.0)], color);
            draw_outline(&[c + vec2(-7.5, 2.0), c + vec2(7.5, 2.0), c + vec2(5.5, -6.0), c + vec2(-5.5, -6.0)], color);
        }
        Tool::Copper => {
            // A coin: a rim, a smaller ring inside it, and a bar across the middle.
            draw_circle_lines(c.x, c.y, 11.0, t, color);
            draw_circle_lines(c.x, c.y, 7.0, 1.2, color);
            draw_line(c.x - 3.5, c.y, c.x + 3.5, c.y, 1.4, color);
        }
        Tool::Sulphur => {
            // A heap of lumps: two round ones side by side and a third on top.
            draw_circle_lines(c.x - 6.0, c.y + 4.0, 6.5, t, color);
            draw_circle_lines(c.x + 6.0, c.y + 5.0, 6.0, t, color);
            draw_circle_lines(c.x, c.y - 4.5, 7.0, t, color);
        }
        Tool::Oil => {
            // A drop: two sides meeting at a point above a round belly.
            let belly = c + vec2(0.0, 4.0);
            draw_line(c.x, c.y - 12.0, belly.x - 6.9, belly.y - 4.0, t, color);
            draw_line(c.x, c.y - 12.0, belly.x + 6.9, belly.y - 4.0, t, color);
            let steps = 18;
            for k in 0..steps {
                let angle = |k: usize| (-30.0 + 240.0 * k as f32 / steps as f32).to_radians();
                let (a, b) = (angle(k), angle(k + 1));
                draw_line(belly.x + 8.0 * a.cos(), belly.y + 8.0 * a.sin(), belly.x + 8.0 * b.cos(), belly.y + 8.0 * b.sin(), t, color);
            }
        }
        Tool::Shallows => {
            // Waves over water that is shallower toward the shore: three wavy lines, the
            // topmost the faintest.
            for (k, dy) in [-7.0f32, 0.0, 7.0].into_iter().enumerate() {
                let mut shade = color;
                shade.a *= 0.45 + 0.275 * k as f32;
                let steps = 16;
                for s in 0..steps {
                    let (x0, x1) = (c.x - 10.0 + 20.0 * s as f32 / steps as f32, c.x - 10.0 + 20.0 * (s + 1) as f32 / steps as f32);
                    let wave = |x: f32| c.y + dy + 2.2 * ((x - c.x) * 0.55 + k as f32).sin();
                    draw_line(x0, wave(x0), x1, wave(x1), t, shade);
                }
            }
        }
        Tool::Regions => {
            // A map cut into areas: an outline with borders meeting in a point inside it.
            draw_rectangle_lines(c.x - 11.0, c.y - 9.0, 22.0, 18.0, t, color);
            let hub = c + vec2(-1.0, 1.0);
            for end in [c + vec2(-3.0, -9.0), c + vec2(-5.0, 9.0), c + vec2(11.0, -3.0)] {
                draw_line(hub.x, hub.y, end.x, end.y, 1.4, color);
            }
        }
        Tool::Passes => {
            // Two peaks with a road running through the gap between them.
            draw_line(c.x - 12.0, c.y + 4.0, c.x - 6.0, c.y - 9.0, t, color);
            draw_line(c.x - 6.0, c.y - 9.0, c.x, c.y - 1.0, t, color);
            draw_line(c.x, c.y - 1.0, c.x + 6.0, c.y - 9.0, t, color);
            draw_line(c.x + 6.0, c.y - 9.0, c.x + 12.0, c.y + 4.0, t, color);
            for k in 0..4 {
                let x = c.x - 11.0 + k as f32 * 6.0;
                draw_line(x, c.y + 9.0, x + 3.5, c.y + 9.0, t, color);
            }
        }
        Tool::Rivers => {
            // A river winding across the icon, widening toward its mouth.
            let steps = 24;
            let point = |k: usize| {
                let t = k as f32 / steps as f32;
                vec2(c.x - 11.0 + 22.0 * t, c.y + 5.5 * (t * std::f32::consts::TAU * 1.1).sin())
            };
            for k in 0..steps {
                let (p0, p1) = (point(k), point(k + 1));
                draw_line(p0.x, p0.y, p1.x, p1.y, 1.0 + 1.6 * k as f32 / steps as f32, color);
            }
        }
        Tool::Islands => {
            // Three islands of different sizes.
            draw_ellipse_lines(c.x - 3.5, c.y + 2.0, 7.5, 5.0, 0.3, t, color);
            draw_ellipse_lines(c.x + 6.0, c.y - 5.5, 3.6, 2.6, -0.4, t, color);
            draw_ellipse_lines(c.x + 6.5, c.y + 6.0, 2.6, 1.9, 0.2, t, color);
        }
        Tool::Continents => {
            // A globe: outline, a meridian ellipse and two parallels.
            let radius = 10.0;
            draw_circle_lines(c.x, c.y, radius, t, color);
            let steps = 24;
            for k in 0..steps {
                let a0 = std::f32::consts::TAU * k as f32 / steps as f32;
                let a1 = std::f32::consts::TAU * (k + 1) as f32 / steps as f32;
                let p = |a: f32| vec2(c.x + a.cos() * radius * 0.42, c.y + a.sin() * radius);
                let (p0, p1) = (p(a0), p(a1));
                draw_line(p0.x, p0.y, p1.x, p1.y, t, color);
            }
            draw_line(c.x - radius, c.y, c.x + radius, c.y, t, color);
            for dy in [-5.0f32, 5.0] {
                let half = (radius * radius - dy * dy).sqrt();
                draw_line(c.x - half, c.y + dy, c.x + half, c.y + dy, 1.2, color);
            }
        }
    }
}
