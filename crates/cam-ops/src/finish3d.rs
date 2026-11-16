//! Obróbka wykańczająca 3D: przejścia równoległe (raster). Narzędzie jest
//! opuszczane na model w gęstych punktach każdej linii (drop-cutter), więc
//! dotyka powierzchni, nie wcinając się w nią. Linie na przemian w obie strony,
//! sąsiednie połączone po powierzchni; opcjonalnie drugi raz w poprzek.
//! Do tego warstwice i strategia łączona (warstwice na stromym, równoległe na
//! łagodnym) oraz ograniczenie do zakresu nachylenia.

use crate::model::{DropIndex, ModelHeights, ToolProfile};
use crate::operation::{CutParams, Finish3DParams, Finish3DStrategy, OpResult};
use crate::rough3d::ModelInput;
use crate::tool::Tool;
use crate::toolpath::P3;
use cam_geom::{Contour, Vec2};
use cam_i18n::tr;

/// Zakładka strategii łączonej [°]: przejścia równoległe sięgają trochę wyżej niż
/// kąt graniczny, a warstwice trochę niżej — bez paska nieobrobionego między nimi.
const SPLIT_OVERLAP: f64 = 3.0;

pub(crate) fn generate(cut: &CutParams, p: &Finish3DParams, tool: &Tool, contours: &[Contour], m: &ModelInput, res: &mut OpResult) {
    let (lo, hi) = (p.slope_min.clamp(0.0, 90.0), p.slope_max.clamp(0.0, 90.0));
    if lo >= hi {
        res.warnings.push(tr!("Zakres nachylenia jest pusty — „od” musi być mniejsze niż „do”").into());
        return;
    }
    let whole = lo <= 0.0 && hi >= 90.0;
    let idx = DropIndex::new(m.heights);
    let prof = ToolProfile::of(tool);
    // mapa nachylenia — tylko gdy potrzebna (zakres albo strategia łączona)
    let slopes = (!whole || p.strategy == Finish3DStrategy::Combined).then(|| SlopeMap::new(&idx, &prof, p.allowance, m.heights, tool, cut.bottom_z()));
    let range = |a: f64, b: f64| -> Option<(&SlopeMap, f32, f32)> {
        let s = slopes.as_ref()?;
        (a > 0.0 || b < 90.0).then_some((s, a as f32, b as f32))
    };
    match p.strategy {
        Finish3DStrategy::Raster => raster(cut, p, tool, contours, m, &idx, range(lo, hi), res),
        Finish3DStrategy::Waterline => waterline(cut, p, tool, contours, m, &idx, range(lo, hi), res),
        Finish3DStrategy::Combined => {
            let split = p.split_angle.clamp(1.0, 89.0);
            // najpierw strome (warstwice), potem łagodne (równoległe)
            if hi > split - SPLIT_OVERLAP {
                waterline(cut, p, tool, contours, m, &idx, range(lo.max(split - SPLIT_OVERLAP), hi), res);
            }
            if lo < split + SPLIT_OVERLAP {
                raster(cut, p, tool, contours, m, &idx, range(lo, hi.min(split + SPLIT_OVERLAP)), res);
            }
            // te same ostrzeżenia z obu części — raz
            let mut seen = std::collections::HashSet::new();
            res.warnings.retain(|w| seen.insert(w.clone()));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn raster(
    cut: &CutParams,
    p: &Finish3DParams,
    tool: &Tool,
    contours: &[Contour],
    m: &ModelInput,
    idx: &DropIndex,
    slope: Option<(&SlopeMap, f32, f32)>,
    res: &mut OpResult,
) {
    let h = m.heights;
    let r = tool.radius();
    let closed: Vec<&Contour> = contours.iter().filter(|c| c.closed && c.verts.len() >= 3).collect();
    if closed.len() < contours.len() {
        res.warnings.push(tr!("Pominięto krzywe otwarte — granicą obróbki są tylko zamknięte").into());
    }
    // obszar dla środka narzędzia: wybrane krzywe albo model poszerzony o promień (wykończone też brzegi)
    let area: Vec<Vec<Vec2>> = if closed.is_empty() {
        let b = h.bounds().expanded(r);
        vec![vec![b.min, Vec2::new(b.max.x, b.min.y), b.max, Vec2::new(b.min.x, b.max.y)]]
    } else {
        closed.iter().map(|c| c.flatten(0.01)).collect()
    };
    let step = (p.stepover.clamp(0.001, 1.0) * tool.diameter).max(0.01);
    let ds = (h.cell * 2.0).clamp(0.05, 0.25);
    let tol = (h.cell * 0.25).max(0.005);
    let prof = ToolProfile::of(tool);
    let bottom = cut.bottom_z();
    let a = p.allowance;
    let z_at = |q: Vec2| idx.drop_tool(q, &prof, a).map_or(bottom, |z| z.max(bottom));
    let Some(rest) = rest_filter(cut, p, tool, m, idx, res) else { return };
    // gdzie obrabiać: resztki i zakres nachylenia
    let filtered = rest.is_some() || slope.is_some();
    let keep = |q: Vec2| {
        rest.as_ref().is_none_or(|r| r.height_at(q).is_some()) && slope.is_none_or(|(s, lo, hi)| s.within(q, lo, hi))
    };
    let mut angles = vec![p.angle];
    if p.cross {
        angles.push(p.angle + 90.0);
    }
    let mut over_top = false;
    for ang in angles {
        let (s, c) = ang.to_radians().sin_cos();
        let u = Vec2::new(c, s);
        let v = Vec2::new(-s, c);
        let pts = area.iter().flatten();
        let vmin = pts.clone().map(|q| q.dot(v)).fold(f64::MAX, f64::min);
        let vmax = pts.map(|q| q.dot(v)).fold(f64::MIN, f64::max);
        let n = ((vmax - vmin) / step).ceil().max(1.0) as usize;
        // linie niezależne — liczone równolegle na wszystkich rdzeniach, potem po kolei
        let line_passes = |k: usize| -> Vec<Vec<P3>> {
            let mut passes = Vec::new();
            let vv = vmin + (vmax - vmin) * k as f64 / n as f64;
            let mut spans = spans_on_line(&area, u, v, vv);
            if k % 2 == 1 {
                spans.reverse();
                spans.iter_mut().for_each(|s| *s = (s.1, s.0));
            }
            for (u0, u1) in spans {
                let line = sample(u * u0 + v * vv, u * u1 + v * vv, ds, tol, &z_at);
                if !filtered {
                    passes.push(simplify3(&line, tol * 0.5));
                    continue;
                }
                // resztki / nachylenie: tylko kawałki linii w obszarze (krótkie przerwy
                // przejeżdżane po powierzchni, okruchy pomijane)
                let xy: Vec<Vec2> = line.iter().map(|q| q.xy()).collect();
                let flags: Vec<bool> = xy.iter().map(|&q| keep(q)).collect();
                for (run, _) in kept_runs(&xy, &flags, false, r, r * 0.5) {
                    let pts: Vec<P3> = run.iter().map(|&i| line[i]).collect();
                    if pts.len() >= 2 {
                        passes.push(simplify3(&pts, tol * 0.5));
                    }
                }
            }
            passes
        };
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get()).min(n + 1);
        let mut per_line: Vec<Vec<Vec<P3>>> = vec![Vec::new(); n + 1];
        let chunk = (n + 1).div_ceil(threads);
        std::thread::scope(|sc| {
            for (c, slot) in per_line.chunks_mut(chunk).enumerate() {
                let line_passes = &line_passes;
                sc.spawn(move || {
                    for (k, out) in slot.iter_mut().enumerate() {
                        *out = line_passes(c * chunk + k);
                    }
                });
            }
        });
        let passes: Vec<Vec<P3>> = per_line.into_iter().flatten().collect();
        // przejścia: sąsiednie łączone po powierzchni, dalsze — przez wysokość bezpieczną
        let tp = &mut res.toolpath;
        for pass in passes {
            let Some(&first) = pass.first() else { continue };
            over_top |= pass.iter().any(|q| q.z > cut.top_z + 1e-6);
            let link = tp.pos().filter(|c| c.z < cut.safe_z - 1e-9 && c.xy().distance(first.xy()) <= step * 1.6 + 1e-6);
            match link {
                Some(c) => {
                    for q in simplify3(&sample(c.xy(), first.xy(), ds, tol, &z_at), tol * 0.5).into_iter().skip(1) {
                        tp.feed(q, cut.feed);
                    }
                }
                None => {
                    tp.travel_to(P3::from_xy(first.xy(), first.z + cut.retract_clearance), cut.safe_z);
                    tp.feed(first, cut.plunge_feed);
                }
            }
            for &q in &pass[1..] {
                tp.feed(q, cut.feed);
            }
        }
        res.toolpath.retract(cut.safe_z);
    }
    if over_top {
        res.warnings.push(tr!("Model wystaje ponad górę półfabrykatu — narzędzie pojedzie nad Z0").into());
    }
}

/// Punkty narzędzia na odcinku a–b: co `ds`, a gdzie powierzchnia się wygina
/// (środek odcinka odbiega od prostej o więcej niż `tol`) — gęściej, aż do `tol`.
/// Dzięki temu prosty ruch między punktami nie ścina grzbietów ani krawędzi ścian
/// bardziej niż o `tol` (ćwierć oczka mapy — mniej niż zapas, z jakim liczone jest opuszczanie).
fn sample(a: Vec2, b: Vec2, ds: f64, tol: f64, z_at: &dyn Fn(Vec2) -> f64) -> Vec<P3> {
    let m = (a.distance(b) / ds).ceil().max(1.0) as usize;
    let mut out = vec![P3::from_xy(a, z_at(a))];
    for i in 1..=m {
        let q = a.lerp(b, i as f64 / m as f64);
        let pq = P3::from_xy(q, z_at(q));
        refine(*out.last().unwrap(), pq, tol, z_at, &mut out, 0);
        out.push(pq);
    }
    out
}

fn refine(a: P3, b: P3, tol: f64, z_at: &dyn Fn(Vec2) -> f64, out: &mut Vec<P3>, depth: u32) {
    if depth > 12 || a.xy().distance(b.xy()) < tol {
        return;
    }
    let mid = a.xy().lerp(b.xy(), 0.5);
    let zm = z_at(mid);
    if (zm - (a.z + b.z) * 0.5).abs() <= tol {
        return;
    }
    let pm = P3::from_xy(mid, zm);
    refine(a, pm, tol, z_at, out, depth + 1);
    out.push(pm);
    refine(pm, b, tol, z_at, out, depth + 1);
}

/// Wysokości opuszczenia narzędzia w węzłach siatki `area.min + (i, j)·step` (równolegle);
/// `NEG_INFINITY` — narzędzie nie dotyka modelu.
fn drop_grid(idx: &DropIndex, prof: &ToolProfile, a: f64, area: cam_geom::Aabb2, step: f64) -> (Vec<f32>, usize, usize) {
    let nx = (area.size().x / step).ceil() as usize + 1;
    let ny = (area.size().y / step).ceil() as usize + 1;
    let mut tz = vec![f32::NEG_INFINITY; nx * ny];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows = ny.div_ceil(threads).max(1);
    std::thread::scope(|sc| {
        for (c, chunk) in tz.chunks_mut(rows * nx).enumerate() {
            sc.spawn(move || {
                for (k, v) in chunk.iter_mut().enumerate() {
                    let (i, j) = (k % nx, c * rows + k / nx);
                    let q = area.min + Vec2::new(i as f64 * step, j as f64 * step);
                    *v = idx.drop_tool(q, prof, a).map_or(f32::NEG_INFINITY, |z| z as f32);
                }
            });
        }
    });
    (tz, nx, ny)
}

/// Obszary resztek po narzędziu odniesienia: węzły siatki, w których zostało na
/// modelu więcej niż `min` materiału, poszerzone o `grow` (promień nowego narzędzia).
/// Zostawiony materiał: najniższe położenie powierzchni narzędzia odniesienia
/// (opuszczanego w węzłach) nad danym punktem minus wysokość modelu w tym punkcie.
fn rest_areas(h: &ModelHeights, idx: &DropIndex, reft: &Tool, bottom: f64, min: f64, grow: f64) -> ModelHeights {
    let rr = reft.radius();
    let step = h.cell.max(0.1).max(rr / 20.0);
    let area = h.bounds().expanded(rr + grow + 2.0 * step);
    let prof = ToolProfile::of(reft);
    let (tz, nx, ny) = drop_grid(idx, &prof, 0.0, area, step);
    let tz: Vec<f32> = tz.into_iter().map(|z| if z.is_finite() { z } else { bottom as f32 }).collect();
    // przesunięcia w kole narzędzia odniesienia i wysokość jego powierzchni nad czubkiem
    let k = (rr / step).floor() as isize;
    let mut offs: Vec<(isize, isize, f32)> = Vec::new();
    for dj in -k..=k {
        for di in -k..=k {
            let d = ((di * di + dj * dj) as f64).sqrt() * step;
            if d <= rr {
                offs.push((di, dj, prof.height_at(d) as f32));
            }
        }
    }
    let mut rest = vec![f32::NEG_INFINITY; nx * ny];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows = ny.div_ceil(threads).max(1);
    std::thread::scope(|sc| {
        for (c, chunk) in rest.chunks_mut(rows * nx).enumerate() {
            let (tz, offs) = (&tz, &offs);
            sc.spawn(move || {
                for (kk, v) in chunk.iter_mut().enumerate() {
                    let (i, j) = ((kk % nx) as isize, (c * rows + kk / nx) as isize);
                    let x = area.min + Vec2::new(i as f64 * step, j as f64 * step);
                    let Some(model) = h.height_at(x) else { continue };
                    // najniżej, gdzie sięgnęła powierzchnia narzędzia odniesienia nad x
                    let mut left = f32::INFINITY;
                    for &(di, dj, hh) in offs {
                        let (pi, pj) = (i + di, j + dj);
                        if pi < 0 || pj < 0 || pi >= nx as isize || pj >= ny as isize {
                            continue;
                        }
                        left = left.min(tz[pj as usize * nx + pi as usize] + hh);
                    }
                    if left as f64 - model > min {
                        *v = 1.0;
                    }
                }
            });
        }
    });
    let mask = ModelHeights {
        min: area.min - Vec2::new(step * 0.5, step * 0.5),
        cell: step,
        nx,
        ny,
        z: rest,
        z_min: 1.0,
        z_max: 1.0,
    };
    mask.dilated(grow)
}

/// Warstwice: na każdej wysokości narzędzie obchodzi model po linii, na której
/// opuszczone narzędzie dotyka go dokładnie na tej wysokości. Wysokości opuszczenia
/// liczone raz w węzłach siatki (równolegle), linie — z interpolacji między węzłami.
#[allow(clippy::too_many_arguments)]
fn waterline(
    cut: &CutParams,
    p: &Finish3DParams,
    tool: &Tool,
    contours: &[Contour],
    m: &ModelInput,
    idx: &DropIndex,
    slope: Option<(&SlopeMap, f32, f32)>,
    res: &mut OpResult,
) {
    let h = m.heights;
    let r = tool.radius();
    let closed: Vec<&Contour> = contours.iter().filter(|c| c.closed && c.verts.len() >= 3).collect();
    if closed.len() < contours.len() {
        res.warnings.push(tr!("Pominięto krzywe otwarte — granicą obróbki są tylko zamknięte").into());
    }
    let limit: Vec<Vec<Vec2>> = closed.iter().map(|c| c.flatten(0.01)).collect();
    let inside = |q: Vec2| {
        if limit.is_empty() {
            return true;
        }
        let mut n = 0;
        for poly in &limit {
            for k in 0..poly.len() {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                    n += 1;
                }
            }
        }
        n % 2 == 1
    };
    // siatka wysokości opuszczenia narzędzia: wokół modelu z zapasem na promień
    let step = h.cell.clamp(0.1, 0.5);
    let area = h.bounds().expanded(r + p.allowance.max(0.0) + 2.0 * step);
    let prof = ToolProfile::of(tool);
    let (tz, nx, ny) = drop_grid(idx, &prof, p.allowance, area, step);
    let Some(rest) = rest_filter(cut, p, tool, m, idx, res) else { return };
    let keep = |q: Vec2| {
        inside(q) && rest.as_ref().is_none_or(|r| r.height_at(q).is_some()) && slope.is_none_or(|(s, lo, hi)| s.within(q, lo, hi))
    };
    let dz = (p.stepover.clamp(0.001, 1.0) * tool.diameter).max(0.01);
    let bottom = cut.bottom_z();
    let tol = (step * 0.1).max(0.005);
    let tp = &mut res.toolpath;
    for z in crate::path::z_levels(cut.top_z, bottom, dz) {
        // linie z modelem po prawej (frezowanie współbieżne wokół wysp)
        let mut paths: Vec<(Vec<Vec2>, bool)> = Vec::new();
        for mut c in crate::model::iso_contours(&tz, nx, ny, area.min, step, z, step * 2.0, tol) {
            c.reverse();
            let pts: Vec<Vec2> = c.verts.iter().map(|v| v.p).collect();
            if limit.is_empty() && rest.is_none() && slope.is_none() {
                paths.push((pts, true));
                continue;
            }
            // z granicą, resztkami albo zakresem nachylenia: tylko kawałki w obszarze
            let pts = densify(&pts, step);
            let flags: Vec<bool> = pts.iter().map(|&q| keep(q)).collect();
            for (run, closed) in kept_runs(&pts, &flags, true, r, r * 0.5) {
                let run: Vec<Vec2> = run.iter().map(|&i| pts[i]).collect();
                if run.len() >= 2 {
                    paths.push((run, closed));
                }
            }
        }
        // kolejność: najbliższa następna
        while !paths.is_empty() {
            let pos = tp.pos().map_or(Vec2::ZERO, |q| q.xy());
            let (k, _) = paths
                .iter()
                .enumerate()
                .map(|(k, (pts, _))| (k, pts[0].distance(pos)))
                .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap())
                .unwrap();
            let (pts, closed) = paths.swap_remove(k);
            tp.travel_to(P3::from_xy(pts[0], z + cut.retract_clearance), cut.safe_z);
            tp.feed(P3::from_xy(pts[0], z), cut.plunge_feed);
            for &q in &pts[1..] {
                tp.feed(P3::from_xy(q, z), cut.feed);
            }
            if closed {
                tp.feed(P3::from_xy(pts[0], z), cut.feed);
            }
        }
    }
    res.toolpath.retract(cut.safe_z);
}

