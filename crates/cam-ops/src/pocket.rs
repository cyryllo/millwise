//! Kieszeń: ścieżki równoległe do konturu (offsetowe) od środka na zewnątrz,
//! z obsługą wysp. Kontury zagnieżdżone na zmianę: granica / wyspa / granica…

use crate::operation::{ramp_length, CutParams, Direction, Entry, OpResult, PocketParams, PocketStrategy};
use crate::path::{z_levels, PathWalk, WalkSpec};
use crate::tool::Tool;
use crate::toolpath::P3;
use cam_geom::{Contour, Vec2};
use cavalier_contours::polyline::Polyline;
use cavalier_contours::shape_algorithms::{Shape, ShapeOffsetOptions};

struct Loop {
    contour: Contour,
    level: usize,
    parent: Option<usize>,
    children: usize,
}

pub(crate) fn generate(cut: &CutParams, p: &PocketParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let closed: Vec<&Contour> = contours.iter().filter(|c| c.closed && c.verts.len() >= 2).collect();
    if closed.len() < contours.len() {
        res.warnings.push("Kieszeń: pominięto kontury otwarte".into());
    }
    if closed.is_empty() {
        return;
    }

    // orientacja wg zagnieżdżenia: parzyste = granica (CCW), nieparzyste = wyspa (CW)
    let plines: Vec<Polyline<f64>> = closed
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p0 = c.start().unwrap();
            let depth = closed.iter().enumerate().filter(|(j, o)| *j != i && o.contains_point(p0)).count();
            let mut cc = (*c).clone();
            if (cc.signed_area() > 0.0) != (depth % 2 == 0) {
                cc.reverse();
            }
            cc.to_cavc()
        })
        .collect();
    let shape = Shape::from_plines(plines);
    let opts = ShapeOffsetOptions::new();

    let stepover = p.stepover.clamp(0.05, 1.0) * tool.diameter;
    let mut by_level: Vec<Vec<Contour>> = Vec::new();
    let mut d = tool.radius() + p.stock_to_leave;
    while by_level.len() < 10_000 {
        let sh = shape.parallel_offset(d, &opts);
        let loops: Vec<Contour> = sh
            .ccw_plines
            .iter()
            .chain(sh.cw_plines.iter())
            .map(|ip| Contour::from_cavc(&ip.polyline))
            .filter(|c| c.verts.len() >= 2)
            .collect();
        if loops.is_empty() {
            break;
        }
        by_level.push(loops);
        d += stepover;
    }
    if by_level.is_empty() {
        res.warnings.push(format!("Narzędzie Ø{} nie mieści się w kieszeni", tool.diameter));
        return;
    }

    if p.strategy == PocketStrategy::Zigzag {
        let mut boundary = by_level[0].clone();
        if p.direction == Direction::Conventional {
            boundary.iter_mut().for_each(|c| c.reverse());
        }
        zigzag(cut, p, &boundary, stepover, res);
        return;
    }

    // drzewo: rodzic pętli z poziomu k+1 to najbliższa pętla z poziomu k
    let mut loops: Vec<Loop> = Vec::new();
    let mut level_start = Vec::new();
    for (lvl, ls) in by_level.iter().enumerate() {
        level_start.push(loops.len());
        for c in ls {
            let mut c = c.clone();
            if p.direction == Direction::Conventional {
                c.reverse();
            }
            loops.push(Loop { contour: c, level: lvl, parent: None, children: 0 });
        }
    }
    for i in 0..loops.len() {
        let lvl = loops[i].level;
        if lvl == 0 {
            continue;
        }
        let p0 = loops[i].contour.start().unwrap();
        let range = level_start[lvl - 1]..level_start[lvl];
        let parent = range
            .map(|j| (j, loops[j].contour.distance_to(p0)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|x| x.0);
        loops[i].parent = parent;
        if let Some(pj) = parent {
            loops[pj].children += 1;
        }
    }

    // kolejność: pętla dostępna, gdy wszystkie jej dzieci są zrobione; najbliższa najpierw
    let mut remaining_children: Vec<usize> = loops.iter().map(|l| l.children).collect();
    let mut done = vec![false; loops.len()];
    let mut order = Vec::with_capacity(loops.len());
    let mut pos = Vec2::ZERO;
    for _ in 0..loops.len() {
        let next = (0..loops.len())
            .filter(|&i| !done[i] && remaining_children[i] == 0)
            .map(|i| (i, loops[i].contour.distance_to(pos)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|x| x.0);
        let Some(i) = next else { break };
        done[i] = true;
        if let Some(pj) = loops[i].parent {
            remaining_children[pj] -= 1;
        }
        pos = loops[i].contour.closest_point(pos).map(|x| x.1).unwrap_or(pos);
        order.push(i);
    }

    // granica obszaru dozwolonego dla środka narzędzia — do sprawdzania przejść
    let boundary: Vec<Vec<Vec2>> = by_level[0].iter().map(|c| c.flatten(0.01)).collect();
    let link_max = stepover * 1.6 + 0.01;

    let tp = &mut res.toolpath;
    let mut prev = cut.top_z;
    for z in z_levels(cut.top_z, cut.bottom_z(), cut.step_down) {
        let mut at_depth = false;
        for &i in &order {
            let mut lp = loops[i].contour.clone();
            let cur = tp.pos();
            if let Some(c) = cur {
                if let Some((seg, q)) = lp.closest_point(c.xy()) {
                    lp.rotate_start_to(seg, q);
                }
            }
            let walk = PathWalk::new(&lp);
            if walk.is_empty() {
                continue;
            }
            let start = walk.point_at(0.0);
            let link = at_depth
                && cur.is_some_and(|c| {
                    (c.z - z).abs() < 1e-9 && c.xy().distance(start) <= link_max && !crosses(&boundary, c.xy(), start)
                });
            let ramp = if link {
                tp.feed(P3::from_xy(start, z), cut.feed);
                None
            } else {
                tp.travel_to(P3::from_xy(start, prev + cut.retract_clearance), cut.safe_z);
                ramp_length(p.entry, prev - z)
            };
            let l = ramp.unwrap_or(0.0);
            let z_top = prev;
            let z_of = move |t: f64| match ramp {
                Some(l) if l > 0.0 => z_top - (z_top - z) * (t / l).min(1.0),
                _ => z,
            };
            walk.emit(
                tp,
                &WalkSpec {
                    s_start: 0.0,
                    travel: l + walk.total,
                    z_of: &z_of,
                    tabs: &[],
                    tab_z: f64::NEG_INFINITY,
                    feed: cut.feed,
                    plunge_feed: cut.plunge_feed,
                    ramp_end: l,
                    reverse: false,
                },
            );
            at_depth = true;
        }
        prev = z;
    }
}

/// Czy odcinek a–b przecina którąś z łamanych granicy.
fn crosses(boundary: &[Vec<Vec2>], a: Vec2, b: Vec2) -> bool {
    let eps = 1e-6;
    for poly in boundary {
        let n = poly.len();
        for k in 0..n {
            let (c, d) = (poly[k], poly[(k + 1) % n]);
            let r = b - a;
            let s = d - c;
            let den = r.cross(s);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (c - a).cross(s) / den;
            let u = (c - a).cross(r) / den;
            if t > eps && t < 1.0 - eps && u > -eps && u < 1.0 + eps {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::operation::*;
    use crate::tool::Tool;
    use cam_geom::{Contour, Vec2, Vertex};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
        Contour {
            verts: vec![Vertex::new(x0, y0, 0.0), Vertex::new(x1, y0, 0.0), Vertex::new(x1, y1, 0.0), Vertex::new(x0, y1, 0.0)],
            closed: true,
        }
    }

    fn pocket_op() -> Operation {
        Operation {
            name: "k".into(),
            enabled: true,
            kind: OpKind::Pocket(PocketParams::default()),
            tool_id: 1,
            cut: CutParams { depth: 3.0, step_down: 1.5, ..Default::default() },
            contours: vec![],
        }
    }

    #[test]
    fn pocket_stays_inside_and_avoids_island() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let outer = rect(0.0, 0.0, 60.0, 40.0);
        let island = Contour::circle(Vec2::new(30.0, 20.0), 6.0);
        let res = generate(&pocket_op(), &tool, &[outer, island]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let pts = res.toolpath.polyline(0.01);
        let cutting: Vec<_> = pts.iter().filter(|(p, rapid)| !rapid && p.z < -0.01).collect();
        assert!(!cutting.is_empty());
        for (p, _) in &cutting {
            // środek narzędzia min. r od ścian i od wyspy
            assert!(p.x >= 3.0 - 1e-6 && p.x <= 57.0 + 1e-6 && p.y >= 3.0 - 1e-6 && p.y <= 37.0 + 1e-6, "{p:?}");
            let d = ((p.x - 30.0).powi(2) + (p.y - 20.0).powi(2)).sqrt();
            assert!(d >= 9.0 - 1e-3, "zbyt blisko wyspy: {p:?}");
        }
        let min_z = pts.iter().map(|(p, _)| p.z).fold(f64::MAX, f64::min);
        assert!((min_z + 3.0).abs() < 1e-9);
    }

    #[test]
    fn too_small_pocket_warns() {
        let tool = Tool { diameter: 10.0, ..Default::default() };
        let res = generate(&pocket_op(), &tool, &[rect(0.0, 0.0, 8.0, 8.0)]);
        assert!(!res.warnings.is_empty());
    }
}

/// Kieszeń zygzak: przejścia równoległe w obszarze środka narzędzia (`boundary`),
/// łączone krótkimi przejściami, potem przejście wykańczające po obrysie.
fn zigzag(cut: &CutParams, p: &PocketParams, boundary: &[Contour], stepover: f64, res: &mut OpResult) {
    let ang = p.zigzag_angle.to_radians();
    let (sa, ca) = ang.sin_cos();
    // obrót o -kąt: przejścia stają się poziome
    let rot = |v: Vec2| Vec2::new(ca * v.x + sa * v.y, -sa * v.x + ca * v.y);
    let unrot = |v: Vec2| Vec2::new(ca * v.x - sa * v.y, sa * v.x + ca * v.y);
    let polys: Vec<Vec<Vec2>> = boundary.iter().map(|c| c.flatten(0.01).into_iter().map(rot).collect()).collect();
    let (mut ymin, mut ymax) = (f64::MAX, f64::MIN);
    for poly in &polys {
        for q in poly {
            ymin = ymin.min(q.y);
            ymax = ymax.max(q.y);
        }
    }
    if ymin > ymax {
        return;
    }
    // równe odstępy, pierwszy i ostatni rząd tuż przy krawędzi
    let span = ymax - ymin;
    let n = ((span / stepover).ceil() as usize).max(1);
    let eps = 1e-4;
    let mut rows: Vec<Vec<(f64, f64, f64)>> = Vec::new(); // (y, x0, x1)
    for k in 0..=n {
        let y = (ymin + eps + (span - 2.0 * eps) * k as f64 / n as f64).min(ymax - eps);
        let mut xs: Vec<f64> = Vec::new();
        for poly in &polys {
            let m = poly.len();
            for i in 0..m {
                let (a, b) = (poly[i], poly[(i + 1) % m]);
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        rows.push(xs.chunks_exact(2).filter(|c| c[1] - c[0] > 1e-6).map(|c| (y, c[0], c[1])).collect());
    }

    // łańcuchy: kolejne rzędy łączone, jeśli przejście nie wychodzi poza obszar
    let flat_boundary: Vec<Vec<Vec2>> = boundary.iter().map(|c| c.flatten(0.01)).collect();
    let mut used: Vec<Vec<bool>> = rows.iter().map(|r| vec![false; r.len()]).collect();
    let mut chains: Vec<Vec<Vec2>> = Vec::new();
    for start_row in 0..rows.len() {
        for start_seg in 0..rows[start_row].len() {
            if used[start_row][start_seg] {
                continue;
            }
            let mut chain: Vec<Vec2> = Vec::new();
            let (mut r, mut si, mut forward) = (start_row, start_seg, true);
            loop {
                used[r][si] = true;
                let (y, x0, x1) = rows[r][si];
                let (a, b) = if forward { (x0, x1) } else { (x1, x0) };
                chain.push(unrot(Vec2::new(a, y)));
                chain.push(unrot(Vec2::new(b, y)));
                let end = Vec2::new(b, y);
                let Some(next_row) = rows.get(r + 1) else { break };
                let cand = next_row.iter().enumerate().find(|&(j, &(_, nx0, nx1))| {
                    !used[r + 1][j] && end.x >= nx0 - 1e-6 && end.x <= nx1 + 1e-6
                });
                let Some((j, &(ny, nx0, nx1))) = cand else { break };
                let next_start = Vec2::new(if forward { nx1 } else { nx0 }, ny);
                if crosses(&flat_boundary, unrot(end), unrot(next_start)) {
                    break;
                }
                r += 1;
                si = j;
                forward = !forward;
            }
            chains.push(chain);
        }
    }

    let tp = &mut res.toolpath;
    let mut prev = cut.top_z;
    for z in z_levels(cut.top_z, cut.bottom_z(), cut.step_down) {
        for chain in &chains {
            let (a, b) = (chain[0], chain[1]);
            tp.travel_to(P3::from_xy(a, prev + cut.retract_clearance), cut.safe_z);
            tp.feed(P3::from_xy(a, prev), cut.plunge_feed);
            // rampa tam i z powrotem po pierwszym odcinku
            if let Some(l) = ramp_length(p.entry, prev - z).filter(|_| matches!(p.entry, Entry::Ramp { .. })) {
                let seg = a.distance(b);
                if seg > 1e-6 && l > 1e-6 {
                    let mut cur_z = prev;
                    let mut at_a = true;
                    let mut guard = 0;
                    while cur_z > z + 1e-9 && guard < 1000 {
                        guard += 1;
                        let nz = (cur_z - (prev - z) * seg / l).max(z);
                        let target = if at_a { b } else { a };
                        tp.feed(P3::from_xy(target, nz), cut.feed);
                        cur_z = nz;
                        at_a = !at_a;
                    }
                    if !at_a {
                        tp.feed(P3::from_xy(a, z), cut.feed);
                    }
                }
            }
            tp.feed(P3::from_xy(a, z), cut.plunge_feed);
            for q in &chain[1..] {
                tp.feed(P3::from_xy(*q, z), cut.feed);
            }
        }
        // przejście wykańczające po obrysie
        for c in boundary {
            let mut lp = c.clone();
            if let Some(pos) = tp.pos() {
                if let Some((seg, q)) = lp.closest_point(pos.xy()) {
                    lp.rotate_start_to(seg, q);
                }
            }
            let walk = PathWalk::new(&lp);
            if walk.is_empty() {
                continue;
            }
            tp.travel_to(P3::from_xy(walk.point_at(0.0), z + cut.retract_clearance), cut.safe_z);
            let z_of = move |_t: f64| z;
            walk.emit(
                tp,
                &WalkSpec {
                    s_start: 0.0,
                    travel: walk.total,
                    z_of: &z_of,
                    tabs: &[],
                    tab_z: f64::NEG_INFINITY,
                    feed: cut.feed,
                    plunge_feed: cut.plunge_feed,
                    ramp_end: 0.0,
                    reverse: false,
                },
            );
        }
        prev = z;
    }
}

#[cfg(test)]
mod zigzag_tests {
    use crate::operation::*;
    use crate::tool::Tool;
    use cam_geom::{Contour, Vec2, Vertex};

    #[test]
    fn zigzag_covers_and_stays_inside() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let rect = Contour {
            verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(60.0, 0.0, 0.0), Vertex::new(60.0, 40.0, 0.0), Vertex::new(0.0, 40.0, 0.0)],
            closed: true,
        };
        let island = Contour::circle(Vec2::new(30.0, 20.0), 6.0);
        for angle in [0.0, 30.0] {
            let op = Operation {
                name: "z".into(),
                enabled: true,
                kind: OpKind::Pocket(PocketParams { strategy: PocketStrategy::Zigzag, zigzag_angle: angle, ..Default::default() }),
                tool_id: 1,
                cut: CutParams { depth: 2.0, step_down: 2.0, ..Default::default() },
                contours: vec![],
            };
            let res = generate(&op, &tool, &[rect.clone(), island.clone()]);
            assert!(res.warnings.is_empty(), "{:?}", res.warnings);
            let pts = res.toolpath.polyline(0.01);
            let cutting: Vec<_> = pts.iter().filter(|(p, r)| !r && p.z < -1.0).collect();
            assert!(cutting.len() > 20);
            for (p, _) in &cutting {
                assert!(p.x >= 3.0 - 1e-3 && p.x <= 57.0 + 1e-3 && p.y >= 3.0 - 1e-3 && p.y <= 37.0 + 1e-3, "{p:?}");
                let d = ((p.x - 30.0).powi(2) + (p.y - 20.0).powi(2)).sqrt();
                assert!(d >= 9.0 - 1e-2, "kolizja z wyspą {p:?}");
            }
        }
    }
}
