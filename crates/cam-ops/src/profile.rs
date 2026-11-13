//! Profil: obróbka po obrysie z korekcją promienia narzędzia (na zewnątrz,
//! wewnątrz lub po linii), wiele przejść w Z, rampa, mostki.

use crate::operation::{
    order_inside_first, ramp_length, start_near, CutParams, Direction, Entry, OpResult, ProfileParams, ProfileSide, Tabs,
};
use crate::path::{z_levels, PathWalk, WalkSpec};
use crate::tool::Tool;
use crate::toolpath::{Toolpath, P3};
use cam_geom::{Contour, Vec2};
use cavalier_contours::polyline::PlineSource;

pub(crate) fn generate(cut: &CutParams, p: &ProfileParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let r = tool.radius();
    let levels = z_levels(cut.top_z, cut.bottom_z(), cut.step_down);
    let finish = p.finish.filter(|a| *a > 1e-6);
    let lead = p.lead.filter(|l| *l > 1e-3 && p.side != ProfileSide::On);
    let tabs = p.tabs.as_ref().filter(|t| t.count > 0 && t.height > 0.0 && t.height < cut.depth);
    let mut open_warned = false;

    for i in order_inside_first(contours) {
        let c = &contours[i];
        if c.verts.len() < 2 {
            continue;
        }
        if !c.closed || p.side == ProfileSide::On {
            if !c.closed && p.side != ProfileSide::On && !open_warned {
                res.warnings.push("Kontury otwarte obrabiane po linii (bez korekcji promienia)".into());
                open_warned = true;
            }
            let mut lp = c.clone();
            let pos = res.toolpath.pos().map_or(Vec2::ZERO, |q| q.xy());
            start_near(&mut lp, pos, false);
            if lp.closed {
                orient(&mut lp, p);
                cut_closed(&mut res.toolpath, &lp, &levels, cut, p.entry, tabs.map(|t| (t, tool.diameter)), &mut res.warnings);
            } else {
                cut_open(&mut res.toolpath, &lp, &levels, cut);
            }
            continue;
        }

        let mut cc = c.clone();
        if cc.signed_area() < 0.0 {
            cc.reverse();
        }
        // cavalier: dodatni offset konturu CCW = do środka
        let offset = |extra: f64| -> Vec<Contour> {
            let off = r + p.stock_to_leave + extra;
            let d = if p.side == ProfileSide::Outside { -off } else { off };
            cc.to_cavc().parallel_offset(d).iter().map(Contour::from_cavc).collect()
        };
        let rough = offset(finish.unwrap_or(0.0));
        let finishing = if finish.is_some() { offset(0.0) } else { Vec::new() };
        if rough.is_empty() && finishing.is_empty() {
            res.warnings.push(format!("Kontur #{}: narzędzie Ø{} nie mieści się w środku", i + 1, tool.diameter));
            continue;
        }

        // zgrubnie na kolejnych poziomach, potem wykańczająco na pełnej głębokości
        let bottom = [cut.bottom_z()];
        for (loops, lv, entry) in [(&rough, &levels[..], p.entry), (&finishing, &bottom[..], Entry::Plunge)] {
            for lp in loops {
                let mut lp = lp.clone();
                let pos = res.toolpath.pos().map_or(Vec2::ZERO, |q| q.xy());
                start_near(&mut lp, pos, lead.is_some());
                orient(&mut lp, p);
                let tabs = tabs.map(|t| (t, tool.diameter));
                match lead {
                    Some(lr) => {
                        // odpad: na zewnątrz pętli dla profilu zewnętrznego, w środku dla wewnętrznego
                        let ccw = lp.signed_area() > 0.0;
                        let waste_left = if p.side == ProfileSide::Outside { !ccw } else { ccw };
                        let max_r = if lv.len() == 1 && finish.is_some() { r + finish.unwrap() - 0.05 } else { f64::MAX };
                        cut_closed_lead(&mut res.toolpath, &lp, lv, cut, tabs, lr.min(max_r), waste_left, &mut res.warnings);
                    }
                    None => cut_closed(&mut res.toolpath, &lp, lv, cut, entry, tabs, &mut res.warnings),
                }
            }
        }
    }
}

