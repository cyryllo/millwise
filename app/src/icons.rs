//! Ikony i rysunki rysowane wektorowo (ostre w każdej skali, bez plików graficznych).

use cam_i18n::tr;
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
    Measure,
    ProfileOut,
    ProfileIn,
    Corners,
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
    Text,
    ZeroBL,
    ZeroCenter,
    ZeroTL,
    ZeroPick,
    Preview3D,
    Rough3D,
    Finish3D,
    Cutout3D,
    Shape3D,
    // ikony kroków historii zmian
    Open,
    Delete,
    Paste,
    Cut,
    Stock,
    Document,
    Curve,
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
        Icon::Rough3D => {
            // przekrój: kopuła modelu na płycie, schodki po zgrubnej i warstwy ścieżek
            painter.rect_filled(rect(3.0, 31.0, 34.0, 6.0), k, PLATE);
            let dome: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let a = PI * i as f32 / 24.0;
                    (20.0 - 14.0 * a.cos(), 31.0 - 20.0 * a.sin())
                })
                .collect();
            poly(&dome, BLUE, st(1.0, OUTLINE));
            line(&[(3.0, 31.0), (3.0, 25.0), (8.5, 25.0), (8.5, 18.0), (12.0, 18.0), (12.0, 11.5), (28.0, 11.5), (28.0, 18.0), (31.5, 18.0), (31.5, 25.0), (37.0, 25.0), (37.0, 31.0)], st(1.4, AMBER));
            for y in [7.0, 14.5, 21.5, 28.0] {
                line(&[(2.0, y), (6.0, y)], st(1.3, GREEN));
                line(&[(34.0, y), (38.0, y)], st(1.3, GREEN));
            }
            tool_dot(4.0, 21.5, 2.4);
        }
        Icon::Finish3D => {
            // przekrój: gładka kopuła i frez kulowy jadący po niej
            painter.rect_filled(rect(3.0, 31.0, 34.0, 6.0), k, PLATE);
            let dome: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let a = PI * i as f32 / 24.0;
                    (20.0 - 14.0 * a.cos(), 31.0 - 20.0 * a.sin())
                })
                .collect();
            poly(&dome, BLUE, st(1.0, OUTLINE));
            let path: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = PI * i as f32 / 24.0;
                    p(20.0 - 17.0 * a.cos(), 31.0 - 23.0 * a.sin())
                })
                .collect();
            painter.add(Shape::line(path, st(1.4, GREEN)));
            // frez kulowy nad lewym zboczem
            let (cx, cy) = (8.0, 14.0);
            painter.rect_filled(rect(cx - 3.0, 2.0, 6.0, cy - 2.0), 0.0, STEEL_DARK);
            painter.circle_filled(p(cx, cy), 3.0 * k, STEEL);
            painter.circle_stroke(p(cx, cy), 3.0 * k, st(0.8, OUTLINE));
        }
        Icon::Cutout3D => {
            // z góry: płyta, detal z kopułą i obrys cięcia z mostkami
            painter.rect_filled(rect(2.0, 2.0, 36.0, 36.0), 2.0 * k, PLATE);
            painter.rect_filled(rect(9.0, 9.0, 22.0, 22.0), 2.0 * k, BLUE);
            painter.circle_filled(p(20.0, 20.0), 6.5 * k, BLUE_LIGHT);
            painter.circle_stroke(p(20.0, 20.0), 6.5 * k, st(0.8, OUTLINE));
            for seg in [
                [(6.0, 6.0), (16.0, 6.0)],
                [(24.0, 6.0), (34.0, 6.0)],
                [(34.0, 6.0), (34.0, 16.0)],
                [(34.0, 24.0), (34.0, 34.0)],
                [(34.0, 34.0), (24.0, 34.0)],
                [(16.0, 34.0), (6.0, 34.0)],
                [(6.0, 34.0), (6.0, 24.0)],
                [(6.0, 16.0), (6.0, 6.0)],
            ] {
                line(&seg, st(2.0, GREEN));
            }
            tool_dot(34.0, 11.0, 2.6);
        }
        Icon::Shape3D => {
            // przekrój: litera z zarysu (u góry) podniesiona nad płytę z zaokrąglonymi krawędziami
            painter.rect_filled(rect(3.0, 31.0, 34.0, 6.0), k, PLATE);
            let mut prof: Vec<(f32, f32)> = vec![(8.0, 31.0)];
            for i in 0..=6 {
                let a = std::f32::consts::FRAC_PI_2 * i as f32 / 6.0;
                prof.push((8.0 + 6.0 * (1.0 - a.cos()), 31.0 - 9.0 * a.sin()));
            }
            for i in 0..=6 {
                let a = std::f32::consts::FRAC_PI_2 * i as f32 / 6.0;
                prof.push((32.0 - 6.0 * (1.0 - a.sin()), 22.0 + 9.0 * (1.0 - a.cos())));
            }
            prof.push((32.0, 31.0));
            poly(&prof, BLUE, st(1.0, OUTLINE));
            // zarys krzywej nad kształtem
            let a = [(9.0, 15.0), (16.0, 3.0), (24.0, 3.0), (31.0, 15.0)];
            line(&a, st(1.8, AMBER));
            line(&[(13.5, 10.0), (26.5, 10.0)], st(1.4, AMBER));
        }
        Icon::Text => {
            // litera „A” z zarysu i linia bazowa
            let a = [(6.0, 34.0), (17.0, 6.0), (23.0, 6.0), (34.0, 34.0), (28.0, 34.0), (25.5, 27.0), (14.5, 27.0), (12.0, 34.0)];
            painter.add(Shape::closed_line(a.iter().map(|&(x, y)| p(x, y)).collect(), st(2.2, BLUE_LIGHT)));
            painter.add(Shape::closed_line(vec![p(16.5, 21.5), p(20.0, 12.0), p(23.5, 21.5)], st(1.8, BLUE_LIGHT)));
            line(&[(3.0, 37.0), (37.0, 37.0)], st(1.2, AMBER));
        }
        Icon::Measure => {
            // linijka z podziałką i strzałka wymiaru
            let ruler = [(4.0, 24.0), (36.0, 24.0), (36.0, 34.0), (4.0, 34.0)];
            poly(&ruler, AMBER, st(1.0, OUTLINE));
            for (k, x) in [8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0].iter().enumerate() {
                let h = if k % 2 == 0 { 5.0 } else { 3.0 };
                line(&[(*x, 24.0), (*x, 24.0 + h)], st(1.2, OUTLINE));
            }
            line(&[(6.0, 13.0), (34.0, 13.0)], st(1.6, WHITE));
            poly(&[(4.0, 13.0), (9.0, 9.5), (9.0, 16.5)], WHITE, Stroke::NONE);
            poly(&[(36.0, 13.0), (31.0, 9.5), (31.0, 16.5)], WHITE, Stroke::NONE);
            line(&[(4.0, 7.0), (4.0, 19.0)], st(1.2, WHITE));
            line(&[(36.0, 7.0), (36.0, 19.0)], st(1.2, WHITE));
        }
        Icon::Corners => {
            // płyta z kwadratowym otworem i okrągłymi uszkami w rogach
            painter.rect_filled(rect(3.0, 3.0, 34.0, 34.0), 2.0 * k, PLATE);
            painter.rect_filled(rect(10.0, 10.0, 20.0, 20.0), 0.0, HOLE);
            for (x, y) in [(10.5, 10.5), (29.5, 10.5), (29.5, 29.5), (10.5, 29.5)] {
                painter.circle_filled(p(x, y), 3.6 * k, HOLE);
            }
            painter.rect_stroke(rect(13.0, 13.0, 14.0, 14.0), 0.0, st(1.4, GREEN), StrokeKind::Middle);
        }
        Icon::Open => {
            poly(&[(4.0, 9.0), (15.0, 9.0), (18.0, 13.0), (36.0, 13.0), (36.0, 33.0), (4.0, 33.0)], AMBER, st(1.0, OUTLINE));
            poly(&[(4.0, 33.0), (9.0, 18.0), (39.0, 18.0), (34.0, 33.0)], Color32::from_rgb(250, 205, 115), st(1.0, OUTLINE));
        }
        Icon::Delete => {
            painter.add(Shape::line(wave(-4.0), st(2.6, BLUE_LIGHT)));
            line(&[(11.0, 11.0), (29.0, 29.0)], st(4.0, RED));
            line(&[(29.0, 11.0), (11.0, 29.0)], st(4.0, RED));
        }
        Icon::Paste => {
            painter.rect_filled(rect(7.0, 7.0, 26.0, 30.0), 3.0 * k, Color32::from_rgb(160, 120, 75));
            painter.rect_filled(rect(10.0, 11.0, 20.0, 23.0), 1.0 * k, WHITE);
            painter.rect_filled(rect(14.0, 4.0, 12.0, 6.0), 2.0 * k, STEEL_DARK);
            for y in [17.0, 22.0, 27.0] {
                line(&[(13.0, y), (27.0, y)], st(1.6, BLUE));
            }
        }
        Icon::Cut => {
            line(&[(13.0, 4.0), (25.0, 26.0)], st(3.0, STEEL));
            line(&[(27.0, 4.0), (15.0, 26.0)], st(3.0, STEEL));
            painter.circle_stroke(p(13.0, 30.0), 5.0 * k, st(2.6, RED));
            painter.circle_stroke(p(27.0, 30.0), 5.0 * k, st(2.6, RED));
        }
        Icon::Stock => {
            let wood = Color32::from_rgb(200, 164, 112);
            poly(&[(20.0, 8.0), (37.0, 15.0), (20.0, 22.0), (3.0, 15.0)], wood, st(1.0, OUTLINE));
            poly(&[(3.0, 15.0), (20.0, 22.0), (20.0, 32.0), (3.0, 25.0)], Color32::from_rgb(160, 120, 75), st(1.0, OUTLINE));
            poly(&[(37.0, 15.0), (37.0, 25.0), (20.0, 32.0), (20.0, 22.0)], Color32::from_rgb(125, 90, 55), st(1.0, OUTLINE));
        }
        Icon::Document => {
            poly(&[(8.0, 4.0), (26.0, 4.0), (33.0, 11.0), (33.0, 36.0), (8.0, 36.0)], WHITE, st(1.0, OUTLINE));
            for y in [15.0, 21.0, 27.0] {
                line(&[(13.0, y), (28.0, y)], st(1.8, LINE_DARK));
            }
        }
        Icon::Curve => {
            painter.add(Shape::line(wave(-4.0), st(3.0, BLUE_LIGHT)));
        }
    }
}

