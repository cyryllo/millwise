//! Profil: obróbka po obrysie z korekcją promienia narzędzia (na zewnątrz,
//! wewnątrz lub po linii), wiele przejść w Z, rampa, mostki.

use crate::operation::{order_inside_first, ramp_length, CutParams, Direction, Entry, OpResult, ProfileParams, ProfileSide};
use crate::path::{z_levels, PathWalk, WalkSpec};
use crate::tool::Tool;
use crate::toolpath::{Toolpath, P3};
use cam_geom::Contour;
use cavalier_contours::polyline::PlineSource;

pub(crate) fn generate(cut: &CutParams, p: &ProfileParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let r = tool.radius();
    let levels = z_levels(cut.top_z, cut.bottom_z(), cut.step_down);
    let mut open_warned = false;

    for i in order_inside_first(contours) {
        let c = &contours[i];
        if c.verts.len() < 2 {
            continue;
        }
        let loops: Vec<Contour> = if !c.closed || p.side == ProfileSide::On {
            if !c.closed && p.side != ProfileSide::On && !open_warned {
                res.warnings.push("Kontury otwarte obrabiane po linii (bez korekcji promienia)".into());
                open_warned = true;
            }
            vec![c.clone()]
        } else {
            let mut cc = c.clone();
            if cc.signed_area() < 0.0 {
                cc.reverse();
            }
            let off = r + p.stock_to_leave;
            // cavalier: dodatni offset konturu CCW = do środka
            let d = if p.side == ProfileSide::Outside { -off } else { off };
            let out: Vec<Contour> = cc.to_cavc().parallel_offset(d).iter().map(Contour::from_cavc).collect();
            if out.is_empty() {
                res.warnings.push(format!("Kontur #{}: narzędzie Ø{} nie mieści się w środku", i + 1, tool.diameter));
            }
            out
        };

        for mut lp in loops {
            if lp.closed {
                let ccw = lp.signed_area() > 0.0;
                // przy M3: współbieżne na zewnątrz = CW, wewnątrz = CCW
                let want_ccw = match (p.side, p.direction) {
                    (ProfileSide::Inside, Direction::Climb) => true,
                    (ProfileSide::Inside, Direction::Conventional) => false,
                    (_, Direction::Climb) => false,
                    (_, Direction::Conventional) => true,
                };
                if ccw != want_ccw {
                    lp.reverse();
                }
                let tabs = p.tabs.as_ref().filter(|t| t.count > 0 && t.height > 0.0 && t.height < cut.depth);
                cut_closed(&mut res.toolpath, &lp, &levels, cut, p.entry, tabs.map(|t| (t, tool.diameter)), &mut res.warnings);
            } else {
                cut_open(&mut res.toolpath, &lp, &levels, cut);
            }
        }
    }
}

/// Przejścia po zamkniętym konturze: na każdym poziomie rampa (lub wejście
/// pionowe), potem pełne okrążenie. Narzędzie nie podnosi się między poziomami.
pub(crate) fn cut_closed(
    tp: &mut Toolpath,
    lp: &Contour,
    levels: &[f64],
    cut: &CutParams,
    entry: Entry,
    tabs: Option<(&crate::operation::Tabs, f64)>,
    warnings: &mut Vec<String>,
) {
    let walk = PathWalk::new(lp);
    if walk.is_empty() {
        return;
    }
    let mut tab_ranges: Vec<(f64, f64)> = Vec::new();
    let mut tab_z = f64::NEG_INFINITY;
    if let Some((t, dia)) = tabs {
        let n = t.count as f64;
        let w = t.width + dia;
        if walk.total < n * w * 1.5 {
            warnings.push(format!("Kontur za krótki na {} mostki — pominięto", t.count));
        } else {
            for k in 0..t.count {
                let c = walk.total * (k as f64 + 0.5) / n;
                tab_ranges.push((c - w * 0.5, c + w * 0.5));
            }
            tab_z = cut.bottom_z() + t.height;
        }
    }

    let start = walk.point_at(0.0);
    tp.travel_to(P3::from_xy(start, cut.top_z + cut.retract_clearance), cut.safe_z);
    let mut s_pos = 0.0;
    let mut prev = cut.top_z;
    for &z in levels {
        let ramp = ramp_length(entry, prev - z);
        let l = ramp.unwrap_or(0.0);
        let z_of = move |t: f64| match ramp {
            Some(l) if l > 0.0 => prev - (prev - z) * (t / l).min(1.0),
            _ => z,
        };
        walk.emit(
            tp,
            &WalkSpec {
                s_start: s_pos,
                travel: l + walk.total,
                z_of: &z_of,
                tabs: &tab_ranges,
                tab_z,
                feed: cut.feed,
                plunge_feed: cut.plunge_feed,
                ramp_end: l,
                reverse: false,
            },
        );
        s_pos = (s_pos + l + walk.total).rem_euclid(walk.total);
        prev = z;
    }
}

