//! Symulacja na bryłach (prototyp): półfabrykat jako bryła, każdy ruch narzędzia
//! jako bryła przejścia (otoczka wypukła narzędzia w punkcie początkowym i
//! końcowym — narzędzia są wypukłe), usuwanie materiału = różnica brył.

use cam_i18n::tr;
use crate::ToolShape;
use cam_ops::P3;
use manifold_rust::linalg::Vec3;
use manifold_rust::manifold::Manifold;

/// Pierścienie profilu narzędzia: (promień, wysokość nad czubkiem).
pub fn tool_rings(tool: &ToolShape, height: f64) -> Vec<(f64, f64)> {
    let r = tool.radius;
    // pierścienie jak w siatce narzędzia: gęściej na kulkach i zaokrągleniach
    let mut rings: Vec<(f64, f64)> = std::iter::once(0.0)
        .chain(tool.ring_radii())
        .map(|d| (d, tool.profile(d).unwrap_or(0.0)))
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
use std::sync::Arc;

/// Odcinek ścieżki do wycięcia w bryle.
#[derive(Clone, Copy)]
struct CutSeg {
    a: P3,
    b: P3,
    op: usize,
    t0: f64,
    t1: f64,
    /// Zasięg bryły przejścia z góry: x0, y0, x1, y1.
    bb: [f64; 4],
}

/// Podział półfabrykatu na kolumny (kafelki). Każdy ruch narzędzia zmienia tylko
/// kafelki, których dotyka — różnice liczone są na małych bryłach, a siatki
/// pozostałych kafelków biorą się z pamięci. Bez tego każda klatka przeliczała
/// całą bryłę, która po obróbce wykańczającej ma setki tysięcy trójkątów.
struct Tiles {
    x0: f64,
    y0: f64,
    size: f64,
    nx: usize,
    ny: usize,
}

/// Kafelki zachodzą na sąsiadów o tyle [mm]: na styku stykających się krawędzi
/// powstawały włoskowate szczeliny (widoczne jako przerywane linie na górze płyty).
const TILE_OVERLAP: f64 = 0.02;

impl Tiles {
    /// Kafelki dotykane przez prostokąt (x0, y0, x1, y1).
    fn touched(&self, bb: [f64; 4]) -> impl Iterator<Item = usize> + '_ {
        // z zakładką: przejście tuż obok kafelka wcina się też w jego zakładkę
        let bb = [bb[0] - TILE_OVERLAP, bb[1] - TILE_OVERLAP, bb[2] + TILE_OVERLAP, bb[3] + TILE_OVERLAP];
        let cl = |v: f64, n: usize| (v.floor().max(0.0) as usize).min(n - 1);
        let (i0, i1) = (cl((bb[0] - self.x0) / self.size, self.nx), cl((bb[2] - self.x0) / self.size, self.nx));
        let (j0, j1) = (cl((bb[1] - self.y0) / self.size, self.ny), cl((bb[3] - self.y0) / self.size, self.ny));
        (j0..=j1).flat_map(move |j| (i0..=i1).map(move |i| j * self.nx + i))
    }
}

type State = Vec<Arc<Manifold>>;

/// Symulacja bryłowa sterowana czasem programu, z klatkami kluczowymi.
pub struct SolidSim {
    tools: Vec<Vec<Vec3>>,
    segs: Vec<CutSeg>,
    tiles: Tiles,
    /// (liczba w pełni wyciętych odcinków, kafelki) — niezmienione kafelki są współdzielone.
    keys: Vec<(usize, State)>,
    /// Stan bieżący: liczba wyciętych odcinków i kafelki.
    cur: (usize, State),
    /// Siatki kafelków (bryła, z której powstała siatka).
    meshes: Vec<Option<(Arc<Manifold>, Arc<SimMesh>)>>,
    /// Wewnętrzne boki każdego kafelka (płaszczyzny x albo y na styku z sąsiadem) —
    /// pomijane w siatce do wyświetlania.
    inner: Vec<Vec<(usize, f64)>>,
    top: f64,
    bottom: f64,
    /// Bryła przerosła limit trójkątów (liczba trójkątów) — przeliczenie przerwane.
    pub too_big: Option<usize>,
    /// Czas: budowa brył przejść i sumowanie, odejmowanie [s].
    pub t_hull: f64,
    pub t_union: f64,
    pub t_diff: f64,
}

