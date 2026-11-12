//! Symulacja na bryłach (prototyp): półfabrykat jako bryła, każdy ruch narzędzia
//! jako bryła przejścia (otoczka wypukła narzędzia w punkcie początkowym i
//! końcowym — narzędzia są wypukłe), usuwanie materiału = różnica brył.

use crate::ToolShape;
use cam_ops::P3;
use manifold_rust::linalg::Vec3;
use manifold_rust::manifold::Manifold;

/// Pierścienie profilu narzędzia: (promień, wysokość nad czubkiem).
pub fn tool_rings(tool: &ToolShape, height: f64) -> Vec<(f64, f64)> {
    let r = tool.radius;
    let n = 8;
    let mut rings: Vec<(f64, f64)> = (0..=n)
        .map(|k| {
            let d = r * k as f64 / n as f64;
            (d, tool.profile(d).unwrap_or(0.0))
        })
        .collect();
    rings.push((r, height));
    // pierścienie leżące na prostej między sąsiadami nie zmieniają otoczki
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(rings.len());
    for (k, &p) in rings.iter().enumerate() {
        if k > 0 && k + 1 < rings.len() {
            let (a, b) = (out[out.len() - 1], rings[k + 1]);
            let cross = (p.0 - a.0) * (b.1 - a.1) - (p.1 - a.1) * (b.0 - a.0);
            if cross.abs() < 1e-9 {
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// Punkty narzędzia (względem czubka) do otoczki wypukłej.
pub fn tool_points(tool: &ToolShape, height: f64, segments: usize) -> Vec<Vec3> {
    let mut pts = Vec::new();
    for (rad, z) in tool_rings(tool, height) {
        if rad < 1e-9 {
            pts.push(Vec3::new(0.0, 0.0, z));
            continue;
        }
        for k in 0..segments {
            let a = k as f64 / segments as f64 * std::f64::consts::TAU;
            pts.push(Vec3::new(rad * a.cos(), rad * a.sin(), z));
        }
    }
    pts
}

/// Bryła przejścia narzędzia z `a` do `b`.
pub fn sweep(tool_pts: &[Vec3], a: P3, b: P3) -> Manifold {
    let mut pts = Vec::with_capacity(tool_pts.len() * 2);
    for p in tool_pts {
        pts.push(Vec3::new(p.x + a.x, p.y + a.y, p.z + a.z));
        if a.distance(b) > 1e-9 {
            pts.push(Vec3::new(p.x + b.x, p.y + b.y, p.z + b.z));
        }
    }
    Manifold::hull(&pts)
}

/// Półfabrykat jako prostopadłościan.
pub fn stock(min: P3, max: P3) -> Manifold {
    Manifold::cube(Vec3::new(max.x - min.x, max.y - min.y, max.z - min.z), false).translate(Vec3::new(min.x, min.y, min.z))
}

use crate::{SimMesh, SimVertex, Simulator};

/// Odcinek ścieżki do wycięcia w bryle.
#[derive(Clone, Copy)]
struct CutSeg {
    a: P3,
    b: P3,
    op: usize,
    t0: f64,
    t1: f64,
}

/// Symulacja bryłowa sterowana czasem programu, z klatkami kluczowymi.
pub struct SolidSim {
    tools: Vec<Vec<Vec3>>,
    segs: Vec<CutSeg>,
    /// (liczba w pełni wyciętych odcinków, bryła).
    keys: Vec<(usize, Manifold)>,
    /// Stan bieżący: liczba wyciętych odcinków i bryła.
    cur: (usize, Manifold),
    top: f64,
    bottom: f64,
    /// Czas: budowa brył przejść, sumowanie, odejmowanie [s].
    pub t_hull: f64,
    pub t_union: f64,
    pub t_diff: f64,
}

impl SolidSim {
    pub fn stats(&self) -> String {
        format!("otoczki {:.2} s, sumy {:.2} s, różnice {:.2} s", self.t_hull, self.t_union, self.t_diff)
    }

    /// Buduje z ścieżki symulatora mapy wysokości (te same czasy i narzędzia).
    pub fn new(sim: &Simulator, min: P3, max: P3) -> Self {
        let height = (max.z - min.z) + 30.0;
        let tools: Vec<Vec<Vec3>> = sim
            .tools
            .iter()
            .map(|t| tool_points(t, height, ((t.radius * 10.0) as usize).clamp(24, 64)))
            .collect();
        let mut segs = Vec::new();
        for w in sim.path.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.p.z.min(b.p.z) >= max.z || a.p.distance(b.p) < 1e-9 {
                continue;
            }
            segs.push(CutSeg { a: a.p, b: b.p, op: b.op, t0: a.t, t1: b.t });
        }
        let s = stock(min, max);
        SolidSim { tools, segs, keys: vec![(0, s.clone())], cur: (0, s), top: max.z, bottom: min.z, t_hull: 0.0, t_union: 0.0, t_diff: 0.0 }
    }

    pub fn segment_count(&self) -> usize {
        self.segs.len()
    }

    fn cut_range(&mut self, solid: &Manifold, from: usize, to: usize) -> Manifold {
        let mut s = solid.clone();
        let mut k = from;
        while k < to {
            let end = (k + 48).min(to);
            let t0 = std::time::Instant::now();
            let batch: Vec<Manifold> = self.segs[k..end].iter().map(|c| sweep(&self.tools[c.op], c.a, c.b)).collect();
            let t1 = std::time::Instant::now();
            let cut = if batch.len() == 1 { batch.into_iter().next().unwrap() } else { union_tree(batch) };
            let t2 = std::time::Instant::now();
            s = s.difference(&cut).as_original().simplify(SIMPLIFY_TOL);
            let t3 = std::time::Instant::now();
            self.t_hull += (t1 - t0).as_secs_f64();
            self.t_union += (t2 - t1).as_secs_f64();
            self.t_diff += (t3 - t2).as_secs_f64();
            k = end;
        }
        s
    }

    /// Przeliczenie całości z klatkami kluczowymi co `every` odcinków; `progress(0..1)`, false = przerwij.
    ///
    /// Najpierw (równolegle) dla każdego fragmentu między klatkami powstaje suma
    /// brył przejść, potem fragmenty są kolejno odejmowane od półfabrykatu.
    pub fn precompute(&mut self, every: usize, mut progress: impl FnMut(f32) -> bool) -> bool {
        use rayon::prelude::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let n = self.segs.len();
        let every = every.max(1);
        let ranges: Vec<(usize, usize)> = (0..n).step_by(every).map(|k| (k, (k + every).min(n))).collect();
        let done = AtomicUsize::new(0);
        let t0 = std::time::Instant::now();
        let cutters: Vec<Manifold> = {
            let (tools, segs) = (&self.tools, &self.segs);
            ranges
                .par_iter()
                .map(|&(a, b)| {
                    let hulls: Vec<Manifold> = segs[a..b].iter().map(|c| sweep(&tools[c.op], c.a, c.b)).collect();
                    let u = union_tree(hulls);
                    done.fetch_add(1, Ordering::Relaxed);
                    u
                })
                .collect()
        };
        self.t_union += t0.elapsed().as_secs_f64();
        // postęp: sumy ≈ połowa pracy
        if !progress(0.5) {
            return false;
        }
        let mut s = self.keys[0].1.clone();
        for (idx, (cut, &(_, end))) in cutters.iter().zip(&ranges).enumerate() {
            let t = std::time::Instant::now();
            s = s.difference(cut).as_original().simplify(SIMPLIFY_TOL);
            self.t_diff += t.elapsed().as_secs_f64();
            self.keys.push((end, s.clone()));
            if !progress(0.5 + 0.5 * (idx + 1) as f32 / ranges.len() as f32) {
                return false;
            }
        }
        let _ = done.load(Ordering::Relaxed);
        self.cur = (0, self.keys[0].1.clone());
        true
    }

    /// Bryła w chwili `t` (z częściowo wykonanym bieżącym ruchem).
    pub fn at(&mut self, t: f64) -> Manifold {
        // odcinki w pełni wykonane do czasu t
        let done = self.segs.partition_point(|c| c.t1 <= t);
        if done < self.cur.0 || done - self.cur.0 > 64 {
            // start od najbliższej wcześniejszej klatki
            let ki = self.keys.partition_point(|k| k.0 <= done).saturating_sub(1);
            let (kk, ref solid) = self.keys[ki];
            if kk > self.cur.0 || done < self.cur.0 {
                self.cur = (kk, solid.clone());
            }
        }
        if done > self.cur.0 {
            let base = self.cur.1.clone();
            let s = self.cut_range(&base, self.cur.0, done);
            self.cur = (done, s);
        }
        // część bieżącego odcinka
        if let Some(c) = self.segs.get(done) {
            if t > c.t0 && c.t1 > c.t0 {
                let f = ((t - c.t0) / (c.t1 - c.t0)).clamp(0.0, 1.0);
                let q = P3::new(c.a.x + (c.b.x - c.a.x) * f, c.a.y + (c.b.y - c.a.y) * f, c.a.z + (c.b.z - c.a.z) * f);
                return self.cur.1.difference(&sweep(&self.tools[c.op], c.a, q));
            }
        }
        self.cur.1.clone()
    }

    /// Siatka do wyświetlenia: gładkie normalne z zachowaniem ostrych krawędzi.
    pub fn mesh(&self, solid: &Manifold) -> SimMesh {
        mesh_of(solid, self.top as f32, self.bottom as f32)
    }
}

/// Tolerancja upraszczania siatki po operacjach boolowskich [mm] — scala
/// rozdrobnione trójkąty leżące w jednej płaszczyźnie (bez zmiany kształtu).
const SIMPLIFY_TOL: f64 = 1e-4;

/// Suma brył parami (drzewo), poziomy liczone równolegle, z upraszczaniem siatki.
fn union_tree(mut v: Vec<Manifold>) -> Manifold {
    use rayon::prelude::*;
    while v.len() > 1 {
        let pairs: Vec<(Manifold, Option<Manifold>)> = {
            let mut it = v.into_iter();
            let mut out = Vec::new();
            while let Some(a) = it.next() {
                out.push((a, it.next()));
            }
            out
        };
        let f = |(a, b): (Manifold, Option<Manifold>)| match b {
            Some(b) => a.union(&b).as_original().simplify(SIMPLIFY_TOL),
            None => a,
        };
        v = pairs.into_par_iter().map(f).collect();
    }
    v.pop().unwrap_or_else(Manifold::new)
}

/// Bryła → siatka z normalnymi wygładzonymi tylko w obrębie łagodnych krawędzi (< 35°).
pub fn mesh_of(m: &Manifold, _top: f32, bottom: f32) -> SimMesh {
    let gl = m.get_mesh_gl(-1);
    let np = gl.num_prop as usize;
    let pos = |i: u32| {
        let o = i as usize * np;
        [gl.vert_properties[o], gl.vert_properties[o + 1], gl.vert_properties[o + 2]]
    };
    let tris: Vec<[u32; 3]> = gl.tri_verts.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
    let face_n: Vec<[f32; 3]> = tris
        .iter()
        .map(|t| {
            let (a, b, c) = (pos(t[0]), pos(t[1]), pos(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
            [n[0] / l, n[1] / l, n[2] / l]
        })
        .collect();
    let nv = gl.vert_properties.len() / np.max(1);
    let mut incident: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (f, t) in tris.iter().enumerate() {
        for &v in t {
            incident[v as usize].push(f as u32);
        }
    }
    let cos_lim = 35f32.to_radians().cos();
    let mut out = SimMesh::default();
    for (f, t) in tris.iter().enumerate() {
        let fnrm = face_n[f];
        for &v in t {
            let mut n = [0.0f32; 3];
            for &g in &incident[v as usize] {
                let gn = face_n[g as usize];
                if gn[0] * fnrm[0] + gn[1] * fnrm[1] + gn[2] * fnrm[2] > cos_lim {
                    n = [n[0] + gn[0], n[1] + gn[1], n[2] + gn[2]];
                }
            }
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
            let n = [n[0] / l, n[1] / l, n[2] / l];
            let p = pos(v);
            // powierzchnie poziome (góra, dna) / spód / ściany
            let kind = if fnrm[2] > 0.9 {
                0
            } else if fnrm[2] < -0.9 && (p[2] - bottom).abs() < 1e-3 {
                2
            } else {
                1
            };
            out.verts.push(SimVertex { pos: p, normal: n, kind });
            out.indices.push((out.verts.len() - 1) as u32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Heightmap, SimOp};
    use cam_geom::{Aabb2, Vec2};

    #[test]
    fn solid_slot_volume_matches_analytic() {
        // rowek: frez Ø6 na głębokość 2 mm, długość 30 mm → objętość = 2·(30·6 + π·9)
        let area = Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(50.0, 20.0) };
        let pts = [
            (P3::new(10.0, 10.0, 5.0), true, 0.0),
            (P3::new(10.0, 10.0, -2.0), false, 300.0),
            (P3::new(40.0, 10.0, -2.0), false, 1000.0),
            (P3::new(40.0, 10.0, 5.0), true, 0.0),
        ];
        let op = SimOp { tool: ToolShape::flat(6.0), points: &pts };
        let sim = Simulator::new(Heightmap::new(area, 0.0, -10.0, 100), &[op], 3000.0);
        let mut s = SolidSim::new(&sim, P3::new(0.0, 0.0, -10.0), P3::new(50.0, 20.0, 0.0));
        assert!(s.precompute(1, |_| true));
        let full = 50.0 * 20.0 * 10.0;
        let removed = full - s.at(sim.duration()).volume();
        let expect = 2.0 * (30.0 * 6.0 + std::f64::consts::PI * 9.0);
        // wielokąt 30-boczny zamiast okręgu — błąd < 1%
        assert!((removed - expect).abs() < 0.01 * expect, "{removed} vs {expect}");
        // w połowie przejazdu wycięta mniej więcej połowa rowka
        let mid = full - s.at(sim.path[2].t * 0.5 + sim.path[1].t * 0.5).volume();
        assert!(mid > 0.3 * expect && mid < 0.7 * expect, "{mid}");
    }
}