/// Kierunek pętli: przy M3 współbieżne na zewnątrz = CW, wewnątrz = CCW.
fn orient(lp: &mut Contour, p: &ProfileParams) {
    let ccw = lp.signed_area() > 0.0;
    let want_ccw = match (p.side, p.direction) {
        (ProfileSide::Inside, Direction::Climb) => true,
        (ProfileSide::Inside, Direction::Conventional) => false,
        (_, Direction::Climb) => false,
        (_, Direction::Conventional) => true,
    };
    if ccw != want_ccw {
        lp.reverse();
    }
}

/// Przedziały mostków na pętli (w długości od jej początku) i Z ich wierzchu.
fn tab_ranges(walk: &PathWalk, cut: &CutParams, tabs: Option<(&Tabs, f64)>, warnings: &mut Vec<String>) -> (Vec<(f64, f64)>, f64) {
    let mut ranges: Vec<(f64, f64)> = Vec::new();
    let mut tab_z = f64::NEG_INFINITY;
    if let Some((t, dia)) = tabs {
        let n = t.count as f64;
        let w = t.width + dia;
        if walk.total < n * w * 1.5 {
            warnings.push(format!("Kontur za krótki na {} mostki — pominięto", t.count));
        } else {
            for k in 0..t.count {
                let c = walk.total * (k as f64 + 0.5) / n;
                ranges.push((c - w * 0.5, c + w * 0.5));
            }
            tab_z = cut.bottom_z() + t.height;
        }
    }
    (ranges, tab_z)
}

