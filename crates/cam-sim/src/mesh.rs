//! Siatka materiału z mapy wysokości z dokładnie położonymi ścianami.
//!
//! Węzły mapy mają dokładne wysokości. Na każdej krawędzi siatki wyszukiwane są
//! uskoki (ściany wycięć) — rekurencyjnie, na dokładnej funkcji wysokości
//! `Simulator::height_exact` — z położeniem i wysokościami po obu stronach.
//! Komórka z uskokami jest dzielona cięciwami (ścianami) na regiony; każdy region
//! dostaje własny wielokąt, a każda cięciwa pionową ścianę. Obsługiwana jest
//! dowolna liczba poziomów w komórce (wąskie półki, front narzędzia przy ścianie).
//! Łagodne zbocza (frez V, kulowy) są rysowane jako gładka powierzchnia.

use crate::{SimMesh, SimVertex, Simulator};
use cam_geom::Vec2;
use std::collections::HashMap;

const EPS: f32 = 1e-3;
/// Nachylenie (dz/dx), powyżej którego zmiana wysokości jest uskokiem, a nie zboczem.
const STEEP: f64 = 5.0;
/// Najmniejszy uskok traktowany jako ściana [mm].
const MIN_STEP: f64 = 0.02;
/// Uskoki bliżej siebie niż to [mm] są łączone w jeden.
const MERGE_DIST: f64 = 0.02;

/// Uskok na krawędzi siatki (kolejność od pierwszego węzła klucza krawędzi).
#[derive(Clone, Copy)]
struct Step {
    p: Vec2,
    /// Wysokość po stronie początku krawędzi.
    za: f32,
    /// Wysokość po stronie końca krawędzi.
    zb: f32,
}

type EdgeKey = (usize, usize, u8);

fn find_steps(sim: &Simulator, pa: Vec2, pb: Vec2, ha: f32, hb: f32, zmid: f32) -> Vec<Step> {
    let len = pa.distance(pb);
    let mut out = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn rec(sim: &Simulator, pa: Vec2, pb: Vec2, len: f64, ta: f64, za: f64, tb: f64, zb: f64, out: &mut Vec<Step>, depth: u32) {
        let d = (za - zb).abs();
        if d < MIN_STEP {
            return;
        }
        let l = (tb - ta) * len;
        if l < 2e-4 || depth > 40 {
            let t = (ta + tb) * 0.5;
            out.push(Step { p: pa.lerp(pb, t), za: za as f32, zb: zb as f32 });
            return;
        }
        let tm = (ta + tb) * 0.5;
        let zm = sim.height_exact(pa.lerp(pb, tm));
        // łagodne zbocze: niezbyt strome i liniowe na tym odcinku
        if d < STEEP * l && (zm - (za + zb) * 0.5).abs() < 0.25 * d {
            return;
        }
        rec(sim, pa, pb, len, ta, za, tm, zm, out, depth + 1);
        rec(sim, pa, pb, len, tm, zm, tb, zb, out, depth + 1);
    }
    rec(sim, pa, pb, len, 0.0, ha as f64, 0.5, zmid as f64, &mut out, 1);
    rec(sim, pa, pb, len, 0.5, zmid as f64, 1.0, hb as f64, &mut out, 1);
    // uskoki tuż obok siebie (np. ściana + skraj rampy) = jeden uskok
    let mut merged: Vec<Step> = Vec::with_capacity(out.len());
    for st in out {
        match merged.last_mut() {
            Some(prev) if prev.p.distance(st.p) < MERGE_DIST => prev.zb = st.zb,
            _ => merged.push(st),
        }
    }
    merged.retain(|st| ((st.za - st.zb).abs() as f64) >= MIN_STEP);
    merged
}

/// Punkt na obwodzie komórki (w kolejności CCW).
#[derive(Clone, Copy)]
struct PerimPt {
    p: Vec2,
    /// Narożnik (indeks 0..4) albo uskok.
    corner: Option<usize>,
    /// Wysokość przed i za punktem (idąc CCW); dla narożnika obie równe.
    z_before: f32,
    z_after: f32,
    n_before: [f32; 3],
    n_after: [f32; 3],
    /// Identyfikator uskoku (krawędź, numer na krawędzi) — do łączenia normalnych ścian.
    key: (EdgeKey, usize),
}