/// Rysunek maszyny danego rodzaju (widok z przodu) w prostokącie `r`; układ 60 × 44 jednostek.
pub fn draw_machine(painter: &egui::Painter, r: Rect, kind: cam_post::MachineKind) {
    use cam_post::MachineKind;
    let k = (r.width() / 60.0).min(r.height() / 44.0);
    let o = Pos2::new(r.center().x - 30.0 * k, r.center().y - 22.0 * k);
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(o.x + x * k, o.y + y * k), egui::vec2(w * k, h * k));
    let fill = |x: f32, y: f32, w: f32, h: f32, c: Color32| {
        painter.rect(rect(x, y, w, h), 1.0 * k, c, Stroke::new(1.0, OUTLINE), StrokeKind::Inside);
    };
    let wood = Color32::from_rgb(200, 164, 112);
    let frame = BLUE;
    let frame_dark = BLUE_DARK;
    // wrzeciono z frezem: korpus (x, y) szerokości 8, frez w dół
    let spindle = |x: f32, y: f32, h: f32| {
        fill(x, y, 8.0, h, STEEL);
        fill(x + 2.5, y + h, 3.0, 3.0, STEEL_DARK);
        painter.line_segment(
            [Pos2::new(o.x + (x + 4.0) * k, o.y + (y + h + 3.0) * k), Pos2::new(o.x + (x + 4.0) * k, o.y + (y + h + 6.0) * k)],
            Stroke::new(1.6 * k.max(0.6), WHITE),
        );
    };
    let arrows = |y: f32| {
        let s = Stroke::new(1.6 * k.max(0.6), RED);
        let p = |x: f32, yy: f32| Pos2::new(o.x + x * k, o.y + yy * k);
        painter.line_segment([p(20.0, y), p(40.0, y)], s);
        painter.add(Shape::convex_polygon(vec![p(17.0, y), p(21.0, y - 2.5), p(21.0, y + 2.5)], RED, Stroke::NONE));
        painter.add(Shape::convex_polygon(vec![p(43.0, y), p(39.0, y - 2.5), p(39.0, y + 2.5)], RED, Stroke::NONE));
    };
    match kind {
        MachineKind::Gantry => {
            fill(3.0, 33.0, 54.0, 5.0, PLATE); // stół
            fill(5.0, 38.0, 4.0, 4.0, LINE_DARK);
            fill(51.0, 38.0, 4.0, 4.0, LINE_DARK);
            fill(16.0, 30.0, 28.0, 3.0, wood); // materiał
            fill(9.0, 8.0, 5.0, 25.0, frame); // boki bramy
            fill(46.0, 8.0, 5.0, 25.0, frame);
            fill(7.0, 6.0, 46.0, 7.0, frame_dark); // belka
            spindle(26.0, 11.0, 11.0);
            arrows(3.0);
        }
        MachineKind::MovingTable => {
            fill(6.0, 4.0, 6.0, 34.0, LINE_DARK); // nieruchome boki
            fill(48.0, 4.0, 6.0, 34.0, LINE_DARK);
            fill(6.0, 4.0, 48.0, 7.0, STEEL_DARK); // belka
            fill(4.0, 38.0, 52.0, 4.0, LINE_DARK); // podstawa
            spindle(26.0, 11.0, 10.0);
            fill(14.0, 29.0, 32.0, 4.0, frame); // ruchomy stół
            fill(18.0, 26.0, 24.0, 3.0, wood);
            arrows(36.0);
        }
        MachineKind::BenchMill => {
            fill(8.0, 37.0, 40.0, 5.0, LINE_DARK); // podstawa
            fill(38.0, 4.0, 9.0, 33.0, frame); // kolumna
            fill(14.0, 6.0, 30.0, 10.0, frame_dark); // wrzeciennik
            spindle(20.0, 16.0, 7.0);
            fill(8.0, 30.0, 30.0, 4.0, PLATE); // stół
            fill(12.0, 34.0, 22.0, 3.0, STEEL_DARK);
            fill(16.0, 26.0, 16.0, 4.0, Color32::from_rgb(170, 180, 190)); // imadło
            painter.circle(Pos2::new(o.x + 5.0 * k, o.y + 32.0 * k), 3.0 * k, STEEL, Stroke::new(1.0, OUTLINE)); // pokrętło
        }
        MachineKind::SheetRouter => {
            fill(1.0, 30.0, 58.0, 5.0, PLATE); // długi stół
            for x in [3.0, 28.0, 53.0] {
                fill(x, 35.0, 4.0, 7.0, LINE_DARK);
            }
            fill(2.0, 27.0, 56.0, 3.0, wood); // cała płyta
            fill(18.0, 12.0, 4.0, 15.0, frame);
            fill(38.0, 12.0, 4.0, 15.0, frame);
            fill(16.0, 9.0, 28.0, 6.0, frame_dark);
            spindle(26.0, 14.0, 8.0);
            arrows(4.0);
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

/// Szeroki przycisk przejścia do innego widoku (np. symulacji): wyróżniony
/// kolorem, z ikoną po lewej, żeby nie mylił się ze zwykłymi przyciskami.
pub fn wide_view_button(ui: &mut egui::Ui, icon: Icon, label: &str, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
    let painter = ui.painter();
    let bg = if resp.hovered() { color.gamma_multiply(0.35) } else { color.gamma_multiply(0.2) };
    painter.rect(rect, 7.0, bg, Stroke::new(1.5, color), StrokeKind::Inside);
    let icon_rect = Rect::from_center_size(Pos2::new(rect.left() + 22.0, rect.center().y), egui::vec2(24.0, 24.0));
    draw(painter, icon_rect, icon);
    painter.text(
        Pos2::new(rect.center().x + 12.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    resp
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
        resp.on_hover_text(tr!("Wkrótce"))
    } else {
        resp
    }
}

/// Gradient poziomy (od lewej krawędzi walca do prawej): punkty (pozycja 0..1, kolor).
type Stops = [(f32, [u8; 3]); 6];
/// Stal chwytu — połysk przesunięty w lewo, ciemne brzegi.
const METAL: Stops = [(0.0, [69, 74, 81]), (0.18, [149, 155, 162]), (0.32, [228, 232, 236]), (0.5, [158, 164, 171]), (0.85, [90, 95, 102]), (1.0, [56, 60, 66])];
/// Węglik spiekany części tnącej — jaśniejszy.
const CARBIDE: Stops = [(0.0, [89, 96, 103]), (0.2, [181, 188, 195]), (0.33, [245, 247, 249]), (0.55, [176, 182, 189]), (0.85, [103, 109, 116]), (1.0, [69, 74, 80])];
/// Wiertło HSS-TiN — złote.
const GOLD: Stops = [(0.0, [107, 83, 32]), (0.2, [201, 162, 74]), (0.33, [246, 226, 164]), (0.55, [195, 154, 67]), (0.85, [122, 94, 34]), (1.0, [77, 59, 21])];

fn grad(stops: &Stops, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let k = stops.iter().position(|s| s.0 >= t).unwrap_or(stops.len() - 1).max(1);
    let (a, b) = (stops[k - 1], stops[k]);
    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 0.0 };
    let c = |i: usize| (a.1[i] as f32 + (b.1[i] as f32 - a.1[i] as f32) * f).round() as u8;
    Color32::from_rgb(c(0), c(1), c(2))
}

/// Kolor oznaczenia rodzaju (pierścień na chwycie) — jak u producentów frezów.
pub fn tool_color(kind: &ToolKind) -> Option<Color32> {
    Some(match kind {
        ToolKind::FlatEnd => Color32::from_rgb(61, 127, 224),
        ToolKind::BallEnd => Color32::from_rgb(63, 191, 127),
        ToolKind::BullNose { .. } => Color32::from_rgb(43, 181, 181),
        ToolKind::VBit { .. } => Color32::from_rgb(232, 137, 58),
        ToolKind::Engraver { .. } => Color32::from_rgb(168, 116, 232),
        ToolKind::TaperedBall { .. } => Color32::from_rgb(232, 92, 154),
        ToolKind::Drill { .. } => return None,
    })
}

/// Rysunek freza w prostokącie `r` (układ 40 × 100 jednostek): metaliczny chwyt na pełną
/// szerokość z kolorowym pierścieniem rodzaju, część tnąca z węglika (wiertło złote) ze
/// spiralnymi rowkami. Każda część to siatka pionowych pasków z kolorami w wierzchołkach
/// (połysk walca), rowki to ukośne pasy przycięte do kształtu.
/// Zwraca prostokąty: (część skrawająca, cała sylwetka) — do wymiarowania.
pub fn draw_tool(painter: &egui::Painter, r: Rect, kind: &ToolKind) -> (Rect, Rect) {
    let k = (r.width() / 40.0).min(r.height() / 100.0);
    let o = Pos2::new(r.center().x - 20.0 * k, r.center().y - 50.0 * k);
    let p = |x: f32, y: f32| Pos2::new(o.x + x * k, o.y + y * k);
    let rr = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_max(p(x, y), p(x + w, y + h));
    let edge = Stroke::new((0.5 * k).max(0.6), Color32::from_rgb(13, 15, 18));
    const N: usize = 28;

    // kształt: x0..x1, w każdym x od góry `top` do dołu `bot`
    type Prof<'a> = &'a dyn Fn(f32) -> f32;
    let fill = |x0: f32, x1: f32, top: Prof, bot: Prof, stops: &Stops| {
        let mut m = egui::Mesh::default();
        for i in 0..=N {
            let t = i as f32 / N as f32;
            let x = x0 + (x1 - x0) * t;
            let c = grad(stops, t);
            m.colored_vertex(p(x, top(x)), c);
            m.colored_vertex(p(x, bot(x).max(top(x))), c);
        }
        for i in 0..N as u32 {
            let a = 2 * i;
            m.add_triangle(a, a + 1, a + 2);
            m.add_triangle(a + 1, a + 3, a + 2);
        }
        painter.add(Shape::mesh(m));
    };
    // spiralne rowki: ukośne pasy (ciemny rowek + jasna krawędź) przycięte do kształtu
    let flutes = |x0: f32, x1: f32, top: Prof, bot: Prof| {
        let slope = 26f32.to_radians().tan();
        let (period, band) = (10.0, 3.4);
        let mut m = egui::Mesh::default();
        let mut quad = |xa: f32, xb: f32, ya: (f32, f32), yb: (f32, f32), ca: Color32, cb: Color32| {
            let n = m.vertices.len() as u32;
            m.colored_vertex(p(xa, ya.0), ca);
            m.colored_vertex(p(xa, ya.1), cb);
            m.colored_vertex(p(xb, yb.0), ca);
            m.colored_vertex(p(xb, yb.1), cb);
            m.add_triangle(n, n + 1, n + 2);
            m.add_triangle(n + 1, n + 3, n + 2);
        };
        let dark = Color32::from_rgb(28, 31, 35);
        let mid = Color32::from_rgb(64, 69, 76);
        let light = Color32::from_rgba_unmultiplied(255, 255, 255, 110);
        for i in 0..N {
            let (xa, xb) = (x0 + (x1 - x0) * i as f32 / N as f32, x0 + (x1 - x0) * (i + 1) as f32 / N as f32);
            let clip = |x: f32, y0: f32, y1: f32| (y0.clamp(top(x), bot(x)), y1.clamp(top(x), bot(x)));
            for kk in 2..14 {
                let y = |x: f32| kk as f32 * period - slope * x;
                let (a, b) = (clip(xa, y(xa), y(xa) + band), clip(xb, y(xb), y(xb) + band));
                if a.1 - a.0 > 1e-3 || b.1 - b.0 > 1e-3 {
                    quad(xa, xb, a, b, dark, mid);
                }
                let (a, b) = (clip(xa, y(xa) + band, y(xa) + band + 0.7), clip(xb, y(xb) + band, y(xb) + band + 0.7));
                if a.1 - a.0 > 1e-3 || b.1 - b.0 > 1e-3 {
                    quad(xa, xb, a, b, light, light);
                }
            }
        }
        painter.add(Shape::mesh(m));
    };
    // kontur kształtu
    let outline = |x0: f32, x1: f32, top: Prof, bot: Prof| {
        let mut pts: Vec<Pos2> = (0..=N).map(|i| x0 + (x1 - x0) * i as f32 / N as f32).map(|x| p(x, top(x))).collect();
        pts.extend((0..=N).rev().map(|i| x0 + (x1 - x0) * i as f32 / N as f32).map(|x| p(x, bot(x).max(top(x)))));
        painter.add(Shape::closed_line(pts, edge));
    };
    // chwyt na pełną szerokość, pierścień rodzaju, pasek przejścia do części tnącej
    let shank = |x0: f32, x1: f32, len: f32| {
        let (t, b) = (|_: f32| 2.0, move |_: f32| len);
        fill(x0, x1, &t, &b, &METAL);
        if let Some(c) = tool_color(kind) {
            // pierścień cieniowany jak walec
            let ring_y = len - 15.0;
            let mut m = egui::Mesh::default();
            for i in 0..=N {
                let tt = i as f32 / N as f32;
                let g = grad(&METAL, tt);
                let f = (g.r() as f32 / 200.0).clamp(0.35, 1.15);
                let cc = Color32::from_rgb(
                    (c.r() as f32 * f).min(255.0) as u8,
                    (c.g() as f32 * f).min(255.0) as u8,
                    (c.b() as f32 * f).min(255.0) as u8,
                );
                let x = x0 + (x1 - x0) * tt;
                m.colored_vertex(p(x, ring_y), cc);
                m.colored_vertex(p(x, ring_y + 4.0), cc);
            }
            for i in 0..N as u32 {
                let a = 2 * i;
                m.add_triangle(a, a + 1, a + 2);
                m.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(m));
        }
        outline(x0, x1, &t, &b);
        let (nt, nb) = (move |_: f32| len, move |_: f32| len + 2.5);
        fill(x0, x1, &nt, &nb, &METAL);
        outline(x0, x1, &nt, &nb);
    };
    // rowek prosty (frezy stożkowe)
    let groove = |pts: [(f32, f32); 3]| {
        painter.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| p(x, y)).collect(), Color32::from_rgb(36, 40, 45), Stroke::NONE));
        painter.line_segment([p(pts[2].0, pts[2].1), p(pts[1].0 + 0.2, pts[1].1)], Stroke::new((0.5 * k).max(0.5), Color32::from_rgba_unmultiplied(255, 255, 255, 120)));
    };

    let cutting;
    match *kind {
        ToolKind::FlatEnd | ToolKind::BallEnd | ToolKind::BullNose { .. } => {
            shank(11.0, 29.0, 45.0);
            let top = |_: f32| 47.5;
            let bot = move |x: f32| -> f32 {
                let dx = (x - 20.0).abs();
                match kind {
                    ToolKind::BallEnd => 87.0 + (81.0 - dx * dx).max(0.0).sqrt(),
                    ToolKind::BullNose { .. } if dx > 4.0 => 91.0 + (25.0 - (dx - 4.0).powi(2)).max(0.0).sqrt(),
                    _ => 96.0,
                }
            };
            fill(11.0, 29.0, &top, &bot, &CARBIDE);
            flutes(11.0, 29.0, &top, &bot);
            outline(11.0, 29.0, &top, &bot);
            cutting = rr(11.0, 47.5, 18.0, 48.5);
        }
        ToolKind::VBit { .. } | ToolKind::Engraver { .. } | ToolKind::TaperedBall { .. } => {
            // kulowy stożkowy: długi, smukły stożek od pełnej średnicy aż do kulki
            let tapered = matches!(kind, ToolKind::TaperedBall { .. });
            let len = if tapered { 33.0 } else { 55.0 };
            let y0 = len + 2.5;
            shank(11.0, 29.0, len);
            let top = move |_: f32| y0;
            let bot = move |x: f32| -> f32 {
                let dx = (x - 20.0).abs();
                let cone = |tip_dx: f32, tip_y: f32| y0 + (tip_y - y0) * (9.0 - dx) / (9.0 - tip_dx);
                match kind {
                    ToolKind::Engraver { .. } if dx <= 1.5 => 95.0,
                    ToolKind::Engraver { .. } => cone(1.5, 95.0),
                    // kulka r = 3.4 (środek 92.6), styczna ze stożkiem
                    ToolKind::TaperedBall { .. } if dx <= 3.35 => 92.6 + (11.56 - dx * dx).max(0.0).sqrt(),
                    ToolKind::TaperedBall { .. } => cone(3.35, 93.2),
                    _ => cone(0.0, 97.0),
                }
            };
            fill(11.0, 29.0, &top, &bot, &CARBIDE);
            match kind {
                ToolKind::Engraver { .. } => groove([(14.6, 58.5), (19.4, 94.2), (18.1, 58.5)]),
                ToolKind::TaperedBall { .. } => {
                    groove([(14.4, y0 + 1.0), (18.2, 91.5), (17.8, y0 + 1.0)]);
                    painter.circle_filled(p(18.8, 94.0), 1.1 * k, Color32::from_rgba_unmultiplied(255, 255, 255, 200));
                }
                _ => groove([(14.4, 58.5), (20.0, 95.5), (18.0, 58.5)]),
            }
            outline(11.0, 29.0, &top, &bot);
            cutting = rr(11.0, y0, 18.0, 96.0 - y0);
        }
        ToolKind::Drill { .. } => {
            let (t, b) = (|_: f32| 2.0, |_: f32| 39.0);
            fill(12.0, 28.0, &t, &b, &METAL);
            outline(12.0, 28.0, &t, &b);
            let top = |_: f32| 40.0;
            let bot = |x: f32| 87.0 + 11.0 * (1.0 - (x - 20.0).abs() / 8.0);
            fill(12.0, 28.0, &top, &bot, &GOLD);
            flutes(12.0, 28.0, &top, &bot);
            painter.add(Shape::line(vec![p(12.0, 87.0), p(20.0, 91.5), p(28.0, 87.0)], Stroke::new((0.6 * k).max(0.5), Color32::from_rgb(58, 44, 14))));
            outline(12.0, 28.0, &top, &bot);
            cutting = rr(12.0, 40.0, 16.0, 58.0);
        }
    }
    (cutting, rr(11.0, 1.0, 18.0, 97.0))
}
