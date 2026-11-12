//! Generuje samples/test.dxf: płytka z zaokrągleniami (LINE+ARC),
//! otwory (CIRCLE), kieszeń z wyspą (LWPOLYLINE z bulge), rowek (SPLINE)
//! i otwarta ścieżka grawerowania.
//!
//! cargo run -p cam-io --example make_sample

use dxf::entities::*;
use dxf::enums::{AcadVersion, Units};
use dxf::{Drawing, LwPolylineVertex, Point};

fn ent(t: EntityType, layer: &str) -> Entity {
    let mut e = Entity::new(t);
    e.common.layer = layer.into();
    e
}

fn lw(pts: &[(f64, f64, f64)], closed: bool) -> EntityType {
    let mut p = LwPolyline::default();
    p.vertices = pts.iter().map(|&(x, y, b)| LwPolylineVertex { x, y, bulge: b, ..Default::default() }).collect();
    p.set_is_closed(closed);
    EntityType::LwPolyline(p)
}

fn main() -> anyhow::Result<()> {
    let mut d = Drawing::new();
    d.header.version = AcadVersion::R2000;
    d.header.default_drawing_units = Units::Millimeters;

    // obrys 160 x 100, R8, rozbity na odcinki i łuki
    let (w, h, r) = (160.0, 100.0, 8.0);
    let l = |a: (f64, f64), b: (f64, f64)| EntityType::Line(Line::new(Point::new(a.0, a.1, 0.0), Point::new(b.0, b.1, 0.0)));
    let a = |cx: f64, cy: f64, s: f64, e: f64| EntityType::Arc(Arc::new(Point::new(cx, cy, 0.0), r, s, e));
    for e in [
        l((r, 0.0), (w - r, 0.0)), a(w - r, r, 270.0, 0.0),
        l((w, r), (w, h - r)), a(w - r, h - r, 0.0, 90.0),
        l((w - r, h), (r, h)), a(r, h - r, 90.0, 180.0),
        l((0.0, h - r), (0.0, r)), a(r, r, 180.0, 270.0),
    ] {
        d.add_entity(ent(e, "OBRYS"));
    }

    // otwory montażowe
    for (x, y) in [(12.0, 12.0), (w - 12.0, 12.0), (12.0, h - 12.0), (w - 12.0, h - 12.0)] {
        d.add_entity(ent(EntityType::Circle(Circle::new(Point::new(x, y, 0.0), 3.25)), "OTWORY"));
    }

    // kieszeń: prostokąt z zaokrąglonymi narożnikami (bulge) + wyspa (okrąg)
    let b = (std::f64::consts::PI / 8.0).tan(); // ćwiartka
    d.add_entity(ent(
        lw(&[(35.0, 25.0, 0.0), (85.0, 25.0, b), (90.0, 30.0, 0.0), (90.0, 70.0, b), (85.0, 75.0, 0.0),
             (35.0, 75.0, b), (30.0, 70.0, 0.0), (30.0, 30.0, b)], true),
        "KIESZEN",
    ));
    d.add_entity(ent(EntityType::Circle(Circle::new(Point::new(60.0, 50.0, 0.0), 10.0)), "KIESZEN"));

    // fasolka (slot) z dwóch łuków i dwóch odcinków
    d.add_entity(ent(lw(&[(105.0, 30.0, 0.0), (135.0, 30.0, 1.0), (135.0, 42.0, 0.0), (105.0, 42.0, 1.0)], true), "KIESZEN"));

    // splajn — falisty rowek
    let mut s = Spline::default();
    s.degree_of_curve = 3;
    s.control_points = [(105.0, 60.0), (113.0, 80.0), (121.0, 55.0), (129.0, 80.0), (137.0, 60.0), (145.0, 78.0)]
        .iter().map(|&(x, y)| Point::new(x, y, 0.0)).collect();
    s.knot_values = vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0];
    d.add_entity(ent(EntityType::Spline(s), "GRAWER"));

    // otwarta łamana grawerowania
    d.add_entity(ent(lw(&[(35.0, 88.0, 0.0), (55.0, 88.0, 0.0), (60.0, 92.0, 0.0), (90.0, 92.0, 0.0)], false), "GRAWER"));

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/test.dxf");
    d.save_file(&out)?;
    println!("zapisano {}", out.display());
    Ok(())
}
