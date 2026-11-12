//! Ikony i rysunki rysowane wektorowo (ostre w każdej skali, bez plików graficznych).

use cam_ops::ToolKind;
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, StrokeKind};
use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Select,
    Move,
    Rotate,
    FlipH,
    FlipV,
    Scale,
    Align,
    Array,
    ProfileOut,
    ProfileIn,
    PocketOffset,
    PocketZigzag,
    PocketAdaptive,
    DrillSimple,
    DrillPeck,
    DrillHelix,
    EngCenter,
    EngLeft,
    EngRight,
    EngVcarve,
    ZeroBL,
    ZeroCenter,
    ZeroTL,
    ZeroPick,
    Preview3D,
}

const BLUE: Color32 = Color32::from_rgb(52, 120, 220);
const BLUE_LIGHT: Color32 = Color32::from_rgb(110, 165, 240);
const BLUE_DARK: Color32 = Color32::from_rgb(28, 70, 150);
const PLATE: Color32 = Color32::from_rgb(170, 176, 185);
const HOLE: Color32 = Color32::from_rgb(35, 38, 43);
const GREEN: Color32 = Color32::from_rgb(70, 210, 110);
const RED: Color32 = Color32::from_rgb(235, 90, 70);
const AMBER: Color32 = Color32::from_rgb(240, 180, 80);
const STEEL: Color32 = Color32::from_rgb(207, 210, 214);
const STEEL_DARK: Color32 = Color32::from_rgb(150, 154, 160);
const LINE_DARK: Color32 = Color32::from_rgb(92, 96, 102);
const OUTLINE: Color32 = Color32::from_rgb(17, 19, 22);
const WHITE: Color32 = Color32::from_rgb(230, 232, 235);