/// Obszary resztek, gdy operacja zbiera resztki po innym narzędziu: `Some(None)` — bez
/// resztek (cała powierzchnia), `Some(Some(maska))`, `None` — nie da się (ostrzeżenie).
fn rest_filter(cut: &CutParams, p: &Finish3DParams, tool: &Tool, m: &ModelInput, idx: &DropIndex, res: &mut OpResult) -> Option<Option<ModelHeights>> {
    if p.rest_tool.is_none() {
        return Some(None);
    }
    let Some(reft) = &m.ref_tool else {
        res.warnings.push(tr!("Resztki: nie ma narzędzia odniesienia w bibliotece").into());
        return None;
    };
    if reft.diameter <= tool.diameter {
        res.warnings.push(tr!("Resztki: narzędzie odniesienia powinno być większe niż to (Ø{} ≤ Ø{})", reft.diameter, tool.diameter));
    }
    Some(Some(rest_areas(m.heights, idx, reft, cut.bottom_z(), p.rest_min.max(0.0), tool.radius())))
}

/// Nachylenie powierzchni, po której jedzie narzędzie [°], w węzłach siatki —
/// liczone z wysokości opuszczonego narzędzia (tak „czuje” je frez) i lekko
/// wygładzone, żeby granica strome/łagodne nie szarpała ścieżki.
pub(crate) struct SlopeMap {
    min: Vec2,
    step: f64,
    nx: usize,
    ny: usize,
    deg: Vec<f32>,
}