impl SolidSim {
    pub fn stats(&self) -> String {
        tr!("otoczki {:.2} s, sumy {:.2} s, różnice {:.2} s", self.t_hull, self.t_union, self.t_diff)
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
            let r = sim.tools[b.op].radius + 1e-3;
            let bb = [a.p.x.min(b.p.x) - r, a.p.y.min(b.p.y) - r, a.p.x.max(b.p.x) + r, a.p.y.max(b.p.y) + r];
            segs.push(CutSeg { a: a.p, b: b.p, op: b.op, t0: a.t, t1: b.t, bb });
        }
        // kafelki ok. 1/10 większego wymiaru, nie mniejsze niż 8 mm
        let (w, h) = (max.x - min.x, max.y - min.y);
        let size = (w.max(h) / 10.0).max(8.0);
        let (nx, ny) = (((w / size).ceil() as usize).max(1), ((h / size).ceil() as usize).max(1));
        let tiles = Tiles { x0: min.x, y0: min.y, size, nx, ny };
        let mut inner = vec![Vec::new(); nx * ny];
        let state: State = (0..nx * ny)
            .map(|k| {
                let (i, j) = (k % nx, k / nx);
                let lo = P3::new(min.x + i as f64 * size, min.y + j as f64 * size, min.z);
                let hi = P3::new((lo.x + size).min(max.x), (lo.y + size).min(max.y), max.z);
                // zakładka na sąsiadów (nie poza półfabrykat)
                let lo = P3::new((lo.x - TILE_OVERLAP).max(min.x), (lo.y - TILE_OVERLAP).max(min.y), lo.z);
                let hi = P3::new((hi.x + TILE_OVERLAP).min(max.x), (hi.y + TILE_OVERLAP).min(max.y), hi.z);
                // boki wewnątrz półfabrykatu (nie na jego brzegu)
                for (axis, v, edge) in [(0, lo.x, min.x), (0, hi.x, max.x), (1, lo.y, min.y), (1, hi.y, max.y)] {
                    if (v - edge).abs() > 1e-9 {
                        inner[k].push((axis, v));
                    }
                }
                Arc::new(stock(lo, hi))
            })
            .collect();
        SolidSim {
            tools,
            segs,
            tiles,
            keys: vec![(0, state.clone())],
            cur: (0, state),
            meshes: vec![None; nx * ny],
            inner,
            too_big: None,
            top: max.z,
            bottom: min.z,
            t_hull: 0.0,
            t_union: 0.0,
            t_diff: 0.0,
        }
    }

    pub fn segment_count(&self) -> usize {
        self.segs.len()
    }

    /// Wspólna bryła wszystkich przejść z `segs` i kafelki, których dotyka.
    fn cutter(tools: &[Vec<Vec3>], tiles: &Tiles, segs: &[CutSeg]) -> (Manifold, Vec<usize>) {
        use rayon::prelude::*;
        let hulls: Vec<Manifold> = segs.par_iter().map(|c| sweep(&tools[c.op], c.a, c.b)).collect();
        let mut touched: Vec<usize> = segs.iter().flat_map(|c| tiles.touched(c.bb)).collect();
        touched.sort_unstable();
        touched.dedup();
        (union_tree(hulls), touched)
    }

    /// Odejmuje bryłę od wskazanych kafelków (kafelki niezależne — równolegle).
    fn subtract(&mut self, state: &mut State, cutter: &Manifold, touched: &[usize]) {
        use rayon::prelude::*;
        let t0 = std::time::Instant::now();
        let cur: &State = state;
        let out: Vec<(usize, Arc<Manifold>)> =
            touched.par_iter().map(|&t| (t, Arc::new(cur[t].difference(cutter).as_original().simplify(SIMPLIFY_TOL)))).collect();
        for (t, s) in out {
            state[t] = s;
        }
        self.t_diff += t0.elapsed().as_secs_f64();
    }

    /// Wycina odcinki `segs` z kafelków.
    fn cut(&mut self, state: &mut State, segs: &[CutSeg]) {
        let t0 = std::time::Instant::now();
        let (cutter, touched) = Self::cutter(&self.tools, &self.tiles, segs);
        self.t_union += t0.elapsed().as_secs_f64();
        self.subtract(state, &cutter, &touched);
    }

    /// Przeliczenie całości z klatkami kluczowymi co `every` odcinków; `progress(0..1)`, false = przerwij.
    /// Dla każdego fragmentu między klatkami powstaje suma brył przejść (równolegle,
    /// porcjami), potem fragmenty są kolejno odejmowane od dotkniętych kafelków.
    /// Klatki dzielą niezmienione kafelki; razem mieszczą się w `KEY_TRIANGLES`.
    pub fn precompute(&mut self, every: usize, mut progress: impl FnMut(f32) -> bool) -> bool {
        use rayon::prelude::*;
        let n = self.segs.len();
        let every = every.max(1);
        let ranges: Vec<(usize, usize)> = (0..n).step_by(every).map(|k| (k, (k + every).min(n))).collect();
        let mut state = self.keys[0].1.clone();
        let mut stride = 1;
        // porcjami: bryły przejść dla kilkudziesięciu fragmentów naraz (równolegle),
        // potem odejmowane po kolei i od razu zwalniane — w pamięci tylko jedna porcja
        let chunk = rayon::current_num_threads().max(4) * 4;
        for (ci, part) in ranges.chunks(chunk).enumerate() {
            let t0 = std::time::Instant::now();
            let cutters: Vec<(Manifold, Vec<usize>)> = {
                let (tools, tiles, segs) = (&self.tools, &self.tiles, &self.segs);
                part.par_iter().map(|&(a, b)| Self::cutter(tools, tiles, &segs[a..b])).collect()
            };
            self.t_union += t0.elapsed().as_secs_f64();
            for (k, ((cutter, touched), &(_, end))) in cutters.into_iter().zip(part).enumerate() {
                self.subtract(&mut state, &cutter, &touched);
                // za szczegółowa bryła: rysowanie i odtwarzanie byłyby bardzo wolne (i za duże dla karty)
                let tris: usize = state.iter().map(|t| t.num_tri()).sum();
                if tris > MAX_TRIANGLES {
                    self.too_big = Some(tris);
                    return false;
                }
                if end == n || (end / every) % stride == 0 {
                    self.keys.push((end, state.clone()));
                }
                let idx = ci * chunk + k;
                if !progress((idx + 1) as f32 / ranges.len() as f32) {
                    return false;
                }
            }
            self.thin_keys(KEY_TRIANGLES, every, &mut stride);
        }
        self.cur = (0, self.keys[0].1.clone());
        true
    }

    /// Przerzedza klatki kluczowe równomiernie (zostaje co `stride`-ty fragment, pierwsza
    /// i ostatnia klatka), aż razem mają najwyżej `budget` trójkątów. Odtwarzanie ich nie
    /// potrzebuje (każda klatka wycina tylko swój kawałek ścieżki) — przyspieszają przewijanie.
    fn thin_keys(&mut self, budget: usize, every: usize, stride: &mut usize) {
        let n = self.segs.len();
        while self.keys.len() > 2 && self.keyframe_triangles() > budget {
            *stride *= 2;
            let s = *stride;
            self.keys.retain(|k| k.0 == 0 || k.0 == n || (k.0 / every) % s == 0);
        }
    }

    /// Przesuwa stan bieżący do `done` w pełni wyciętych odcinków.
    fn advance(&mut self, done: usize) {
        // start od najbliższej wcześniejszej klatki, jeśli jest bliżej niż stan bieżący
        let ki = self.keys.partition_point(|k| k.0 <= done).saturating_sub(1);
        let kk = self.keys[ki].0;
        if done < self.cur.0 || kk > self.cur.0 {
            self.cur = (kk, self.keys[ki].1.clone());
        }
        if done > self.cur.0 {
            let mut state = std::mem::take(&mut self.cur.1);
            let segs = self.segs[self.cur.0..done].to_vec();
            self.cut(&mut state, &segs);
            self.cur = (done, state);
        }
    }

    /// Kafelki w chwili `t` (z częściowo wykonanym bieżącym ruchem).
    fn state_at(&mut self, t: f64) -> State {
        let done = self.segs.partition_point(|c| c.t1 <= t);
        self.advance(done);
        let mut state = self.cur.1.clone();
        if let Some(c) = self.segs.get(done).copied() {
            if t > c.t0 && c.t1 > c.t0 {
                let f = ((t - c.t0) / (c.t1 - c.t0)).clamp(0.0, 1.0);
                let q = P3::new(c.a.x + (c.b.x - c.a.x) * f, c.a.y + (c.b.y - c.a.y) * f, c.a.z + (c.b.z - c.a.z) * f);
                let part = CutSeg { b: q, ..c };
                self.cut(&mut state, &[part]);
            }
        }
        state
    }

    /// Siatka materiału w chwili `t` do wyświetlenia (siatki niezmienionych kafelków z pamięci).
    pub fn mesh_at(&mut self, t: f64) -> SimMesh {
        let state = self.state_at(t);
        let (top, bottom) = (self.top as f32, self.bottom as f32);
        let mut out = SimMesh::default();
        for (k, solid) in state.iter().enumerate() {
            let cached = self.meshes[k].as_ref().filter(|(m, _)| Arc::ptr_eq(m, solid)).map(|(_, mesh)| mesh.clone());
            let mesh = match cached {
                Some(m) => m,
                None => {
                    let m = Arc::new(mesh_without_planes(solid, top, bottom, &self.inner[k]));
                    // pamiętamy tylko siatki stanu bieżącego (nie chwilowego, z częścią ruchu)
                    if self.cur.1.get(k).is_some_and(|c| Arc::ptr_eq(c, solid)) {
                        self.meshes[k] = Some((solid.clone(), m.clone()));
                    }
                    m
                }
            };
            let base = out.verts.len() as u32;
            out.verts.extend_from_slice(&mesh.verts);
            out.indices.extend(mesh.indices.iter().map(|i| i + base));
        }
        out
    }

    /// Liczba trójkątów we wszystkich klatkach kluczowych (wspólne kafelki liczone raz).
    pub fn keyframe_triangles(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        self.keys.iter().flat_map(|k| k.1.iter()).filter(|s| seen.insert(Arc::as_ptr(s))).map(|s| s.num_tri()).sum()
    }

    /// Objętość materiału w chwili `t` [mm³].
    pub fn volume_at(&mut self, t: f64) -> f64 {
        // każdy kafelek tylko we własnym obszarze — bez zakładki na sąsiadów
        let state = self.state_at(t);
        let (tl, bottom, top) = (&self.tiles, self.bottom, self.top);
        let core = |k: usize| {
            let (i, j) = (k % tl.nx, k / tl.nx);
            let lo = P3::new(tl.x0 + i as f64 * tl.size, tl.y0 + j as f64 * tl.size, bottom - 1.0);
            stock(lo, P3::new(lo.x + tl.size, lo.y + tl.size, top + 1.0))
        };
        state.iter().enumerate().map(|(k, s)| s.intersection(&core(k)).volume()).sum()
    }

    /// Liczba trójkątów bryły w chwili `t`.
    pub fn triangles_at(&mut self, t: f64) -> usize {
        self.state_at(t).iter().map(|s| s.num_tri()).sum()
    }
}