/// Przejścia po zamkniętym konturze: na każdym poziomie rampa (lub wejście
/// pionowe), potem pełne okrążenie. Narzędzie nie podnosi się między poziomami.
pub(crate) fn cut_closed(
    tp: &mut Toolpath,
    lp: &Contour,
    levels: &[f64],
    cut: &CutParams,
    entry: Entry,
    tabs: Option<(&Tabs, f64)>,
    warnings: &mut Vec<String>,
) {
    let walk = PathWalk::new(lp);
    if walk.is_empty() {
        return;
    }
    let (tab_ranges, tab_z) = tab_ranges(&walk, cut, tabs, warnings);

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

/// Przejścia z wejściem i wyjściem po łuku stycznym. Łuki wejścia i wyjścia
/// leżą na jednym okręgu po stronie odpadu: zejście w Z odbywa się na jego
/// początku (z dala od ścianki), narzędzie dochodzi do ścianki stycznie, robi
/// pełne okrążenie i odchodzi stycznie. Między poziomami wraca przez środek
/// okręgu (w odpadzie). Gdy łuk się nie mieści, promień jest zmniejszany.
#[allow(clippy::too_many_arguments)]
fn cut_closed_lead(
    tp: &mut Toolpath,
    lp: &Contour,
    levels: &[f64],
    cut: &CutParams,
    tabs: Option<(&Tabs, f64)>,
    lead_r: f64,
    waste_left: bool,
    warnings: &mut Vec<String>,
) {
    let walk = PathWalk::new(lp);
    if walk.is_empty() {
        return;
    }
    let s = walk.point_at(0.0);
    let t = walk.start_tangent();
    let n = if waste_left { t.perp() } else { t.perp() * -1.0 };
    // łuk nie może zbliżyć się do ścieżki bardziej niż w punkcie styku
    let mut rr = lead_r;
    while rr > 0.1 && lp.distance_to(s + n * rr) < rr * 0.98 {
        rr *= 0.5;
    }
    if rr <= 0.1 {
        warnings.push("Za mało miejsca na wejście po łuku — wejście pionowe".into());
        cut_closed(tp, lp, levels, cut, Entry::Plunge, tabs, warnings);
        return;
    }
    let (tab_ranges, tab_z) = tab_ranges(&walk, cut, tabs, warnings);
    let c = s + n * rr;
    let p0 = c - t * rr;
    let pe = c + t * rr;
    let ccw = t.cross(n) > 0.0;

    tp.travel_to(P3::from_xy(p0, cut.top_z + cut.retract_clearance), cut.safe_z);
    let mut prev = cut.top_z;
    for (k, &z) in levels.iter().enumerate() {
        if k > 0 {
            tp.feed(P3::from_xy(p0, prev), cut.feed);
        }
        tp.feed(P3::from_xy(p0, z), cut.plunge_feed);
        tp.arc(P3::from_xy(s, z), c, ccw, cut.feed);
        let z_of = move |_t: f64| z;
        walk.emit(
            tp,
            &WalkSpec {
                s_start: 0.0,
                travel: walk.total,
                z_of: &z_of,
                tabs: &tab_ranges,
                tab_z,
                feed: cut.feed,
                plunge_feed: cut.plunge_feed,
                ramp_end: 0.0,
                reverse: false,
            },
        );
        tp.arc(P3::from_xy(pe, z), c, ccw, cut.feed);
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

    /// Odległość punktu od kwadratu [0, s]² (0 w środku).
    fn dist_to_square(p: Vec2, s: f64) -> f64 {
        let dx = (0.0 - p.x).max(0.0).max(p.x - s);
        let dy = (0.0 - p.y).max(0.0).max(p.y - s);
        (dx * dx + dy * dy).sqrt()
    }

    fn cutting(tp: &crate::Toolpath) -> Vec<crate::toolpath::P3> {
        tp.polyline(0.001).into_iter().filter(|(p, rapid)| !rapid && p.z < -0.01).map(|(p, _)| p).collect()
    }

    fn with(side: ProfileSide, f: impl Fn(&mut ProfileParams)) -> Operation {
        let mut o = op(side);
        if let OpKind::Profile(p) = &mut o.kind {
            f(p);
        }
        o
    }

    #[test]
    fn finishing_pass_leaves_allowance_until_the_end() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let o = with(ProfileSide::Outside, |p| p.finish = Some(0.5));
        let res = generate(&o, &tool, &[square(20.0)]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        // lewa ścianka (y w środku): x ścieżki na poziomach
        let left = |z: f64| -> Vec<f64> {
            cutting(&res.toolpath)
                .into_iter()
                .filter(|p| (p.z - z).abs() < 1e-9 && p.x < -1.0 && p.y >= -1e-9 && p.y <= 20.0 + 1e-9)
                .map(|p| p.x)
                .collect()
        };
        assert!(left(-2.0).iter().all(|x| (x + 3.5).abs() < 1e-6), "zgrubnie: r + 0,5 mm");
        let bottom = left(-4.0);
        assert!(bottom.iter().any(|x| (x + 3.5).abs() < 1e-6), "zgrubnie też na dnie");
        assert!(bottom.iter().any(|x| (x + 3.0).abs() < 1e-6), "wykańczanie do wymiaru");
        // wykańczanie tylko na pełnej głębokości
        assert!(!left(-2.0).is_empty());
        assert!(left(-2.0).iter().all(|x| (x + 3.0).abs() > 1e-6));
    }

    #[test]
    fn lead_in_out_never_cuts_into_the_part() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        for finish in [None, Some(0.4)] {
            let o = with(ProfileSide::Outside, |p| {
                p.lead = Some(3.0);
                p.finish = finish;
            });
            let res = generate(&o, &tool, &[square(20.0)]);
            assert!(res.warnings.is_empty(), "{:?}", res.warnings);
            for p in cutting(&res.toolpath) {
                assert!(dist_to_square(p.xy(), 20.0) >= 3.0 - 1e-6, "narzędzie w detalu: {p:?}");
            }
            // zejścia w Z są w odpadzie, z dala od ścianki
            let mut prev: Option<crate::toolpath::P3> = None;
            for m in &res.toolpath.moves {
                let e = m.end();
                if let Some(q) = prev {
                    if e.z < q.z - 1e-9 && q.xy().distance(e.xy()) < 1e-9 && e.z < -0.01 {
                        assert!(dist_to_square(e.xy(), 20.0) > 3.0 + 1.0, "zejście przy ściance: {e:?}");
                    }
                }
                prev = Some(e);
            }
            assert!(res.toolpath.moves.iter().filter(|m| matches!(m, Move::Arc { .. })).count() >= 4);
        }
        // wewnątrz: zawsze w otworze, odsunięte o promień
        let o = with(ProfileSide::Inside, |p| p.lead = Some(3.0));
        let res = generate(&o, &tool, &[square(20.0)]);
        for p in cutting(&res.toolpath) {
            assert!(p.x >= 3.0 - 1e-6 && p.x <= 17.0 + 1e-6 && p.y >= 3.0 - 1e-6 && p.y <= 17.0 + 1e-6, "{p:?}");
        }
        // za mały otwór na łuk: promień zmniejszony albo wejście pionowe, bez wcinania się
        let res = generate(&o, &tool, &[square(7.0)]);
        for p in cutting(&res.toolpath) {
            assert!(p.x >= 3.0 - 1e-6 && p.x <= 4.0 + 1e-6 && p.y >= 3.0 - 1e-6 && p.y <= 4.0 + 1e-6, "{p:?}");
        }
    }

    #[test]
    fn open_contour_goes_back_and_forth_without_lifting() {
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let line = Contour { verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(50.0, 0.0, 0.0)], closed: false };
        let res = generate(&op(ProfileSide::Outside), &tool, &[line]);
        assert_eq!(res.warnings.len(), 1, "ostrzeżenie o obróbce po linii");
        let cut = cutting(&res.toolpath);
        assert!(cut.iter().all(|p| p.y.abs() < 1e-9), "po linii");
        // dwa poziomy: tam na -2, z powrotem na -4, bez podnoszenia pomiędzy
        assert!(cut.iter().any(|p| (p.x - 50.0).abs() < 1e-9 && (p.z + 2.0).abs() < 1e-9));
        assert!(cut.iter().any(|p| p.x.abs() < 1e-9 && (p.z + 4.0).abs() < 1e-9));
        let first_cut = res.toolpath.moves.iter().position(|m| m.end().z < 0.0).unwrap();
        let last_cut = res.toolpath.moves.iter().rposition(|m| m.end().z < 0.0).unwrap();
        assert!(res.toolpath.moves[first_cut..=last_cut].iter().all(|m| !matches!(m, Move::Rapid(_))));
    }

    #[test]
    fn next_contour_starts_close_to_the_previous_one() {
        let tool = Tool { diameter: 2.0, ..Default::default() };
        let far = Contour { verts: vec![Vertex::new(100.0, 0.0, 0.0), Vertex::new(150.0, 0.0, 0.0)], closed: false };
        let near = Contour { verts: vec![Vertex::new(30.0, 0.0, 0.0), Vertex::new(0.0, 0.0, 0.0)], closed: false };
        let o = with(ProfileSide::On, |_| {});
        let res = generate(&o, &tool, &[far, near]);
        // najpierw bliższa linia, od bliższego końca (0,0), potem druga od (100,0)
        let starts: Vec<_> = res
            .toolpath
            .moves
            .windows(2)
            .filter(|w| matches!(w[0], Move::Rapid(_)) && !matches!(w[1], Move::Rapid(_)))
            .map(|w| w[0].end().xy())
            .collect();
        assert!(starts[0].distance(Vec2::new(0.0, 0.0)) < 1e-9, "{starts:?}");
        assert!(starts.iter().any(|p| p.distance(Vec2::new(100.0, 0.0)) < 1e-9), "{starts:?}");
    }

    #[test]
    fn tool_too_big_warns() {
        let tool = Tool { diameter: 30.0, ..Default::default() };
        let res = generate(&op(ProfileSide::Inside), &tool, &[square(20.0)]);
        assert!(!res.warnings.is_empty());
    }
}