impl SlopeMap {
    fn new(idx: &DropIndex, prof: &ToolProfile, allowance: f64, h: &ModelHeights, tool: &Tool, bottom: f64) -> Self {
        let step = h.cell.clamp(0.1, 0.5);
        let area = h.bounds().expanded(tool.radius() + allowance.max(0.0) + 2.0 * step);
        let (tz, nx, ny) = drop_grid(idx, prof, allowance, area, step);
        let b = bottom as f32;
        let z = |i: usize, j: usize| {
            let v = tz[j * nx + i];
            if v.is_finite() { v.max(b) } else { b }
        };
        let mut deg = vec![0f32; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let (i0, i1) = (i.saturating_sub(1), (i + 1).min(nx - 1));
                let (j0, j1) = (j.saturating_sub(1), (j + 1).min(ny - 1));
                let gx = (z(i1, j) - z(i0, j)) / ((i1 - i0).max(1) as f64 * step) as f32;
                let gy = (z(i, j1) - z(i, j0)) / ((j1 - j0).max(1) as f64 * step) as f32;
                deg[j * nx + i] = (gx * gx + gy * gy).sqrt().atan().to_degrees();
            }
        }
        // wygładzanie: średnia w kwadracie ok. ćwierci promienia narzędzia
        let k = ((tool.radius() * 0.25) / step).round().max(1.0) as usize;
        let mut tmp = vec![0f32; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let (a, b) = (i.saturating_sub(k), (i + k).min(nx - 1));
                tmp[j * nx + i] = deg[j * nx + a..=j * nx + b].iter().sum::<f32>() / (b - a + 1) as f32;
            }
        }
        for j in 0..ny {
            for i in 0..nx {
                let (a, b) = (j.saturating_sub(k), (j + k).min(ny - 1));
                deg[j * nx + i] = (a..=b).map(|jj| tmp[jj * nx + i]).sum::<f32>() / (b - a + 1) as f32;
            }
        }
        SlopeMap { min: area.min, step, nx, ny, deg }
    }

    /// Nachylenie w punkcie (najbliższy węzeł); poza mapą — płasko.
    fn at(&self, q: Vec2) -> f32 {
        let i = ((q.x - self.min.x) / self.step).round();
        let j = ((q.y - self.min.y) / self.step).round();
        if i < 0.0 || j < 0.0 || i >= self.nx as f64 || j >= self.ny as f64 {
            return 0.0;
        }
        self.deg[j as usize * self.nx + i as usize]
    }

    fn within(&self, q: Vec2, lo: f32, hi: f32) -> bool {
        let d = self.at(q);
        d >= lo && d <= hi
    }
}