/// Rysuje ikonę w kwadracie `r` (współrzędne projektu 0..40).
pub fn draw(painter: &egui::Painter, r: Rect, icon: Icon) {
    let k = r.width() / 40.0;
    let p = |x: f32, y: f32| Pos2::new(r.min.x + x * k, r.min.y + y * k);
    let st = |w: f32, c: Color32| Stroke::new(w * k.max(0.6), c);
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(p(x, y), egui::vec2(w * k, h * k));
    let poly = |pts: &[(f32, f32)], fill: Color32, stroke: Stroke| {
        painter.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| p(x, y)).collect(), fill, stroke));
    };
    let line = |pts: &[(f32, f32)], s: Stroke| {
        painter.add(Shape::line(pts.iter().map(|&(x, y)| p(x, y)).collect(), s));
    };
    let dashed = |pts: Vec<Pos2>, s: Stroke| {
        painter.extend(Shape::dashed_line(&pts, s, 3.0 * k, 2.0 * k));
    };
    let tool_dot = |x: f32, y: f32, rr: f32| {
        painter.circle_filled(p(x, y), rr * k, RED);
        painter.circle_stroke(p(x, y), rr * k, st(1.0, OUTLINE));
    };
    let plate_with_pocket = || {
        painter.rect_filled(rect(3.0, 7.0, 34.0, 26.0), 2.0 * k, PLATE);
        painter.rect_filled(rect(7.0, 11.0, 26.0, 18.0), 4.0 * k, HOLE);
    };
    let drill_bit = |x: f32, y: f32| {
        painter.rect_filled(rect(x + 1.1, y, 4.8, 6.5), 0.0, STEEL_DARK);
        painter.rect_filled(rect(x, y + 6.5, 7.0, 12.0), 0.0, STEEL);
        for d in [4.0, 8.0, 12.0] {
            line(&[(x, y + 6.5 + d), (x + 7.0, y + 4.0 + d)], st(1.0, LINE_DARK));
        }
        poly(&[(x, y + 18.5), (x + 7.0, y + 18.5), (x + 3.5, y + 23.0)], STEEL, Stroke::NONE);
    };
    let wave = |dy: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let t = i as f32 / 24.0;
                p(4.0 + 32.0 * t, 24.0 + dy + 7.0 * (t * PI * 2.2 + 0.4).sin())
            })
            .collect()
    };
    let zero_frame = |ox: f32, oy: f32, up: bool| {
        painter.rect_stroke(rect(7.0, 8.0, 27.0, 24.0), 0.0, st(1.4, PLATE), StrokeKind::Middle);
        line(&[(ox, oy), (ox + 12.0, oy)], st(2.4, Color32::from_rgb(224, 90, 90)));
        let dy = if up { -12.0 } else { 12.0 };
        line(&[(ox, oy), (ox, oy + dy)], st(2.4, Color32::from_rgb(70, 194, 90)));
        painter.circle_filled(p(ox, oy), 4.0 * k, WHITE);
        painter.circle_stroke(p(ox, oy), 4.0 * k, st(1.0, OUTLINE));
    };

    match icon {
        Icon::Select => {
            let pts = [(12.0, 4.0), (12.0, 33.0), (18.5, 26.5), (23.0, 37.0), (28.0, 35.0), (23.5, 24.5), (32.0, 24.0)];
            poly(&[pts[0], pts[1], pts[2], pts[6]], RED, Stroke::NONE);
            poly(&[pts[2], pts[3], pts[4], pts[5]], RED, Stroke::NONE);
            painter.add(Shape::closed_line(pts.iter().map(|&(x, y)| p(x, y)).collect(), st(1.4, OUTLINE)));
        }
        Icon::Move => {
            line(&[(20.0, 7.0), (20.0, 33.0)], st(2.4, BLUE_LIGHT));
            line(&[(7.0, 20.0), (33.0, 20.0)], st(2.4, BLUE_LIGHT));
            poly(&[(20.0, 2.0), (15.0, 9.0), (25.0, 9.0)], BLUE_LIGHT, Stroke::NONE);
            poly(&[(20.0, 38.0), (25.0, 31.0), (15.0, 31.0)], BLUE_LIGHT, Stroke::NONE);
            poly(&[(2.0, 20.0), (9.0, 25.0), (9.0, 15.0)], BLUE_LIGHT, Stroke::NONE);
            poly(&[(38.0, 20.0), (31.0, 15.0), (31.0, 25.0)], BLUE_LIGHT, Stroke::NONE);
        }
        Icon::Rotate => {
            let pts: Vec<Pos2> = (0..=30)
                .map(|i| {
                    let a = (-40.0 + 290.0 * i as f32 / 30.0) * PI / 180.0;
                    p(20.0 + 13.0 * a.cos(), 20.0 - 13.0 * a.sin())
                })
                .collect();
            painter.add(Shape::line(pts, st(3.0, BLUE_LIGHT)));
            poly(&[(26.0, 33.0), (37.0, 30.0), (30.0, 23.0)], BLUE_LIGHT, Stroke::NONE);
        }
        Icon::FlipH => {
            dashed(vec![p(20.0, 3.0), p(20.0, 37.0)], st(1.2, Color32::from_gray(150)));
            poly(&[(17.0, 8.0), (17.0, 32.0), (4.0, 32.0)], BLUE, Stroke::NONE);
            painter.add(Shape::closed_line(vec![p(23.0, 8.0), p(36.0, 32.0), p(23.0, 32.0)], st(1.6, BLUE_LIGHT)));
        }
        Icon::FlipV => {
            dashed(vec![p(3.0, 20.0), p(37.0, 20.0)], st(1.2, Color32::from_gray(150)));
            poly(&[(8.0, 17.0), (32.0, 4.0), (32.0, 17.0)], BLUE, Stroke::NONE);
            painter.add(Shape::closed_line(vec![p(8.0, 23.0), p(32.0, 23.0), p(32.0, 36.0)], st(1.6, BLUE_LIGHT)));
        }
        Icon::Scale => {
            dashed(
                vec![p(4.0, 4.0), p(36.0, 4.0), p(36.0, 36.0), p(4.0, 36.0), p(4.0, 4.0)],
                st(1.3, BLUE_LIGHT),
            );
            painter.rect_filled(rect(4.0, 21.0, 15.0, 15.0), 0.0, BLUE);
            line(&[(17.0, 23.0), (31.0, 9.0)], st(2.0, WHITE));
            poly(&[(34.0, 6.0), (32.5, 15.0), (25.0, 7.5)], WHITE, Stroke::NONE);
        }
        Icon::Align => {
            line(&[(7.0, 4.0), (7.0, 36.0)], st(2.0, WHITE));
            painter.rect_filled(rect(9.0, 7.0, 22.0, 6.0), k, BLUE);
            painter.rect_filled(rect(9.0, 17.0, 14.0, 6.0), k, BLUE_LIGHT);
            painter.rect_filled(rect(9.0, 27.0, 26.0, 6.0), k, BLUE);
        }
        Icon::Array => {
            for (i, (x, y)) in [(4.0, 8.0), (15.5, 8.0), (27.0, 8.0), (4.0, 22.0), (15.5, 22.0), (27.0, 22.0)].into_iter().enumerate() {
                painter.rect_filled(rect(x, y, 9.0, 10.0), 2.0 * k, if i == 0 { BLUE_LIGHT } else { BLUE });
            }
        }
        Icon::ProfileOut => {
            painter.rect_filled(rect(9.0, 11.0, 22.0, 18.0), 3.0 * k, BLUE);
            painter.rect_stroke(rect(9.0, 11.0, 22.0, 18.0), 3.0 * k, st(1.0, BLUE_DARK), StrokeKind::Inside);
            dashed(round_rect_pts(rect(4.5, 6.5, 31.0, 27.0), 7.0 * k), st(1.8, GREEN));
            tool_dot(35.5, 20.0, 3.6);
        }
        Icon::ProfileIn => {
            painter.rect_filled(rect(3.0, 7.0, 34.0, 26.0), 2.0 * k, PLATE);
            painter.rect_filled(rect(9.0, 12.0, 22.0, 16.0), 4.0 * k, HOLE);
            painter.rect_stroke(rect(9.0, 12.0, 22.0, 16.0), 4.0 * k, st(1.4, BLUE), StrokeKind::Outside);
            dashed(round_rect_pts(rect(12.5, 15.5, 15.0, 9.0), 1.5 * k), st(1.6, GREEN));
            tool_dot(27.5, 20.0, 3.0);
        }
        Icon::PocketOffset => {
            plate_with_pocket();
            painter.rect_stroke(rect(10.0, 14.0, 20.0, 12.0), 3.0 * k, st(1.4, GREEN), StrokeKind::Middle);
            painter.rect_stroke(rect(13.5, 17.5, 13.0, 5.0), k, st(1.4, GREEN), StrokeKind::Middle);
            tool_dot(30.0, 20.0, 2.8);
        }
        Icon::PocketZigzag => {
            plate_with_pocket();
            line(&[(10.0, 14.0), (30.0, 14.0), (30.0, 18.0), (10.0, 18.0), (10.0, 22.0), (30.0, 22.0), (30.0, 26.0), (10.0, 26.0)], st(1.4, GREEN));
            tool_dot(10.0, 26.0, 2.6);
        }
        Icon::PocketAdaptive => {
            plate_with_pocket();
            let pts: Vec<Pos2> = (0..=80)
                .map(|i| {
                    let t = i as f32 / 80.0;
                    let a = t * PI * 2.0 * 4.0;
                    p(11.0 + 18.0 * t + 3.0 * a.cos(), 20.0 + 4.0 * a.sin())
                })
                .collect();
            painter.add(Shape::line(pts, st(1.3, GREEN)));
        }
        Icon::DrillSimple => {
            painter.circle_filled(p(20.0, 29.0), 7.5 * k, HOLE);
            painter.circle_stroke(p(20.0, 29.0), 7.5 * k, st(1.5, BLUE));
            line(&[(9.0, 29.0), (31.0, 29.0)], st(1.0, GREEN));
            line(&[(20.0, 19.0), (20.0, 39.0)], st(1.0, GREEN));
            drill_bit(16.5, 1.5);
        }
        Icon::DrillPeck => {
            painter.circle_filled(p(16.0, 30.0), 7.0 * k, HOLE);
            painter.circle_stroke(p(16.0, 30.0), 7.0 * k, st(1.5, BLUE));
            drill_bit(12.5, 2.0);
            line(&[(32.0, 9.0), (32.0, 29.0)], st(2.0, AMBER));
            poly(&[(32.0, 4.0), (28.0, 10.0), (36.0, 10.0)], AMBER, Stroke::NONE);
            poly(&[(32.0, 34.0), (36.0, 28.0), (28.0, 28.0)], AMBER, Stroke::NONE);
        }
        Icon::DrillHelix => {
            painter.circle_filled(p(20.0, 20.0), 15.0 * k, HOLE);
            painter.circle_stroke(p(20.0, 20.0), 15.0 * k, st(1.5, BLUE));
            let pts: Vec<Pos2> = (0..=90)
                .map(|i| {
                    let t = i as f32 / 90.0;
                    let a = t * PI * 2.0 * 2.5;
                    let rr = 11.0 - 6.0 * t;
                    p(20.0 + rr * a.cos(), 20.0 + rr * a.sin())
                })
                .collect();
            painter.add(Shape::line(pts, st(1.3, GREEN)));
            tool_dot(31.0, 20.0, 2.8);
        }
        Icon::EngCenter => {
            painter.add(Shape::line(wave(2.0), st(4.5, BLUE)));
            painter.add(Shape::line(wave(2.0), st(1.2, GREEN)));
            painter.rect_filled(rect(27.8, 2.0, 4.4, 8.0), 0.0, STEEL_DARK);
            poly(&[(25.5, 10.0), (34.5, 10.0), (30.0, 20.0)], STEEL, st(1.0, OUTLINE));
        }
        Icon::EngLeft => {
            painter.add(Shape::line(wave(4.0), st(3.0, BLUE)));
            dashed(wave(-3.0), st(1.6, GREEN));
            tool_dot(36.0, 21.0 - 3.0 + 7.0 * (PI * 2.2 + 0.4).sin(), 2.8);
            line(&[(12.0, 6.0), (7.0, 9.0), (12.0, 12.0)], st(1.6, WHITE));
        }
        Icon::EngRight => {
            painter.add(Shape::line(wave(-3.0), st(3.0, BLUE)));
            dashed(wave(4.0), st(1.6, GREEN));
            tool_dot(36.0, 28.0 + 7.0 * (PI * 2.2 + 0.4).sin(), 2.8);
            line(&[(28.0, 32.0), (33.0, 35.0), (28.0, 38.0)], st(1.6, WHITE));
        }
        Icon::EngVcarve => {
            line(&[(7.0, 7.0), (20.0, 34.0), (33.0, 7.0)], st(7.0, BLUE));
            line(&[(7.0, 7.0), (20.0, 34.0), (33.0, 7.0)], st(1.2, GREEN));
        }
        Icon::ZeroBL => zero_frame(7.0, 32.0, true),
        Icon::ZeroCenter => zero_frame(20.0, 20.0, true),
        Icon::ZeroTL => zero_frame(7.0, 8.0, false),
        Icon::ZeroPick => {
            painter.circle_stroke(p(18.0, 18.0), 10.0 * k, st(1.6, WHITE));
            line(&[(18.0, 4.0), (18.0, 32.0)], st(1.2, WHITE));
            line(&[(4.0, 18.0), (32.0, 18.0)], st(1.2, WHITE));
            let pts = [(24.0, 24.0), (24.0, 38.0), (28.0, 34.0), (31.0, 39.0), (34.0, 37.5), (31.0, 32.5), (36.0, 32.0)];
            poly(&[pts[0], pts[1], pts[2], pts[6]], RED, Stroke::NONE);
            poly(&[pts[2], pts[3], pts[4], pts[5]], RED, Stroke::NONE);
        }
        Icon::Preview3D => {
            poly(&[(20.0, 5.0), (34.0, 13.0), (20.0, 21.0), (6.0, 13.0)], BLUE_LIGHT, st(1.0, OUTLINE));
            poly(&[(6.0, 13.0), (20.0, 21.0), (20.0, 36.0), (6.0, 28.0)], BLUE, st(1.0, OUTLINE));
            poly(&[(34.0, 13.0), (34.0, 28.0), (20.0, 36.0), (20.0, 21.0)], BLUE_DARK, st(1.0, OUTLINE));
        }
    }
}

