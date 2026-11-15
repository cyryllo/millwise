//! V-carving: frez V prowadzony po osi kształtu ze zmienną głębokością.
//!
//! Dla każdego punktu brzegu szukamy największego koła wpisanego w kształt,
//! stycznego do brzegu w tym punkcie (środek na normalnej do wnętrza). Koło
//! styczne w `p` i przechodzące przez inny punkt brzegu `q` ma promień
//! `|q − p|² / (2 (q − p)·n)`; najmniejszy taki promień to koło wpisane.
//! Środek koła to miejsce, gdzie stożek freza dotyka obu ścianek; głębokość
//! = promień / tg(połowy kąta) (minus czubek freza), ograniczona głębokością
//! operacji. Ścieżka idzie przez środki kół w kolejności punktów brzegu.

use crate::operation::{CutParams, OpResult, VCarveParams};
use crate::path::z_levels;
use crate::tool::{Tool, ToolKind};
use crate::toolpath::P3;
use cam_geom::{Contour, Vec2};

/// Punkt brzegu z normalną do wnętrza kształtu.
struct Sample {
    p: Vec2,
    n: Vec2,
}

/// Punkty co `step` mm wzdłuż zamkniętych konturów, z normalnymi do wnętrza
/// (wnętrze = parzysto-nieparzyste: litera tak, jej otwór nie).
fn samples(contours: &[&Contour], step: f64) -> Vec<Vec<Sample>> {
    contours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p0 = c.start().unwrap_or_default();
            let depth = contours.iter().enumerate().filter(|(j, o)| *j != i && o.contains_point(p0)).count();
            let mut cc = (*c).clone();
            // kontur „zewnętrzny” CCW, otwór CW — wnętrze zawsze po lewej
            if (cc.signed_area() > 0.0) != (depth % 2 == 0) {
                cc.reverse();
            }
            let pts = cc.flatten(step * 0.1);
            let mut out = Vec::new();
            let n = pts.len();
            if n < 3 {
                return out;
            }
            for k in 0..n {
                let (a, b) = (pts[k], pts[(k + 1) % n]);
                let len = a.distance(b);
                if len < 1e-9 {
                    continue;
                }
                let dir = (b - a) * (1.0 / len);
                let parts = (len / step).ceil().max(1.0) as usize;
                for s in 0..parts {
                    let t = s as f64 / parts as f64;
                    out.push(Sample { p: a.lerp(b, t), n: dir.perp() });
                }
            }
            // normalna w wierzchołkach łamanej: średnia sąsiednich odcinków
            let m = out.len();
            let smoothed: Vec<Vec2> = (0..m)
                .map(|k| {
                    let prev = out[(k + m - 1) % m].n;
                    let cur = out[k].n;
                    if prev.dot(cur) < 0.999 && out[k].p.distance(out[(k + m - 1) % m].p) < step * 1.01 { (prev + cur).normalized() } else { cur }
                })
                .collect();
            for (s, n) in out.iter_mut().zip(smoothed) {
                s.n = n;
            }
            out
        })
        .collect()
}

/// Promień największego koła wpisanego stycznego w `s` (najwyżej `r_max`).
fn inscribed_radius(s: &Sample, all: &[Vec2], r_max: f64) -> f64 {
    let mut r = r_max;
    for &q in all {
        let d = q - s.p;
        let along = d.dot(s.n);
        if along <= 1e-9 {
            continue;
        }
        let d2 = d.length_sq();
        if d2 < 1e-12 {
            continue;
        }
        // szybkie odrzucenie: punkt dalej niż 2r od p nie zmniejszy promienia
        if d2 > 4.0 * r * r {
            continue;
        }
        let rq = d2 / (2.0 * along);
        if rq < r {
            r = rq;
        }
    }
    r
}