pub(crate) fn build_mesh(sim: &Simulator) -> SimMesh {
    let debug = std::env::var("CAMSIM_DEBUG").is_ok();
    let mut debug_left = 6;
    let m = &sim.map;
    let (nx, ny) = (m.nx, m.ny);
    let h = |i: usize, j: usize| m.z[j * nx + i];
    let through = |z: f32| z <= m.bottom + EPS;

    // --- uskoki na krawędziach (wspólne dla sąsiednich komórek) ---
    let mut step_cache: HashMap<EdgeKey, Vec<Step>> = HashMap::new();
    let mut steps_of = |key: EdgeKey| -> Vec<Step> {
        step_cache
            .entry(key)
            .or_insert_with(|| {
                let (i, j, dir) = key;
                let (i2, j2) = if dir == 0 { (i + 1, j) } else { (i, j + 1) };
                let (ha, hb) = (h(i, j), h(i2, j2));
                // także przy równych końcach może być wąski rowek/żebro — stąd pomiar środka
                let zm = sim.height_exact(m.center(i, j).lerp(m.center(i2, j2), 0.5)) as f32;
                if (ha - hb).abs() <= 0.005 && (zm - ha).abs() <= 0.005 {
                    return Vec::new();
                }
                find_steps(sim, m.center(i, j), m.center(i2, j2), ha, hb, zm)
            })
            .clone()
    };

    // --- normalne w węzłach (bez sąsiadów zza uskoku) ---
    let cell = m.cell as f32;
    let mut normals = vec![[0.0f32, 0.0, 1.0]; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let c = h(i, j);
            let slope = |a: Option<f32>, b: Option<f32>| -> f32 {
                match (a, b) {
                    (Some(a), Some(b)) => (b - a) / (2.0 * cell),
                    (Some(a), None) => (c - a) / cell,
                    (None, Some(b)) => (b - c) / cell,
                    (None, None) => 0.0,
                }
            };
            let l = (i > 0 && steps_of((i - 1, j, 0)).is_empty()).then(|| h(i - 1, j));
            let r = (i + 1 < nx && steps_of((i, j, 0)).is_empty()).then(|| h(i + 1, j));
            let d = (j > 0 && steps_of((i, j - 1, 1)).is_empty()).then(|| h(i, j - 1));
            let u = (j + 1 < ny && steps_of((i, j, 1)).is_empty()).then(|| h(i, j + 1));
            let (sx, sy) = (slope(l, r), slope(d, u));
            let len = (sx * sx + sy * sy + 1.0).sqrt();
            normals[j * nx + i] = [-sx / len, -sy / len, 1.0 / len];
        }
    }

    let mut out = SimMesh::default();
    for j in 0..ny {
        for i in 0..nx {
            let c = m.center(i, j);
            out.verts.push(SimVertex { pos: [c.x as f32, c.y as f32, h(i, j)], normal: normals[j * nx + i], kind: 0 });
        }
    }
    let bottom_base = out.verts.len() as u32;
    for j in 0..ny {
        for i in 0..nx {
            let c = m.center(i, j);
            out.verts.push(SimVertex { pos: [c.x as f32, c.y as f32, m.bottom], normal: [0.0, 0.0, -1.0], kind: 2 });
        }
    }
    let node = |i: usize, j: usize| (j * nx + i) as u32;
    let bnode = |i: usize, j: usize| bottom_base + (j * nx + i) as u32;
    let push = |out: &mut SimMesh, p: Vec2, z: f32, n: [f32; 3], kind: u8| -> u32 {
        out.verts.push(SimVertex { pos: [p.x as f32, p.y as f32, z], normal: n, kind });
        (out.verts.len() - 1) as u32
    };

    struct WallSeg {
        keys: [(EdgeKey, usize); 2],
        p: [Vec2; 2],
        top: [f32; 2],
        bot: [f32; 2],
        n: Vec2,
    }
    let mut wall_segs: Vec<WallSeg> = Vec::new();

    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let hs: [f32; 4] = corners.map(|(a, b)| h(a, b));
            // krawędzie CCW: (klucz, czy kierunek klucza zgodny z CCW)
            let edges: [(EdgeKey, bool); 4] = [((i, j, 0), true), ((i + 1, j, 1), true), ((i, j + 1, 0), false), ((i, j, 1), false)];
            let steps: [Vec<Step>; 4] = edges.map(|(k, _)| steps_of(k));

            let smooth = |out: &mut SimMesh| {
                if hs.iter().all(|&z| through(z)) {
                    return;
                }
                let [a, b, c, d] = corners.map(|(x, y)| node(x, y));
                let (zmin, zmax) = hs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &z| (lo.min(z), hi.max(z)));
                if zmax - zmin > 1e-4 {
                    // zbocze: punkt środkowy o dokładnej wysokości — załamania idą wzdłuż geometrii
                    let pc = m.center(i, j).lerp(m.center(i + 1, j + 1), 0.5);
                    let zc = sim.height_exact(pc) as f32;
                    let mut n = [0.0f32; 3];
                    for (ci, cj) in corners {
                        let k = normals[cj * nx + ci];
                        n = [n[0] + k[0], n[1] + k[1], n[2] + k[2]];
                    }
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                    let e = push(out, pc, zc, [n[0] / l, n[1] / l, n[2] / l], 0);
                    out.indices.extend_from_slice(&[a, b, e, b, c, e, c, d, e, d, a, e]);
                } else {
                    out.indices.extend_from_slice(&[a, b, c, a, c, d]);
                }
                let [a, b, c, d] = corners.map(|(x, y)| bnode(x, y));
                out.indices.extend_from_slice(&[a, c, b, a, d, c]);
            };

            if steps.iter().all(|s| s.is_empty()) {
                smooth(&mut out);
                continue;
            }

            // --- obwód komórki CCW: narożniki + uskoki ---
            let mut perim: Vec<PerimPt> = Vec::new();
            for k in 0..4 {
                let (ci, cj) = corners[k];
                let nk = normals[cj * nx + ci];
                let (ci2, cj2) = corners[(k + 1) % 4];
                let nk2 = normals[cj2 * nx + ci2];
                perim.push(PerimPt {
                    p: m.center(ci, cj),
                    corner: Some(k),
                    z_before: hs[k],
                    z_after: hs[k],
                    n_before: nk,
                    n_after: nk,
                    key: ((0, 0, 9), 0),
                });
                let (key, forward) = edges[k];
                let st = &steps[k];
                let mut list: Vec<(usize, Step)> = st.iter().copied().enumerate().collect();
                if !forward {
                    list.reverse();
                }
                let n_steps = list.len();
                for (pos, (idx, s)) in list.into_iter().enumerate() {
                    let (zb4, zaf) = if forward { (s.za, s.zb) } else { (s.zb, s.za) };
                    perim.push(PerimPt {
                        p: s.p,
                        corner: None,
                        z_before: zb4,
                        z_after: zaf,
                        n_before: if pos == 0 { nk } else { [0.0, 0.0, 1.0] },
                        n_after: if pos + 1 == n_steps { nk2 } else { [0.0, 0.0, 1.0] },
                        key: (key, idx),
                    });
                }
            }

            // --- łączenie uskoków w cięciwy (ściany): ta sama para poziomów ---
            // Uskoki bez pary (np. koniec mostka: trzy poziomy w jednym punkcie)
            // łączą się w punkcie węzłowym wewnątrz komórki.
            let mut partner = pair_steps(&perim);
            // zanikające uskoki bez pary (np. rampa schodząca z poziomu półki) — powierzchnia ciągła
            let fading: Vec<usize> = (0..perim.len())
                .filter(|&q| perim[q].corner.is_none() && partner[q].is_none())
                .filter(|&q| (perim[q].z_before - perim[q].z_after).abs() < 0.25)
                .collect();
            if !fading.is_empty() {
                let mut q = 0;
                perim.retain(|_| {
                    let keep = !fading.contains(&q);
                    q += 1;
                    keep
                });
                partner = pair_steps(&perim);
            }
            let step_ids: Vec<usize> = (0..perim.len()).filter(|&q| perim[q].corner.is_none()).collect();
            let mut ok = true;
            // punkty bez pary — węzeł wewnątrz komórki
            let hub_ids: Vec<usize> = step_ids.iter().copied().filter(|&q| partner[q].is_none()).collect();
            let hub = if hub_ids.is_empty() {
                None
            } else if hub_ids.len() >= 2 {
                let c = hub_ids.iter().fold(Vec2::ZERO, |a, &q| a + perim[q].p) * (1.0 / hub_ids.len() as f64);
                Some(c)
            } else {
                ok = false;
                None
            };
            // poprzedni (w kolejności obwodu) punkt węzła — przeskok przez węzeł
            let hub_prev = |q: usize| -> usize {
                let k = hub_ids.iter().position(|&x| x == q).unwrap();
                hub_ids[(k + hub_ids.len() - 1) % hub_ids.len()]
            };
            // spójność wysokości regionów
            if ok {
                for &q in &step_ids {
                    let (r, z_q, z_r) = match partner[q] {
                        // za q i przed partnerem to ten sam region
                        Some(r) => (r, perim[q].z_after, perim[r].z_before),
                        // przy węźle: przed q i za poprzednim punktem węzła
                        None => {
                            let r = hub_prev(q);
                            (r, perim[q].z_before, perim[r].z_after)
                        }
                    };
                    let _ = r;
                    if (z_q - z_r).abs() > 0.05 {
                        ok = false;
                    }
                }
            }
            // cięciwy nie mogą się przecinać
            if ok {
                let chords: Vec<(usize, usize)> =
                    step_ids.iter().filter_map(|&q| partner[q].filter(|&r| r > q).map(|r| (q, r))).collect();
                for (x, &(a, b)) in chords.iter().enumerate() {
                    for &(c, d) in &chords[x + 1..] {
                        let inside = |v: usize| v > a && v < b;
                        if inside(c) != inside(d) {
                            ok = false;
                        }
                    }
                    // węzeł nie może leżeć po obu stronach cięciwy
                    if !hub_ids.is_empty() {
                        let inside: Vec<bool> = hub_ids.iter().map(|&v| v > a && v < b).collect();
                        if inside.iter().any(|&x| x) && inside.iter().any(|&x| !x) {
                            ok = false;
                        }
                    }
                }
            }
            if !ok {
                if debug && debug_left > 0 {
                    debug_left -= 1;
                    eprintln!("--- komórka ({i},{j}) narożniki {hs:?}");
                    for (q, pt) in perim.iter().enumerate() {
                        eprintln!(
                            "  {q}: {:?} p=({:.4},{:.4}) przed {:.3} za {:.3} partner {:?}",
                            pt.corner, pt.p.x, pt.p.y, pt.z_before, pt.z_after, partner[q]
                        );
                    }
                }
                smooth(&mut out);
                continue;
            }

            // --- regiony: obchodzenie obwodu z przeskokami po cięciwach i przez węzeł ---
            let np = perim.len();
            let mut used_start = vec![false; np];
            for start in 0..np {
                if used_start[start] {
                    continue;
                }
                let mut poly: Vec<(Vec2, f32, [f32; 3], Option<usize>)> = Vec::new();
                let mut q = start;
                let mut guard = 0;
                loop {
                    guard += 1;
                    if guard > 64 {
                        break;
                    }
                    used_start[q] = true;
                    let pt = perim[q];
                    poly.push((pt.p, pt.z_after, pt.n_after, pt.corner));
                    let next = (q + 1) % np;
                    let pn = perim[next];
                    if pn.corner.is_some() {
                        q = next;
                    } else {
                        poly.push((pn.p, pn.z_before, pn.n_before, None));
                        q = match partner[next] {
                            Some(r) => r,
                            None => {
                                poly.push((hub.unwrap(), pn.z_before, [0.0, 0.0, 1.0], None));
                                hub_prev(next)
                            }
                        };
                    }
                    if q == start {
                        break;
                    }
                }
                if poly.len() < 3 {
                    continue;
                }
                let all_through = poly.iter().all(|v| through(v.1));
                if all_through {
                    continue;
                }
                let mut top_ids = Vec::with_capacity(poly.len());
                let mut bot_ids = Vec::with_capacity(poly.len());
                for &(p, z, n, corner) in &poly {
                    match corner {
                        Some(k) => {
                            let (ci, cj) = corners[k];
                            top_ids.push(node(ci, cj));
                            bot_ids.push(bnode(ci, cj));
                        }
                        None => {
                            top_ids.push(push(&mut out, p, z, n, 0));
                            bot_ids.push(push(&mut out, p, m.bottom, [0.0, 0.0, -1.0], 2));
                        }
                    }
                }
                fan(&mut out, &top_ids);
                bot_ids.reverse();
                fan(&mut out, &bot_ids);
            }

            // --- ściany: cięciwy i odcinki do węzła ---
            let add_wall = |a: PerimPt, b_p: Vec2, b_top: f32, b_bot: f32, b_key: (EdgeKey, usize), wall_segs: &mut Vec<WallSeg>| {
                let top = [a.z_before.max(a.z_after), b_top];
                let bot = [a.z_before.min(a.z_after), b_bot];
                let dir = (b_p - a.p).normalized();
                // region „za” a leży po prawej stronie odcinka a → b
                let mut nrm = Vec2::new(dir.y, -dir.x);
                if a.z_after >= a.z_before {
                    nrm = -nrm;
                }
                wall_segs.push(WallSeg { keys: [a.key, b_key], p: [a.p, b_p], top, bot, n: nrm });
            };
            for &q in &step_ids {
                let a = perim[q];
                match partner[q] {
                    Some(r) if r > q => {
                        let b = perim[r];
                        add_wall(a, b.p, b.z_before.max(b.z_after), b.z_before.min(b.z_after), b.key, &mut wall_segs);
                    }
                    Some(_) => {}
                    None => {
                        let (t, bt) = (a.z_before.max(a.z_after), a.z_before.min(a.z_after));
                        let hub_key = (a.key.0, a.key.1 + 1000);
                        add_wall(a, hub.unwrap(), t, bt, hub_key, &mut wall_segs);
                    }
                }
            }
        }
    }

    // ściany z normalnymi uśrednionymi w punktach wspólnych sąsiednich komórek
    let mut acc: HashMap<(EdgeKey, usize), Vec2> = HashMap::with_capacity(wall_segs.len() * 2);
    for w in &wall_segs {
        let len = w.p[0].distance(w.p[1]).max(1e-6);
        for k in w.keys {
            let e = acc.entry(k).or_insert(Vec2::ZERO);
            *e = *e + w.n * len;
        }
    }
    for w in &wall_segs {
        let nv = |k: (EdgeKey, usize)| {
            let v = acc.get(&k).copied().unwrap_or(w.n);
            // ostre narożniki (> ~50°) zostają ostre
            let v = if v.normalized().dot(w.n) < 0.64 { w.n } else { v.normalized() };
            [v.x as f32, v.y as f32, 0.0]
        };
        let (n0, n1) = (nv(w.keys[0]), nv(w.keys[1]));
        let a = push(&mut out, w.p[0], w.top[0], n0, 1);
        let b = push(&mut out, w.p[1], w.top[1], n1, 1);
        let c = push(&mut out, w.p[1], w.bot[1], n1, 1);
        let d = push(&mut out, w.p[0], w.bot[0], n0, 1);
        out.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    // boki półfabrykatu
    let bottom = m.bottom;
    let side = |out: &mut SimMesh, p0: (usize, usize), p1: (usize, usize), n: [f32; 3]| {
        let (h0, h1) = (h(p0.0, p0.1), h(p1.0, p1.1));
        if through(h0) && through(h1) {
            return;
        }
        let (c0, c1) = (m.center(p0.0, p0.1), m.center(p1.0, p1.1));
        let a = push(out, c0, h0, n, 2);
        let b = push(out, c1, h1, n, 2);
        let c = push(out, c1, bottom, n, 2);
        let d = push(out, c0, bottom, n, 2);
        out.indices.extend_from_slice(&[a, b, c, a, c, d]);
    };
    for i in 0..nx - 1 {
        side(&mut out, (i, 0), (i + 1, 0), [0.0, -1.0, 0.0]);
        side(&mut out, (i, ny - 1), (i + 1, ny - 1), [0.0, 1.0, 0.0]);
    }
    for j in 0..ny - 1 {
        side(&mut out, (0, j), (0, j + 1), [-1.0, 0.0, 0.0]);
        side(&mut out, (nx - 1, j), (nx - 1, j + 1), [1.0, 0.0, 0.0]);
    }
    out
}