/// Przejścia po otwartym konturze: kolejne poziomy na zmianę w przód i w tył.
pub(crate) fn cut_open(tp: &mut Toolpath, lp: &Contour, levels: &[f64], cut: &CutParams) {
    let walk = PathWalk::new(lp);
    if walk.is_empty() {
        return;
    }
    let start = walk.point_at(0.0);
    tp.travel_to(P3::from_xy(start, cut.top_z + cut.retract_clearance), cut.safe_z);
    for (k, &z) in levels.iter().enumerate() {
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
                reverse: k % 2 == 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::operation::*;
    use crate::tool::Tool;
    use crate::toolpath::Move;
    use cam_geom::{Contour, Vec2, Vertex};

    fn square(s: f64) -> Contour {
        Contour {
            verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(s, 0.0, 0.0), Vertex::new(s, s, 0.0), Vertex::new(0.0, s, 0.0)],
            closed: true,
        }
    }

    fn op(side: ProfileSide) -> Operation {
        Operation {
            name: "p".into(),
            enabled: true,
            kind: OpKind::Profile(ProfileParams { side, entry: Entry::Plunge, ..Default::default() }),
            tool_id: 1,
            cut: CutParams { depth: 4.0, step_down: 2.0, ..Default::default() },
            contours: vec![0],
        }
    }

    fn xy_extent(tp: &crate::Toolpath, z_max: f64) -> (Vec2, Vec2) {
        let mut mn = Vec2::new(f64::MAX, f64::MAX);
        let mut mx = Vec2::new(f64::MIN, f64::MIN);
        for (p, rapid) in tp.polyline(0.001) {
            if !rapid && p.z <= z_max {
                mn = Vec2::new(mn.x.min(p.x), mn.y.min(p.y));
                mx = Vec2::new(mx.x.max(p.x), mx.y.max(p.y));
            }
        }
        (mn, mx)
    }

    #[test]
    fn outside_profile_offsets_by_radius() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let res = generate(&op(ProfileSide::Outside), &tool, &[square(20.0)]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let (mn, mx) = xy_extent(&res.toolpath, -1.0);
        assert!((mn.x + 3.0).abs() < 1e-6 && (mx.x - 23.0).abs() < 1e-6, "{mn:?} {mx:?}");
        // dwa poziomy: -2 i -4
        let min_z = res.toolpath.moves.iter().map(|m| m.end().z).fold(f64::MAX, f64::min);
        assert!((min_z + 4.0).abs() < 1e-9);
        // zewnętrzne narożniki to łuki
        assert!(res.toolpath.moves.iter().any(|m| matches!(m, Move::Arc { ccw: false, .. })));
        // kończy na wysokości bezpiecznej
        assert!((res.toolpath.pos().unwrap().z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn inside_profile_offsets_inward() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let res = generate(&op(ProfileSide::Inside), &tool, &[square(20.0)]);
        let (mn, mx) = xy_extent(&res.toolpath, -1.0);
        assert!((mn.x - 3.0).abs() < 1e-6 && (mx.x - 17.0).abs() < 1e-6, "{mn:?} {mx:?}");
    }

    #[test]
    fn tool_too_big_warns() {
        let tool = Tool { diameter: 30.0, ..Default::default() };
        let res = generate(&op(ProfileSide::Inside), &tool, &[square(20.0)]);
        assert!(!res.warnings.is_empty());
    }
}