fn round_rect_pts(r: Rect, radius: f32) -> Vec<Pos2> {
    let rad = radius.min(r.width() * 0.5).min(r.height() * 0.5);
    let corners = [
        (Pos2::new(r.right() - rad, r.top() + rad), -90.0f32),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0),
    ];
    let mut pts = Vec::new();
    for (c, a0) in corners {
        for i in 0..=6 {
            let a = (a0 + 90.0 * i as f32 / 6.0) * PI / 180.0;
            pts.push(c + egui::vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    pts
}

/// Duży przycisk paska narzędzi: ikona + podpis. `accent` — kolor ramki aktywnego.
pub fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, active: bool, enabled: bool, accent: Color32) -> egui::Response {
    let wrap = ui.available_width() - 6.0;
    let measure = ui.painter().layout(label.to_string(), egui::FontId::proportional(11.0), Color32::WHITE, wrap);
    let icon_size = 32.0;
    let h = 8.0 + icon_size + 3.0 + measure.size().y + 6.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h.max(62.0)), egui::Sense::click());
    let v = ui.visuals();
    let (bg, border) = if active {
        (Color32::from_rgb(31, 51, 80), Stroke::new(1.5, accent))
    } else if resp.hovered() && enabled {
        (v.widgets.hovered.weak_bg_fill, Stroke::new(1.0, v.widgets.hovered.bg_stroke.color))
    } else {
        (Color32::from_rgb(35, 38, 43), Stroke::new(1.0, Color32::from_rgb(52, 57, 64)))
    };
    let painter = ui.painter();
    painter.rect(rect, 7.0, bg, border, StrokeKind::Inside);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 6.0 + icon_size * 0.5),
        egui::vec2(icon_size, icon_size),
    );
    draw(painter, icon_rect, icon);
    let text_color = if !enabled {
        Color32::from_gray(110)
    } else if active {
        Color32::WHITE
    } else {
        Color32::from_gray(225)
    };
    // tekst wyśrodkowany w liniach
    let galley = {
        let mut job = egui::text::LayoutJob::simple(label.to_string(), egui::FontId::proportional(11.0), text_color, wrap);
        job.halign = egui::Align::Center;
        ui.painter().layout_job(job)
    };
    painter.galley(Pos2::new(rect.center().x, icon_rect.bottom() + 3.0), galley, text_color);
    if !enabled {
        resp.on_hover_text("Wkrótce")
    } else {
        resp
    }
}