/// Łączy uskoki o tej samej parze poziomów w cięciwy (ściany). Bez pary → None.
fn pair_steps(perim: &[PerimPt]) -> Vec<Option<usize>> {
    let mut partner: Vec<Option<usize>> = vec![None; perim.len()];
    let step_ids: Vec<usize> = (0..perim.len()).filter(|&q| perim[q].corner.is_none()).collect();
    let class = |q: usize| {
        let a = perim[q].z_before;
        let b = perim[q].z_after;
        (a.max(b), a.min(b))
    };
    let same = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05;
    let mut left: Vec<usize> = step_ids;
    while let Some(q) = left.first().copied() {
        let cq = class(q);
        let group: Vec<usize> = left.iter().copied().filter(|&r| same(class(r), cq)).collect();
        let mut pair = |a: usize, b: usize| {
            partner[a] = Some(b);
            partner[b] = Some(a);
        };
        match group.len() {
            2 if (perim[group[0]].z_after - perim[group[1]].z_before).abs() < 0.05 => pair(group[0], group[1]),
            4 => {
                let d = |a: usize, b: usize| perim[a].p.distance(perim[b].p);
                let (g0, g1, g2, g3) = (group[0], group[1], group[2], group[3]);
                if d(g0, g1) + d(g2, g3) <= d(g1, g2) + d(g3, g0) {
                    pair(g0, g1);
                    pair(g2, g3);
                } else {
                    pair(g1, g2);
                    pair(g3, g0);
                }
            }
            _ => {} // zostają bez pary → węzeł
        }
        left.retain(|r| !group.contains(r));
    }
    partner
}