/// Kawałki łamanej do obróbki: listy indeksów punktów, dla których `flags` = true,
/// po wypełnieniu przerw krótszych niż `max_gap` (przejazd po powierzchni zamiast
/// podnoszenia freza) i pominięciu kawałków krótszych niż `min_len`. Dla łamanej
/// zamkniętej (`closed`) kawałki mogą przechodzić przez jej początek; `true` przy
/// kawałku — cała pętla.
fn kept_runs(pts: &[Vec2], flags: &[bool], closed: bool, max_gap: f64, min_len: f64) -> Vec<(Vec<usize>, bool)> {
    let n = pts.len();
    if n == 0 || !flags.iter().any(|f| *f) {
        return Vec::new();
    }
    if flags.iter().all(|f| *f) {
        return vec![((0..n).collect(), closed)];
    }
    // kolejność: zamknięta — od początku któregoś kawałka (bez przechodzenia przez koniec)
    let start = if closed { (0..n).find(|&i| flags[i] && !flags[(i + n - 1) % n]).unwrap_or(0) } else { 0 };
    let order: Vec<usize> = (0..n).map(|k| (start + k) % n).collect();
    // ciągi tych samych flag
    let mut runs: Vec<(bool, Vec<usize>)> = Vec::new();
    for &i in &order {
        match runs.last_mut() {
            Some((f, v)) if *f == flags[i] => v.push(i),
            _ => runs.push((flags[i], vec![i])),
        }
    }
    let len = |v: &[usize]| -> f64 { v.windows(2).map(|w| pts[w[0]].distance(pts[w[1]])).sum() };
    let count = runs.len();
    for k in 0..count {
        if runs[k].0 {
            continue;
        }
        let bounded = closed || (k > 0 && k + 1 < count);
        if !bounded {
            continue;
        }
        // przerwa: od ostatniego punktu przed nią do pierwszego po niej
        let prev = *runs[(k + count - 1) % count].1.last().unwrap();
        let next = runs[(k + 1) % count].1[0];
        let mut path = vec![prev];
        path.extend(&runs[k].1);
        path.push(next);
        if len(&path) <= max_gap {
            runs[k].0 = true;
        }
    }
    // sklejenie sąsiednich kawałków
    let mut merged: Vec<Vec<usize>> = Vec::new();
    let mut prev_true = false;
    for (f, v) in runs {
        if f {
            if prev_true {
                merged.last_mut().unwrap().extend(v);
            } else {
                merged.push(v);
            }
        }
        prev_true = f;
    }
    // zamknięta: ostatni i pierwszy kawałek stykają się na początku
    if closed && merged.len() > 1 {
        let first = merged[0][0];
        let last = *merged.last().unwrap().last().unwrap();
        if (last + 1) % n == first {
            let head = merged.remove(0);
            merged.last_mut().unwrap().extend(head);
        }
    }
    if closed && merged.len() == 1 && merged[0].len() == n {
        return vec![(merged.remove(0), true)];
    }
    merged.into_iter().filter(|v| len(v) >= min_len).map(|v| (v, false)).collect()
}

