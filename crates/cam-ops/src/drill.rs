//! Wiercenie w środkach okręgów, opcjonalnie z wycofaniem (peck).

use crate::operation::{CutParams, DrillParams, OpResult};
use crate::tool::Tool;
use crate::toolpath::P3;
use cam_geom::{Contour, Vec2};

pub(crate) fn generate(cut: &CutParams, p: &DrillParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let mut pts: Vec<Vec2> = Vec::new();
    let mut radii: Vec<f64> = Vec::new();
    let mut skipped = 0;
    let mut too_big = 0;
    for c in contours {
        match c.as_circle() {
            Some((center, r)) => {
                if !p.helix && 2.0 * r > tool.diameter * 1.05 {
                    too_big += 1;
                }
                pts.push(center);
                radii.push(r);
            }
            None => skipped += 1,
        }
    }
    if skipped > 0 {
        res.warnings.push(format!("Wiercenie: pominięto {skipped} kontur(ów), które nie są okręgami"));
    }
    if too_big > 0 {
        res.warnings.push(format!(
            "{too_big} otwór(ów) większych niż narzędzie Ø{} — rozważ kieszeń lub profil wewnętrzny",
            tool.diameter
        ));
    }

    // najbliższy sąsiad
    let mut order: Vec<(Vec2, f64)> = Vec::with_capacity(pts.len());
    let mut pos = Vec2::ZERO;
    let mut left: Vec<(Vec2, f64)> = pts.into_iter().zip(radii).collect();
    while !left.is_empty() {
        let (k, _) = left
            .iter()
            .enumerate()
            .map(|(k, q)| (k, q.0.distance(pos)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        let item = left.swap_remove(k);
        pos = item.0;
        order.push(item);
    }

    if p.helix {
        helix_holes(cut, tool, &order, res);
        return;
    }

    let tp = &mut res.toolpath;
    let r_plane = cut.top_z + cut.retract_clearance;
    let bottom = cut.bottom_z();
    for (q, _) in order {
        tp.travel_to(P3::from_xy(q, r_plane), cut.safe_z);
        let mut z = cut.top_z;
        loop {
            let z_next = match p.peck {
                Some(peck) if peck > 0.0 => (z - peck).max(bottom),
                _ => bottom,
            };
            tp.feed(P3::from_xy(q, z_next), cut.plunge_feed);
            if z_next <= bottom + 1e-9 {
                break;
            }
            // wycofanie dla usunięcia wiórów i powrót tuż nad dno
            tp.rapid(P3::from_xy(q, r_plane));
            tp.rapid(P3::from_xy(q, z_next + 0.3));
            z = z_next;
        }
        tp.rapid(P3::from_xy(q, r_plane));
    }
}

/// Frezowanie otworów po helisie: zejście po spirali o skoku = krok w dół,
/// pełny okrąg na dnie, powrót do środka.
fn helix_holes(cut: &CutParams, tool: &Tool, holes: &[(Vec2, f64)], res: &mut OpResult) {
    let r_tool = tool.radius();
    let bottom = cut.bottom_z();
    let mut small = 0;
    let mut core = 0;
    for &(c, r_hole) in holes {
        let rp = r_hole - r_tool;
        let tp = &mut res.toolpath;
        if rp < 0.05 {
            // otwór nie większy niż narzędzie — wejście pionowe
            small += 1;
            tp.travel_to(P3::from_xy(c, cut.top_z + cut.retract_clearance), cut.safe_z);
            tp.feed(P3::from_xy(c, bottom), cut.plunge_feed);
            tp.rapid(P3::from_xy(c, cut.top_z + cut.retract_clearance));
            continue;
        }
        if rp > r_tool {
            core += 1;
        }
        let a = c + Vec2::new(rp, 0.0);
        let b = c - Vec2::new(rp, 0.0);
        tp.travel_to(P3::from_xy(a, cut.top_z + cut.retract_clearance), cut.safe_z);
        tp.feed(P3::from_xy(a, cut.top_z), cut.plunge_feed);
        let depth = cut.top_z - bottom;
        let turns = (depth / cut.step_down.max(0.01) - 1e-9).ceil().max(1.0) as usize;
        let dz = depth / turns as f64;
        let mut z = cut.top_z;
        for _ in 0..turns {
            // współbieżnie w otworze = CCW
            tp.arc(P3::from_xy(b, z - dz * 0.5), c, true, cut.feed);
            tp.arc(P3::from_xy(a, z - dz), c, true, cut.feed);
            z -= dz;
        }
        tp.arc(P3::from_xy(b, bottom), c, true, cut.feed);
        tp.arc(P3::from_xy(a, bottom), c, true, cut.feed);
        tp.feed(P3::from_xy(c, bottom), cut.feed);
        tp.rapid(P3::from_xy(c, cut.top_z + cut.retract_clearance));
    }
    if small > 0 {
        res.warnings.push(format!("{small} otwór(ów) nie większych niż narzędzie — wejście pionowe"));
    }
    if core > 0 {
        res.warnings.push(format!(
            "{core} otwór(ów) ponad 2× większych od narzędzia — w środku zostanie rdzeń (przy przelotowych odpadnie)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use crate::operation::*;
    use crate::tool::Tool;
    use cam_geom::{Contour, Vec2};

    #[test]
    fn helix_hole_stays_on_radius() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let op = Operation {
            name: "h".into(),
            enabled: true,
            kind: OpKind::Drill(DrillParams { peck: None, helix: true }),
            tool_id: 1,
            cut: CutParams { depth: 5.0, step_down: 1.0, ..Default::default() },
            contours: vec![0],
        };
        let res = generate(&op, &tool, &[Contour::circle(Vec2::new(10.0, 10.0), 5.0)]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let pts = res.toolpath.polyline(0.001);
        let on_helix: Vec<_> = pts.iter().filter(|(p, r)| !r && p.z < -0.01 && p.z > -4.99).collect();
        assert!(!on_helix.is_empty());
        for (p, _) in on_helix {
            let d = ((p.x - 10.0).powi(2) + (p.y - 10.0).powi(2)).sqrt();
            assert!((d - 2.0).abs() < 1e-6, "promień ścieżki {d}");
        }
        let min_z = pts.iter().map(|(p, _)| p.z).fold(f64::MAX, f64::min);
        assert!((min_z + 5.0).abs() < 1e-9);
    }
}