fn fan(out: &mut SimMesh, poly: &[u32]) {
    for k in 1..poly.len().saturating_sub(1) {
        out.indices.extend_from_slice(&[poly[0], poly[k], poly[k + 1]]);
    }
}

#[cfg(test)]
mod debug_tests {
    use crate::*;
    use cam_geom::{Aabb2, Vec2};
    use cam_ops::P3;

    fn bad_tris(m: &SimMesh) -> usize {
        m.indices
            .chunks(3)
            .filter(|t| {
                let v = [t[0], t[1], t[2]].map(|i| m.verts[i as usize]);
                if !v.iter().all(|v| v.kind == 0) {
                    return false;
                }
                let zs = v.map(|v| v.pos[2]);
                zs.iter().cloned().fold(f32::MIN, f32::max) - zs.iter().cloned().fold(f32::MAX, f32::min) > 0.5
            })
            .count()
    }

    #[test]
    fn tab_ends_and_ramps_next_to_walls() {
        let area = Aabb2 { min: Vec2::new(0.0, -10.0), max: Vec2::new(60.0, 10.0) };
        // przelot z mostkiem (−16) i rampą zaraz przy ścianie kieszeni
        let pts = [
            (P3::new(5.0, 0.3, 5.0), true, 0.0),
            (P3::new(5.0, 0.3, -18.0), false, 300.0),
            (P3::new(25.0, 1.1, -18.0), false, 1000.0),
            (P3::new(25.0, 1.1, -16.0), false, 300.0),
            (P3::new(33.0, 1.4, -16.0), false, 1000.0),
            (P3::new(33.0, 1.4, -18.0), false, 300.0),
            (P3::new(55.0, 2.2, -18.0), false, 1000.0),
            (P3::new(55.0, 2.2, 5.0), true, 0.0),
            (P3::new(10.0, 5.0, 5.0), true, 0.0),
            (P3::new(10.0, 5.0, -2.0), false, 300.0),
            (P3::new(50.0, 5.0, -2.0), false, 1000.0),
            (P3::new(10.0, 5.6, -4.0), false, 1000.0), // rampa w dół obok ściany
        ];
        let op = SimOp { tool: ToolShape::flat(3.0), points: &pts };
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -18.0, 150), &[op], 3000.0);
        let dur = sim.duration();
        for f in [0.3, 0.6, 0.85, 1.0] {
            sim.seek(dur * f);
            assert_eq!(bad_tris(&sim.mesh()), 0, "t = {f}");
        }
    }

    #[test]
    fn groove_through_cut_has_no_fallback() {
        let area = Aabb2 { min: Vec2::new(0.0, -10.0), max: Vec2::new(60.0, 10.0) };
        for (y0, y1) in [(0.0, 0.0), (0.0, 7.0), (-3.0, 4.0)] {
            let pts = [
                (P3::new(5.0, y0, 5.0), true, 0.0),
                (P3::new(5.0, y0, -20.0), false, 300.0),
                (P3::new(55.0, y1, -20.0), false, 1000.0),
            ];
            let op = SimOp { tool: ToolShape::flat(3.0), points: &pts };
            let mut sim = Simulator::new(Heightmap::new(area, 0.0, -18.0, 140), &[op], 3000.0);
            sim.advance_to(sim.duration());
            let m = sim.mesh();
            assert_eq!(bad_tris(&m), 0, "rowek {y0}->{y1}");
        }
    }
}