/// Punkty łamanej zamkniętej co najwyżej co `step` (do przycinania do obszaru).
fn densify(pts: &[Vec2], step: f64) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(pts.len());
    for (k, &a) in pts.iter().enumerate() {
        let b = pts[(k + 1) % pts.len()];
        let n = (a.distance(b) / step).ceil().max(1.0) as usize;
        for i in 0..n {
            out.push(a.lerp(b, i as f64 / n as f64));
        }
    }
    out
}

/// Odcinki linii {q: q·v = vv} wewnątrz obszaru (parzysto-nieparzyście), jako zakresy współrzędnej u.
fn spans_on_line(area: &[Vec<Vec2>], u: Vec2, v: Vec2, vv: f64) -> Vec<(f64, f64)> {
    let mut xs = Vec::new();
    for poly in area {
        let n = poly.len();
        for k in 0..n {
            let (a, b) = (poly[k], poly[(k + 1) % n]);
            let (va, vb) = (a.dot(v), b.dot(v));
            if (va > vv) != (vb > vv) {
                let t = (vv - va) / (vb - va);
                xs.push(a.dot(u) + (b.dot(u) - a.dot(u)) * t);
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.chunks_exact(2).filter(|p| p[1] - p[0] > 1e-6).map(|p| (p[0], p[1])).collect()
}

/// Upraszczanie łamanej 3D (Douglas–Peucker), tolerancja w mm.
fn simplify3(pts: &[P3], tol: f64) -> Vec<P3> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        let (a, b) = (pts[i], pts[j]);
        let ab = [b.x - a.x, b.y - a.y, b.z - a.z];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let mut worst = (0.0, i);
        for (k, q) in pts.iter().enumerate().take(j).skip(i + 1) {
            let aq = [q.x - a.x, q.y - a.y, q.z - a.z];
            let t = if l2 > 0.0 { ((aq[0] * ab[0] + aq[1] * ab[1] + aq[2] * ab[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let d = ((aq[0] - ab[0] * t).powi(2) + (aq[1] - ab[1] * t).powi(2) + (aq[2] - ab[2] * t).powi(2)).sqrt();
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((i, worst.1));
            stack.push((worst.1, j));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use crate::model::{DropIndex, ModelHeights, ToolProfile};
    use crate::operation::*;
    use crate::tool::{Tool, ToolKind};
    use crate::ModelInput;
    use cam_geom::{Aabb2, Vec2};

    /// Kopuła (półkula R 10 na płycie Z −10), góra na Z 0.
    fn dome() -> ModelHeights {
        let f = |x: f32, y: f32| {
            let r2 = (x - 15.0).powi(2) + (y - 15.0).powi(2);
            if r2 < 100.0 { -10.0 + (100.0 - r2).sqrt() } else { -10.0 }
        };
        let mut t = Vec::new();
        for j in 0..60 {
            for i in 0..60 {
                let (x0, y0, x1, y1) = (i as f32 * 0.5, j as f32 * 0.5, i as f32 * 0.5 + 0.5, j as f32 * 0.5 + 0.5);
                let p = |x, y| [x, y, f(x, y)];
                t.push([p(x0, y0), p(x1, y0), p(x1, y1)]);
                t.push([p(x0, y0), p(x1, y1), p(x0, y1)]);
            }
        }
        ModelHeights::from_triangles(&t, 0.1).unwrap()
    }

    fn op(cross: bool) -> Operation {
        Operation {
            name: "w".into(),
            side: Side::A,
            enabled: true,
            kind: OpKind::Finish3D(Finish3DParams { cross, ..Default::default() }),
            tool_id: 1,
            cut: CutParams { top_z: 0.0, depth: 10.0, ..Default::default() },
            contours: vec![],
        }
    }

    #[test]
    fn raster_follows_the_surface_without_gouging() {
        let h = dome();
        let tool = Tool { diameter: 6.0, kind: ToolKind::BallEnd, ..Default::default() };
        let m = ModelInput { heights: &h, stock: Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(30.0, 30.0) }, ref_tool: None };
        let res = generate_with_model(&op(false), &tool, &[], Some(&m));
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let prof = ToolProfile::of(&tool);
        let idx = DropIndex::new(&h);
        let pl = res.toolpath.polyline(0.01);
        let mut on_top = false;
        let mut cutting = 0;
        for w in pl.windows(2) {
            if w[1].1 {
                continue;
            }
            let (a, b) = (w[0].0, w[1].0);
            let n = (a.distance(b) / 0.1).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let q = a.xy().lerp(b.xy(), t);
                let z = a.z + (b.z - a.z) * t;
                if idx.drop_tool(q, &prof, 0.0).is_some() {
                    // bez wcinania: narzędzie mniejsze o ćwierć oczka z każdej strony nie dotyka modelu
                    // (tolerancja próbkowania i upraszczania, mierzona prostopadle do powierzchni)
                    let zs = idx.drop_tool(q, &prof, -h.cell * 0.25 - 1e-3).unwrap_or(f64::NEG_INFINITY);
                    assert!(z >= zs - 1e-9, "wcięcie w {q:?}: {z} < {zs}");
                    // i blisko powierzchni (narzędzie większe o 0,05 mm już by się wcinało)
                    let zb = idx.drop_tool(q, &prof, 0.05).unwrap();
                    assert!(z <= zb.max(-10.0) + h.cell * 0.25, "nad powierzchnią w {q:?}: {z} > {zb}");
                }
                cutting += 1;
                on_top |= q.distance(Vec2::new(15.0, 15.0)) < 0.5 && (z - 0.0).abs() < 0.02;
            }
        }
        assert!(cutting > 1000);
        assert!(on_top, "przejście przez szczyt kopuły");
        // przejścia co 10 % średnicy: na 30 + 6 mm ok. 61 linii, połączone bez podnoszenia
        let lifts = res.toolpath.moves.iter().filter(|m| matches!(m, crate::Move::Rapid(p) if p.z >= 4.9)).count();
        assert!(lifts <= 3, "podniesienia: {lifts}");
        // w poprzek: dwa razy więcej drogi
        let both = generate_with_model(&op(true), &tool, &[], Some(&m));
        assert!(both.toolpath.moves.len() > res.toolpath.moves.len() * 3 / 2);
    }

    #[test]
    fn waterline_goes_around_the_dome_without_gouging() {
        let h = dome();
        let idx = DropIndex::new(&h);
        let tool = Tool { diameter: 6.0, kind: ToolKind::BallEnd, ..Default::default() };
        let prof = ToolProfile::of(&tool);
        let m = ModelInput { heights: &h, stock: Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(30.0, 30.0) }, ref_tool: None };
        let mut o = op(false);
        o.kind = OpKind::Finish3D(Finish3DParams { strategy: Finish3DStrategy::Waterline, stepover: 1.0 / 6.0, ..Default::default() });
        o.cut.depth = 9.0;
        let res = generate_with_model(&o, &tool, &[], Some(&m));
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        // warstwy co 1 mm: na każdej wysokości pętla wokół kopuły
        let pl = res.toolpath.polyline(0.01);
        let mut levels: Vec<i64> = pl.iter().filter(|q| !q.1 && q.0.z < 0.0).map(|q| (q.0.z * 1000.0).round() as i64).collect();
        levels.sort();
        levels.dedup();
        assert_eq!(levels.len(), 9, "{levels:?}");
        let mut ccw_like = 0.0;
        for w in pl.windows(2) {
            if w[1].1 || w[0].1 {
                continue;
            }
            let (a, b) = (w[0].0, w[1].0);
            if (a.z - b.z).abs() > 1e-9 {
                continue; // zejście na warstwę
            }
            for k in 0..=4 {
                let q = a.xy().lerp(b.xy(), k as f64 / 4.0);
                // nie wcina się: narzędzie mniejsze o krok siatki nie dotyka modelu
                let zs = idx.drop_tool(q, &prof, -0.11).unwrap_or(f64::NEG_INFINITY);
                assert!(a.z >= zs - 1e-9, "wcięcie w {q:?} na z = {}: {zs}", a.z);
                // dotyka: narzędzie większe o 0,15 mm już by się wcinało
                let zb = idx.drop_tool(q, &prof, 0.15).unwrap_or(f64::NEG_INFINITY);
                assert!(a.z <= zb + 1e-9, "za daleko od modelu w {q:?} na z = {}: {zb}", a.z);
            }
            // kierunek: kopuła po prawej = ruch zgodny z ruchem wskazówek zegara wokół środka
            let c = Vec2::new(15.0, 15.0);
            ccw_like += (a.xy() - c).cross(b.xy() - c);
        }
        assert!(ccw_like < 0.0, "współbieżnie: wokół wyspy zgodnie z ruchem wskazówek zegara");
    }

    /// Punkty ruchów roboczych w materiale (co 0,1 mm), bez dojazdów nad Z 0.
    fn cutting_points(tp: &crate::Toolpath) -> Vec<crate::P3> {
        let pl = tp.polyline(0.01);
        let mut out = Vec::new();
        for w in pl.windows(2) {
            if w[1].1 {
                continue;
            }
            let (a, b) = (w[0].0, w[1].0);
            let n = (a.distance(b) / 0.1).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let q = crate::P3::from_xy(a.xy().lerp(b.xy(), t), a.z + (b.z - a.z) * t);
                if q.z < -0.01 {
                    out.push(q);
                }
            }
        }
        out
    }

    #[test]
    fn slope_range_and_combined_strategy() {
        let h = dome();
        let tool = Tool { diameter: 6.0, kind: ToolKind::BallEnd, ..Default::default() };
        let m = ModelInput { heights: &h, stock: Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(30.0, 30.0) }, ref_tool: None };
        let run = |strategy, lo: f64, hi: f64, depth: f64| {
            let mut o = op(false);
            o.cut.depth = depth;
            o.kind = OpKind::Finish3D(Finish3DParams { strategy, stepover: 0.1, slope_min: lo, slope_max: hi, ..Default::default() });
            let res = generate_with_model(&o, &tool, &[], Some(&m));
            assert!(res.warnings.is_empty(), "{:?}", res.warnings);
            res
        };
        // odległość od osi kopuły; nachylenie pod środkiem freza = asin(ρ / 13), a od
        // ρ = √160 ≈ 12,65 frez stoi już na płycie (płasko). Przy tym narożniku mapa
        // nachylenia jest wygładzona (ok. 0,75 mm) — pasek 1 mm nie jest sprawdzany.
        let rho = |q: &crate::P3| q.xy().distance(Vec2::new(15.0, 15.0));
        let foot = 160f64.sqrt();
        let slope_at = |q: &crate::P3| if rho(q) >= foot { 0.0 } else { (rho(q) / 13.0).asin().to_degrees() };
        let near_foot = |q: &crate::P3| (rho(q) - foot).abs() < 1.0;
        // równoległa do 40°: nic tam, gdzie wyraźnie stromiej (z zapasem na wygładzenie mapy)
        let flat = cutting_points(&run(Finish3DStrategy::Raster, 0.0, 40.0, 10.0).toolpath);
        assert!(flat.len() > 1000);
        let bad: Vec<_> = flat.iter().filter(|q| !near_foot(q) && slope_at(q) > 47.0).collect();
        assert!(bad.is_empty(), "równoległa na stromym: {} pkt, np. {:?}", bad.len(), bad.first());
        assert!(flat.iter().any(|q| rho(q) < 1.0), "szczyt obrobiony");
        // warstwice od 50°: tylko na stromym
        let steep = cutting_points(&run(Finish3DStrategy::Waterline, 50.0, 90.0, 9.0).toolpath);
        assert!(steep.len() > 500);
        let bad: Vec<_> = steep.iter().filter(|q| !near_foot(q) && slope_at(q) <= 42.0).collect();
        assert!(bad.is_empty(), "warstwice na łagodnym: {} pkt, np. {:?}", bad.len(), bad.first());
        // tylko płaskie (0–2°): płyta i sam czubek
        let only_flat = cutting_points(&run(Finish3DStrategy::Raster, 0.0, 2.0, 10.0).toolpath);
        assert!(only_flat.iter().all(|q| rho(q) > foot - 1.0 || rho(q) < 1.5), "{:?}", only_flat.iter().find(|q| rho(q) <= foot - 1.0 && rho(q) >= 1.5));
        assert!(only_flat.iter().any(|q| rho(q) > 14.0));
        // łączona = warstwice na stromym i równoległa na łagodnym, bez wcinania
        let both = run(Finish3DStrategy::Combined, 0.0, 90.0, 9.0);
        let pts = cutting_points(&both.toolpath);
        let prof = ToolProfile::of(&tool);
        let idx = DropIndex::new(&h);
        for q in &pts {
            let zs = idx.drop_tool(q.xy(), &prof, -h.cell * 0.25 - 1e-3).unwrap_or(f64::NEG_INFINITY);
            assert!(q.z >= zs - 1e-9, "wcięcie w {q:?}");
        }
        // pokrycie: w każdej strefie nachylenia są punkty (bez paska nieobrobionego)
        for band in [(0.0, 10.0), (35.0, 45.0), (45.0, 55.0), (60.0, 75.0)] {
            assert!(pts.iter().any(|q| slope_at(q) >= band.0 && slope_at(q) < band.1 && q.z > -9.0), "brak obróbki przy {band:?}°");
        }
        // pusty zakres: ostrzeżenie
        let mut o = op(false);
        o.kind = OpKind::Finish3D(Finish3DParams { slope_min: 50.0, slope_max: 20.0, ..Default::default() });
        assert!(!generate_with_model(&o, &tool, &[], Some(&m)).warnings.is_empty());
    }

    #[test]
    fn kept_runs_fill_gaps_and_drop_crumbs() {
        // 20 punktów co 1 mm; przerwa 2 pkt (wypełniana przy max_gap 3) i okruch 1 pkt
        let pts: Vec<Vec2> = (0..20).map(|i| Vec2::new(i as f64, 0.0)).collect();
        let mut f = vec![true; 20];
        f[5] = false;
        f[6] = false;
        for k in 10..20 {
            f[k] = false;
        }
        f[15] = true; // okruch
        let runs = super::kept_runs(&pts, &f, false, 3.0, 1.5);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0].0, (0..10).collect::<Vec<_>>());
        // zamknięta: przerwa przez początek wypełniona — cała pętla
        let mut c = vec![true; 20];
        c[0] = false;
        c[19] = false;
        let runs = super::kept_runs(&pts, &c, true, 30.0, 1.0);
        assert_eq!(runs.len(), 1);
        assert!(runs[0].1 && runs[0].0.len() == 20, "{runs:?}");
        // zamknięta z długą przerwą: jeden kawałek przechodzący przez koniec
        let mut d = vec![true; 20];
        for k in 8..12 {
            d[k] = false;
        }
        let runs = super::kept_runs(&pts, &d, true, 1.0, 1.0);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].0.first(), Some(&12));
        assert_eq!(runs[0].0.last(), Some(&7));
    }

    #[test]
    fn rest_machining_only_where_the_bigger_tool_did_not_reach() {
        // płyta 40 × 20 (wierzch Z 0) z rowkiem szerokości 4 mm (x 18–22), głębokości 3 mm
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32, z: f32| {
            [[[x0, y0, z], [x1, y0, z], [x1, y1, z]], [[x0, y0, z], [x1, y1, z], [x0, y1, z]]]
        };
        let mut t = Vec::new();
        t.extend(quad(0.0, 0.0, 18.0, 20.0, 0.0));
        t.extend(quad(22.0, 0.0, 40.0, 20.0, 0.0));
        t.extend(quad(18.0, 0.0, 22.0, 20.0, -3.0));
        let h = ModelHeights::from_triangles(&t, 0.05).unwrap();
        let big = Tool { id: 7, diameter: 10.0, kind: ToolKind::BallEnd, ..Default::default() };
        let small = Tool { diameter: 3.0, kind: ToolKind::BallEnd, ..Default::default() };
        let stock = Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(40.0, 20.0) };
        let mk = |strategy, rest: bool| {
            let mut o = op(false);
            o.cut.depth = 3.0;
            o.kind = OpKind::Finish3D(Finish3DParams { strategy, stepover: 0.2, rest_tool: rest.then_some(7), ..Default::default() });
            o
        };
        let m = ModelInput { heights: &h, stock, ref_tool: Some(big.clone()) };
        for strategy in [Finish3DStrategy::Raster, Finish3DStrategy::Waterline] {
            let res = generate_with_model(&mk(strategy, true), &small, &[], Some(&m));
            assert!(res.warnings.is_empty(), "{:?}", res.warnings);
            let pts: Vec<_> = res.toolpath.polyline(0.01).into_iter().filter(|q| !q.1 && q.0.z < 4.0).map(|q| q.0).collect();
            assert!(!pts.is_empty(), "{strategy:?}: brak ścieżki resztek");
            for q in &pts {
                assert!(q.x > 16.0 && q.x < 24.0, "{strategy:?}: poza rowkiem: {q:?}");
            }
            assert!(pts.iter().any(|q| q.z < -2.9), "{strategy:?}: do dna rowka");
        }
        // bez resztek: cała płyta
        let all = generate_with_model(&mk(Finish3DStrategy::Raster, false), &small, &[], Some(&m));
        assert!(all.toolpath.polyline(0.01).iter().any(|q| !q.1 && q.0.x < 5.0));
        // bez narzędzia odniesienia: ostrzeżenie, bez ścieżki
        let none = ModelInput { heights: &h, stock, ref_tool: None };
        let r = generate_with_model(&mk(Finish3DStrategy::Raster, true), &small, &[], Some(&none));
        assert!(!r.warnings.is_empty() && r.toolpath.moves.len() <= 1);
    }
}