/// Tolerancja upraszczania siatki po operacjach boolowskich [mm] — scala
/// rozdrobnione trójkąty leżące w jednej płaszczyźnie (bez zmiany kształtu).
const SIMPLIFY_TOL: f64 = 1e-4;

/// Najwięcej trójkątów bryły — powyżej tego symulacja przechodzi na mapę wysokości.
pub const MAX_TRIANGLES: usize = 2_000_000;

/// Najwięcej trójkątów we wszystkich klatkach kluczowych razem (ok. 200 MB pamięci).
const KEY_TRIANGLES: usize = 600_000;

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

/// Jak `mesh_of`, bez trójkątów leżących na podanych płaszczyznach (oś 0 = x, 1 = y):
/// wewnętrzne boki kafelka, które na styku z sąsiadem przebijałyby przez powierzchnię
/// materiału jako cienkie przerywane linie.
fn mesh_without_planes(m: &Manifold, top: f32, bottom: f32, planes: &[(usize, f64)]) -> SimMesh {
    let full = mesh_of(m, top, bottom);
    if planes.is_empty() {
        return full;
    }
    let on_plane = |tri: &[SimVertex]| {
        planes.iter().any(|&(axis, v)| tri.iter().all(|p| (p.pos[axis] as f64 - v).abs() < 1e-3))
    };
    let mut out = SimMesh::default();
    for t in full.indices.chunks(3) {
        let tri = [full.verts[t[0] as usize], full.verts[t[1] as usize], full.verts[t[2] as usize]];
        if on_plane(&tri) {
            continue;
        }
        for v in tri {
            out.verts.push(v);
            out.indices.push((out.verts.len() - 1) as u32);
        }
    }
    out
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
        let removed = full - s.volume_at(sim.duration());
        let expect = 2.0 * (30.0 * 6.0 + std::f64::consts::PI * 9.0);
        // wielokąt 30-boczny zamiast okręgu — błąd < 1%
        assert!((removed - expect).abs() < 0.01 * expect, "{removed} vs {expect}");
        // w połowie przejazdu wycięta mniej więcej połowa rowka
        let mid = full - s.volume_at(sim.path[2].t * 0.5 + sim.path[1].t * 0.5);
        assert!(mid > 0.3 * expect && mid < 0.7 * expect, "{mid}");
    }
}