/// Rysunek freza (sylwetka) w prostokącie `r`; układ 40 × 100 jednostek.
/// Zwraca prostokąty: (część skrawająca, cała sylwetka) — do wymiarowania.
pub fn draw_tool(painter: &egui::Painter, r: Rect, kind: &ToolKind) -> (Rect, Rect) {
    let k = (r.width() / 40.0).min(r.height() / 100.0);
    let o = Pos2::new(r.center().x - 20.0 * k, r.center().y - 50.0 * k);
    let p = |x: f32, y: f32| Pos2::new(o.x + x * k, o.y + y * k);
    let outline = Stroke::new((0.8 * k).max(0.8), Color32::from_rgb(43, 47, 53));
    let helix = Stroke::new((1.1 * k).max(0.8), Color32::from_rgb(107, 112, 120));
    let body = Color32::from_rgb(212, 215, 219);
    let shank = Color32::from_rgb(158, 163, 170);
    let rr = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_max(p(x, y), p(x + w, y + h));
    let helix_lines = |y0: f32, y1: f32, x0: f32, x1: f32| {
        let mut y = y0 + 10.0;
        while y <= y1 {
            painter.line_segment([p(x0, y), p(x1, y - 7.0)], helix);
            y += 10.0;
        }
    };
    let path = |pts: Vec<Pos2>| {
        painter.add(Shape::convex_polygon(pts, body, outline));
    };
    let cutting;
    match kind {
        ToolKind::FlatEnd => {
            painter.rect(rr(13.0, 1.0, 14.0, 43.0), k, shank, outline, StrokeKind::Middle);
            painter.rect(rr(11.0, 44.0, 18.0, 52.0), 0.0, body, outline, StrokeKind::Middle);
            helix_lines(44.0, 96.0, 11.0, 29.0);
            cutting = rr(11.0, 44.0, 18.0, 52.0);
        }
        ToolKind::BallEnd => {
            painter.rect(rr(13.0, 1.0, 14.0, 43.0), k, shank, outline, StrokeKind::Middle);
            let mut pts = vec![p(11.0, 44.0), p(29.0, 44.0)];
            for i in 0..=16 {
                let a = PI * i as f32 / 16.0;
                pts.push(p(20.0 + 9.0 * a.cos(), 87.0 + 9.0 * a.sin()));
            }
            path(pts);
            helix_lines(44.0, 86.0, 11.0, 29.0);
            cutting = rr(11.0, 44.0, 18.0, 52.0);
        }
        ToolKind::BullNose { .. } => {
            painter.rect(rr(13.0, 1.0, 14.0, 43.0), k, shank, outline, StrokeKind::Middle);
            painter.rect(rr(11.0, 44.0, 18.0, 52.0), egui::CornerRadius { nw: 0, ne: 0, sw: (5.0 * k) as u8, se: (5.0 * k) as u8 }, body, outline, StrokeKind::Middle);
            helix_lines(44.0, 90.0, 11.0, 29.0);
            cutting = rr(11.0, 44.0, 18.0, 52.0);
        }
        ToolKind::VBit { .. } => {
            painter.rect(rr(13.0, 1.0, 14.0, 55.0), k, shank, outline, StrokeKind::Middle);
            path(vec![p(11.0, 56.0), p(29.0, 56.0), p(20.0, 97.0)]);
            painter.line_segment([p(14.0, 58.0), p(20.0, 95.0)], helix);
            cutting = rr(11.0, 56.0, 18.0, 41.0);
        }
        ToolKind::Engraver { .. } => {
            painter.rect(rr(13.0, 1.0, 14.0, 55.0), k, shank, outline, StrokeKind::Middle);
            path(vec![p(11.0, 56.0), p(29.0, 56.0), p(21.6, 95.0), p(18.4, 95.0)]);
            painter.line_segment([p(15.0, 58.0), p(19.5, 94.0)], helix);
            cutting = rr(11.0, 56.0, 18.0, 39.0);
        }
        ToolKind::Drill { .. } => {
            painter.rect(rr(14.0, 1.0, 12.0, 39.0), k, shank, outline, StrokeKind::Middle);
            painter.rect(rr(12.0, 40.0, 16.0, 48.0), 0.0, body, outline, StrokeKind::Middle);
            helix_lines(40.0, 88.0, 12.0, 28.0);
            path(vec![p(12.0, 88.0), p(28.0, 88.0), p(20.0, 98.0)]);
            cutting = rr(12.0, 40.0, 16.0, 58.0);
        }
    }
    (cutting, rr(11.0, 1.0, 18.0, 97.0))
}