pub(crate) fn generate(cut: &CutParams, p: &VCarveParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let (angle, tip) = match tool.kind {
        ToolKind::VBit { angle, tip_diameter } | ToolKind::Engraver { angle, tip_diameter } => (angle, tip_diameter),
        _ => {
            res.warnings.push("V-carving wymaga freza V albo grawerskiego (stożkowego)".into());
            return;
        }
    };
    let closed: Vec<&Contour> = contours.iter().filter(|c| c.closed && c.verts.len() >= 2).collect();
    if closed.len() < contours.len() {
        res.warnings.push("V-carving: pominięto kontury otwarte".into());
    }
    if closed.is_empty() {
        return;
    }
    let tan_half = (angle.clamp(1.0, 179.0) * 0.5).to_radians().tan();
    // głębokość → promień koła wpisanego: r = tip/2 + głębokość · tg(połowy kąta)
    let r_cap = tip * 0.5 + cut.depth * tan_half;
    let step = p.step.clamp(0.02, 2.0);
    let sets = samples(&closed, step);
    let all: Vec<Vec2> = sets.iter().flatten().map(|s| s.p).collect();
    let mut flat = false;

    // ścieżki: (środek, głębokość) dla każdego konturu
    let paths: Vec<Vec<(Vec2, f64)>> = sets
        .iter()
        .map(|set| {
            set.iter()
                .map(|s| {
                    let r = inscribed_radius(s, &all, r_cap);
                    if r >= r_cap - 1e-9 {
                        flat = true;
                    }
                    let depth = ((r - tip * 0.5).max(0.0) / tan_half).min(cut.depth);
                    (s.p + s.n * r, depth)
                })
                .collect()
        })
        .collect();
    if flat && p.warn_flat {
        res.warnings.push(format!(
            "Kształt szerszy niż frez na głębokości {} mm — środek zostaje płaski (do wybrania kieszenią)",
            cut.depth
        ));
    }

    let tp = &mut res.toolpath;
    let levels = z_levels(cut.top_z, cut.bottom_z(), cut.step_down);
    for path in paths.iter().filter(|p| p.len() >= 2) {
        for &level in &levels {
            let z_at = |d: f64| (cut.top_z - d).max(level);
            let pts: Vec<P3> = path.iter().chain(std::iter::once(&path[0])).map(|&(c, d)| P3::from_xy(c, z_at(d))).collect();
            let pts = simplify(&pts, SIMPLIFY_TOL);
            tp.travel_to(P3 { z: cut.top_z + cut.retract_clearance, ..pts[0] }, cut.safe_z);
            tp.feed(pts[0], cut.plunge_feed);
            for &q in &pts[1..] {
                tp.feed(q, cut.feed);
            }
            tp.retract(cut.top_z + cut.retract_clearance);
        }
    }
}

/// Tolerancja upraszczania ścieżki [mm]: punkty bliżej prostej niż tyle są pomijane.
const SIMPLIFY_TOL: f64 = 0.005;

/// Upraszczanie łamanej 3D (Douglas–Peucker): mniej odcinków przy prostych
/// fragmentach — krótszy G-code i szybsza symulacja, kształt bez zmian w granicy tolerancji.
fn simplify(pts: &[P3], tol: f64) -> Vec<P3> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let ab = [pb.x - pa.x, pb.y - pa.y, pb.z - pa.z];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let mut worst = (0.0, a);
        for (k, q) in pts.iter().enumerate().take(b).skip(a + 1) {
            let aq = [q.x - pa.x, q.y - pa.y, q.z - pa.z];
            let t = if l2 > 0.0 { ((aq[0] * ab[0] + aq[1] * ab[1] + aq[2] * ab[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let d = ((aq[0] - ab[0] * t).powi(2) + (aq[1] - ab[1] * t).powi(2) + (aq[2] - ab[2] * t).powi(2)).sqrt();
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use crate::operation::*;
    use crate::tool::{Tool, ToolKind};
    use cam_geom::{Contour, Vertex};

    fn rect(w: f64, h: f64) -> Contour {
        Contour {
            verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(w, 0.0, 0.0), Vertex::new(w, h, 0.0), Vertex::new(0.0, h, 0.0)],
            closed: true,
        }
    }

    fn vbit(angle: f64) -> Tool {
        Tool { diameter: 12.0, kind: ToolKind::VBit { angle, tip_diameter: 0.0 }, ..Default::default() }
    }

    fn op(depth: f64) -> Operation {
        Operation {
            side: Side::A,
            name: "v".into(),
            enabled: true,
            kind: OpKind::VCarve(VCarveParams::default()),
            tool_id: 1,
            cut: CutParams { depth, step_down: depth, ..Default::default() },
            contours: vec![0],
        }
    }

    #[test]
    fn depth_follows_the_width_and_never_cuts_past_the_edge() {
        // prostokąt 20 × 10, frez 90°: na osi głębokość = połowa szerokości = 5 mm
        let res = generate(&op(10.0), &vbit(90.0), &[rect(20.0, 10.0)]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let pts: Vec<_> = res.toolpath.polyline(0.01).into_iter().filter(|(p, r)| !r && p.z < 0.0).map(|(p, _)| p).collect();
        let deepest = pts.iter().map(|p| p.z).fold(0.0, f64::min);
        assert!((deepest + 5.0).abs() < 0.05, "najgłębiej {deepest}");
        // stożek na powierzchni ma promień = głębokość (90°) — nigdy nie wychodzi poza prostokąt
        for p in &pts {
            let r_top = -p.z;
            let edge = p.x.min(20.0 - p.x).min(p.y).min(10.0 - p.y);
            assert!(r_top <= edge + 0.05, "wcina się poza krawędź: {p:?}");
        }
        // narożniki: płytko (ostry narożnik → głębokość prawie 0)
        assert!(pts.iter().any(|p| p.x < 0.3 && p.y < 0.3 && p.z > -0.3));
    }

    #[test]
    fn depth_limit_makes_a_flat_bottom_and_warns() {
        let res = generate(&op(2.0), &vbit(90.0), &[rect(20.0, 10.0)]);
        let deepest = res.toolpath.moves.iter().map(|m| m.end().z).fold(0.0, f64::min);
        assert!((deepest + 2.0).abs() < 1e-9);
        assert!(res.warnings.iter().any(|w| w.contains("płaski")), "{:?}", res.warnings);
    }

    #[test]
    fn hole_in_a_letter_is_left_standing() {
        // „O”: kwadrat 20 z otworem 10 w środku — pierścień szerokości 5 (na bokach głębokość 2,5),
        // najszerzej w narożnikach: koło styczne do dwóch ścian zewnętrznych i rogu otworu,
        // r = 5√2 / (1 + √2) ≈ 2,929
        let outer = rect(20.0, 20.0);
        let mut hole = Contour {
            verts: vec![Vertex::new(5.0, 5.0, 0.0), Vertex::new(15.0, 5.0, 0.0), Vertex::new(15.0, 15.0, 0.0), Vertex::new(5.0, 15.0, 0.0)],
            closed: true,
        };
        hole.reverse();
        let mut o = op(10.0);
        o.contours = vec![0, 1];
        let res = generate(&o, &vbit(90.0), &[outer, hole]);
        let pts: Vec<_> = res.toolpath.polyline(0.01).into_iter().filter(|(p, r)| !r && p.z < 0.0).map(|(p, _)| p).collect();
        let deepest = pts.iter().map(|p| p.z).fold(0.0, f64::min);
        let r = 5.0 * 2f64.sqrt() / (1.0 + 2f64.sqrt());
        assert!((deepest + r).abs() < 0.05, "{deepest}");
        // na środku boku pierścienia: 2,5
        // odcinki zagęszczone co 0,1 mm (proste fragmenty są uproszczone do jednego odcinka)
        let mut dense = Vec::new();
        for w in pts.windows(2) {
            let n = (w[0].distance(w[1]) / 0.1).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                dense.push(crate::toolpath::P3::new(
                    w[0].x + (w[1].x - w[0].x) * t,
                    w[0].y + (w[1].y - w[0].y) * t,
                    w[0].z + (w[1].z - w[0].z) * t,
                ));
            }
        }
        let mid = dense.iter().filter(|p| (p.x - 10.0).abs() < 0.2 && p.y < 5.0).map(|p| p.z).fold(0.0, f64::min);
        assert!((mid + 2.5).abs() < 0.05, "{mid}");
        // nic nie jest cięte w środku otworu
        assert!(pts.iter().all(|p| !(p.x > 5.1 && p.x < 14.9 && p.y > 5.1 && p.y < 14.9)));
    }

    #[test]
    fn needs_a_v_tool() {
        let res = generate(&op(2.0), &Tool { diameter: 6.0, ..Default::default() }, &[rect(20.0, 10.0)]);
        assert!(res.toolpath.moves.is_empty() || res.warnings.iter().any(|w| w.contains("V")));
        assert!(!res.warnings.is_empty());
    }
}

