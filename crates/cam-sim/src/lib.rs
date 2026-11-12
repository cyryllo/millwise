//! Symulacja usuwania materiału dla obróbki 3-osiowej: mapa wysokości
//! (Z-buffer) półfabrykatu, odciskanie kształtu narzędzia wzdłuż ścieżki,
//! odtwarzanie w czasie i wykrywanie kolizji ruchów szybkich z materiałem.

use cam_geom::{Aabb2, Vec2};
use cam_ops::{ToolKind, P3};

mod mesh;
pub mod solid;
use mesh::build_mesh;

/// Profil narzędzia: wysokość powierzchni tnącej nad czubkiem w odległości `d` od osi.
#[derive(Debug, Clone)]
pub struct ToolShape {
    pub radius: f64,
    kind: Profile,
}

#[derive(Debug, Clone, Copy)]
enum Profile {
    Flat,
    Ball,
    Bull { r: f64 },
    /// Stożek: tangens połówki kąta, promień płaskiego czubka.
    Cone { tan_half: f64, tip_r: f64 },
}

impl ToolShape {
    pub fn new(kind: &ToolKind, diameter: f64) -> Self {
        let radius = (diameter * 0.5).max(1e-3);
        let kind = match *kind {
            ToolKind::FlatEnd => Profile::Flat,
            ToolKind::BallEnd => Profile::Ball,
            ToolKind::BullNose { corner_radius } => Profile::Bull { r: corner_radius.clamp(0.0, radius) },
            ToolKind::VBit { angle, tip_diameter } | ToolKind::Engraver { angle, tip_diameter } => Profile::Cone {
                tan_half: (angle.clamp(1.0, 179.0).to_radians() * 0.5).tan(),
                tip_r: (tip_diameter * 0.5).clamp(0.0, radius),
            },
            ToolKind::Drill { point_angle } => {
                Profile::Cone { tan_half: (point_angle.clamp(1.0, 179.0).to_radians() * 0.5).tan(), tip_r: 0.0 }
            }
        };
        ToolShape { radius, kind }
    }

    pub fn flat(diameter: f64) -> Self {
        Self::new(&ToolKind::FlatEnd, diameter)
    }

    /// Wysokość dolnej powierzchni narzędzia nad czubkiem w odległości `d` od osi (None = poza narzędziem).
    pub fn profile(&self, d: f64) -> Option<f64> {
        if d > self.radius {
            return None;
        }
        Some(match self.kind {
            Profile::Flat => 0.0,
            Profile::Ball => self.radius - (self.radius * self.radius - d * d).max(0.0).sqrt(),
            Profile::Bull { r } => {
                let flat = self.radius - r;
                if d <= flat {
                    0.0
                } else {
                    let x = d - flat;
                    r - (r * r - x * x).max(0.0).sqrt()
                }
            }
            Profile::Cone { tan_half, tip_r } => ((d - tip_r).max(0.0)) / tan_half.max(1e-6),
        })
    }
}

/// Mapa wysokości półfabrykatu: dla każdej komórki — Z górnej powierzchni materiału.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub origin: Vec2,
    pub cell: f64,
    pub nx: usize,
    pub ny: usize,
    pub top: f32,
    pub bottom: f32,
    pub z: Vec<f32>,
}

impl Heightmap {
    /// `max_cells` — ograniczenie liczby komórek wzdłuż dłuższego boku.
    pub fn new(area: Aabb2, top: f64, bottom: f64, max_cells: usize) -> Self {
        let size = area.size();
        let cell = (size.x.max(size.y) / max_cells.max(8) as f64).max(0.01);
        let nx = ((size.x / cell).ceil() as usize).max(2);
        let ny = ((size.y / cell).ceil() as usize).max(2);
        Heightmap {
            origin: area.min,
            cell,
            nx,
            ny,
            top: top as f32,
            bottom: bottom as f32,
            z: vec![top as f32; nx * ny],
        }
    }

    pub fn reset(&mut self) {
        self.z.iter_mut().for_each(|z| *z = self.top);
    }

    pub fn idx(&self, i: usize, j: usize) -> usize {
        j * self.nx + i
    }

    /// Środek komórki (i, j).
    pub fn center(&self, i: usize, j: usize) -> Vec2 {
        Vec2::new(self.origin.x + (i as f64 + 0.5) * self.cell, self.origin.y + (j as f64 + 0.5) * self.cell)
    }

    pub fn height_at(&self, p: Vec2) -> Option<f32> {
        let i = ((p.x - self.origin.x) / self.cell).floor();
        let j = ((p.y - self.origin.y) / self.cell).floor();
        if i < 0.0 || j < 0.0 || i as usize >= self.nx || j as usize >= self.ny {
            return None;
        }
        Some(self.z[self.idx(i as usize, j as usize)])
    }

    /// Odciska narzędzie z czubkiem w `tip`. Zwraca największą usuniętą grubość.
    pub fn stamp(&mut self, tip: P3, tool: &ToolShape) -> f32 {
        self.sweep(tip, tip, tool)
    }

    /// Usuwa materiał wzdłuż odcinka a → b — wysokość w każdym węźle liczona
    /// dokładnie z geometrii (przecięcie koła narzędzia z odcinkiem).
    pub fn sweep(&mut self, a: P3, b: P3, tool: &ToolShape) -> f32 {
        if a.z.min(b.z) as f32 >= self.top {
            return 0.0;
        }
        let r = tool.radius;
        let (x0, x1) = (a.x.min(b.x) - r, a.x.max(b.x) + r);
        let (y0, y1) = (a.y.min(b.y) - r, a.y.max(b.y) + r);
        let i0 = ((x0 - self.origin.x) / self.cell - 0.5).floor().max(0.0) as usize;
        let j0 = ((y0 - self.origin.y) / self.cell - 0.5).floor().max(0.0) as usize;
        let i1 = (((x1 - self.origin.x) / self.cell - 0.5).ceil() as isize).min(self.nx as isize - 1);
        let j1 = (((y1 - self.origin.y) / self.cell - 0.5).ceil() as isize).min(self.ny as isize - 1);
        if i1 < 0 || j1 < 0 {
            return 0.0;
        }
        let mut removed = 0.0f32;
        for j in j0..=j1 as usize {
            for i in i0..=i1 as usize {
                let c = self.center(i, j);
                if let Some(h) = seg_height(c, a, b, tool) {
                    let k = j * self.nx + i;
                    let h = (h as f32).max(self.bottom);
                    if h < self.z[k] {
                        removed = removed.max(self.z[k] - h);
                        self.z[k] = h;
                    }
                }
            }
        }
        removed
    }
}

/// Najniższe położenie powierzchni narzędzia nad punktem `p`, gdy czubek
/// przesuwa się po odcinku a → b (None, gdy narzędzie nie obejmuje `p`).
pub fn seg_height(p: Vec2, a: P3, b: P3, tool: &ToolShape) -> Option<f64> {
    let r = tool.radius;
    let d = Vec2::new(b.x - a.x, b.y - a.y);
    let w = Vec2::new(a.x - p.x, a.y - p.y);
    let l2 = d.length_sq();
    // przedział t, w którym oś narzędzia jest w odległości ≤ r od p
    let (t0, t1) = if l2 < 1e-18 {
        if w.length_sq() > r * r {
            return None;
        }
        (0.0, 1.0)
    } else {
        let bq = w.dot(d);
        let c = w.length_sq() - r * r;
        let disc = bq * bq - l2 * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let t0 = ((-bq - sq) / l2).max(0.0);
        let t1 = ((-bq + sq) / l2).min(1.0);
        if t0 > t1 {
            return None;
        }
        (t0, t1)
    };
    let z = |t: f64| a.z + (b.z - a.z) * t;
    let f = |t: f64| {
        let q = Vec2::new(w.x + d.x * t, w.y + d.y * t);
        z(t) + tool.profile(q.length().min(r)).unwrap_or(0.0)
    };
    match tool.kind {
        Profile::Flat => Some(z(t0).min(z(t1))),
        Profile::Cone { tan_half, tip_r } => {
            // f(t) = z(t) + max(dist(t) − tip, 0) / tan — wypukła; minimum w punkcie
            // stacjonarnym części stożkowej albo na końcach przedziałów
            let k = 1.0 / tan_half.max(1e-9);
            let mut cand = vec![t0, t1];
            if l2 > 1e-18 {
                let len = l2.sqrt();
                let tp = -w.dot(d) / l2; // rzut p na prostą
                let e = (w + d * tp).length(); // odległość p od prostej
                let g = -(b.z - a.z) / (k * len);
                if g.abs() < 1.0 {
                    let u = g * e / (1.0 - g * g).sqrt();
                    cand.push((tp + u / len).clamp(t0, t1));
                }
                cand.push(tp.clamp(t0, t1));
                // granice płaskiego czubka (dist = tip)
                if tip_r > 0.0 && e < tip_r {
                    let h = (tip_r * tip_r - e * e).sqrt() / len;
                    cand.push((tp - h).clamp(t0, t1));
                    cand.push((tp + h).clamp(t0, t1));
                }
            }
            Some(cand.into_iter().map(f).fold(f64::INFINITY, f64::min))
        }
        Profile::Ball | Profile::Bull { .. } => {
            // funkcja wypukła — przeszukiwanie złotym podziałem
            let (mut lo, mut hi) = (t0, t1);
            let g = 0.618_033_988_749_894_9;
            let mut x1 = hi - g * (hi - lo);
            let mut x2 = lo + g * (hi - lo);
            let (mut f1, mut f2) = (f(x1), f(x2));
            for _ in 0..40 {
                if hi - lo < 1e-7 {
                    break;
                }
                if f1 < f2 {
                    hi = x2;
                    x2 = x1;
                    f2 = f1;
                    x1 = hi - g * (hi - lo);
                    f1 = f(x1);
                } else {
                    lo = x1;
                    x1 = x2;
                    f1 = f2;
                    x2 = lo + g * (hi - lo);
                    f2 = f(x2);
                }
            }
            Some(f(t0).min(f(t1)).min(f1).min(f2))
        }
    }
}

/// Wierzchołek siatki materiału. `kind`: 0 = powierzchnia, 1 = ściana wycięcia, 2 = bok półfabrykatu.
#[derive(Debug, Clone, Copy)]
pub struct SimVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub kind: u8,
}

#[derive(Debug, Default)]
pub struct SimMesh {
    pub verts: Vec<SimVertex>,
    pub indices: Vec<u32>,
}

/// Punkt ścieżki symulacji.
#[derive(Debug, Clone, Copy)]
pub struct SimPoint {
    pub p: P3,
    pub rapid: bool,
    /// Indeks segmentu programu (operacji) — wybiera narzędzie.
    pub op: usize,
    /// Czas od początku programu [s] w chwili osiągnięcia punktu.
    pub t: f64,
}

/// Jedna operacja do symulacji: narzędzie i spłaszczona ścieżka z posuwami.
pub struct SimOp<'a> {
    pub tool: ToolShape,
    /// (punkt, ruch szybki, posuw [mm/min]).
    pub points: &'a [(P3, bool, f64)],
}

/// Zapamiętany stan symulacji (do szybkiego przewijania).
#[derive(Debug, Clone)]
pub struct Keyframe {
    z: Vec<f32>,
    cursor: usize,
    last: Option<P3>,
    pub time: f64,
    segs_len: usize,
    collisions_len: usize,
}

impl Keyframe {
    pub fn bytes(&self) -> usize {
        self.z.len() * 4
    }
}

/// Ruch szybki, który usunąłby materiał.
#[derive(Debug, Clone, Copy)]
pub struct Collision {
    pub at: P3,
    pub op: usize,
    pub depth: f32,
}

pub struct Simulator {
    pub map: Heightmap,
    pub path: Vec<SimPoint>,
    pub tools: Vec<ToolShape>,
    /// Czas rozpoczęcia każdej operacji [s].
    pub op_start: Vec<f64>,
    /// Indeks ostatniego w pełni przetworzonego punktu.
    cursor: usize,
    /// Pozycja, do której materiał jest już usunięty.
    last: Option<P3>,
    pub time: f64,
    pub collisions: Vec<Collision>,
    /// Zwiększany przy każdej zmianie mapy (do odświeżania siatki).
    pub revision: u64,
    /// Przetworzone odcinki (do dokładnego wyznaczania ścian).
    segs: Vec<(P3, P3, u16)>,
    buckets: Vec<Vec<u32>>,
    bucket: f64,
    bnx: usize,
    bny: usize,
}

impl Simulator {
    /// `rapid_rate` — prędkość ruchów szybkich [mm/min].
    pub fn new(map: Heightmap, ops: &[SimOp], rapid_rate: f64) -> Self {
        let mut path = Vec::new();
        let mut tools = Vec::new();
        let mut op_start = Vec::new();
        let mut t = 0.0;
        let mut prev: Option<P3> = None;
        for (oi, op) in ops.iter().enumerate() {
            tools.push(op.tool.clone());
            op_start.push(t);
            for &(p, rapid, feed) in op.points {
                if let Some(a) = prev {
                    let rate = if rapid { rapid_rate } else { feed };
                    t += a.distance(p) / rate.max(1.0) * 60.0;
                }
                path.push(SimPoint { p, rapid, op: oi, t });
                prev = Some(p);
            }
        }
        let max_r = tools.iter().map(|t: &ToolShape| t.radius).fold(0.5, f64::max);
        let bucket = (max_r * 2.0).max(2.0);
        let size = Vec2::new(map.nx as f64 * map.cell, map.ny as f64 * map.cell);
        let bnx = ((size.x / bucket).ceil() as usize).max(1);
        let bny = ((size.y / bucket).ceil() as usize).max(1);
        Simulator {
            map,
            path,
            tools,
            op_start,
            cursor: 0,
            last: None,
            time: 0.0,
            collisions: Vec::new(),
            revision: 0,
            segs: Vec::new(),
            buckets: vec![Vec::new(); bnx * bny],
            bucket,
            bnx,
            bny,
        }
    }

    pub fn duration(&self) -> f64 {
        self.path.last().map_or(0.0, |p| p.t)
    }

    pub fn finished(&self) -> bool {
        self.time >= self.duration()
    }

    /// Bieżąca pozycja narzędzia i indeks operacji.
    pub fn position(&self) -> Option<(P3, usize)> {
        if self.path.is_empty() {
            return None;
        }
        Some((self.last.unwrap_or(self.path[0].p), self.path[self.cursor.min(self.path.len() - 1)].op))
    }

    pub fn reset(&mut self) {
        self.map.reset();
        self.segs.clear();
        self.buckets.iter_mut().for_each(|b| b.clear());
        self.cursor = 0;
        self.last = None;
        self.time = 0.0;
        self.collisions.clear();
        self.revision += 1;
    }

    /// Przewija do czasu `t` (wstecz = od początku).
    pub fn seek(&mut self, t: f64) {
        let t = t.clamp(0.0, self.duration());
        if t < self.time {
            self.reset();
        }
        self.advance_to(t);
    }

    pub fn advance_to(&mut self, t: f64) {
        if self.path.is_empty() {
            return;
        }
        let t = t.clamp(0.0, self.duration());
        if self.last.is_none() {
            self.last = Some(self.path[0].p);
        }
        let mut changed = false;
        while self.cursor + 1 < self.path.len() && self.path[self.cursor + 1].t <= t {
            let next = self.path[self.cursor + 1];
            changed |= self.segment(self.last.unwrap(), next.p, next.rapid, next.op);
            self.last = Some(next.p);
            self.cursor += 1;
        }
        // część bieżącego odcinka
        if self.cursor + 1 < self.path.len() {
            let a = self.path[self.cursor];
            let b = self.path[self.cursor + 1];
            let span = b.t - a.t;
            if span > 0.0 && t > a.t {
                let f = ((t - a.t) / span).clamp(0.0, 1.0);
                let q = P3::new(a.p.x + (b.p.x - a.p.x) * f, a.p.y + (b.p.y - a.p.y) * f, a.p.z + (b.p.z - a.p.z) * f);
                changed |= self.segment(self.last.unwrap(), q, b.rapid, b.op);
                self.last = Some(q);
            }
        }
        self.time = t;
        if changed {
            self.revision += 1;
        }
    }

    pub fn snapshot(&self) -> Keyframe {
        Keyframe {
            z: self.map.z.clone(),
            cursor: self.cursor,
            last: self.last,
            time: self.time,
            segs_len: self.segs.len(),
            collisions_len: self.collisions.len(),
        }
    }

    /// Przywraca stan z klatki kluczowej zapisanej wcześniej w tej samej symulacji.
    pub fn restore(&mut self, k: &Keyframe) {
        self.map.z.copy_from_slice(&k.z);
        self.cursor = k.cursor;
        self.last = k.last;
        self.time = k.time;
        self.segs.truncate(k.segs_len);
        let len = k.segs_len as u32;
        for b in &mut self.buckets {
            while b.last().is_some_and(|&id| id >= len) {
                b.pop();
            }
        }
        self.collisions.truncate(k.collisions_len);
        self.revision += 1;
    }

    /// Przewija do czasu `t` korzystając z klatek kluczowych (posortowanych po czasie).
    pub fn seek_with(&mut self, t: f64, keys: &[Keyframe]) {
        let t = t.clamp(0.0, self.duration());
        let k = keys.partition_point(|k| k.time <= t);
        let best = k.checked_sub(1).map(|i| &keys[i]);
        match best {
            // cofanie albo duży skok do przodu — start od najbliższej wcześniejszej klatki
            Some(key) if t < self.time || key.time > self.time => self.restore(key),
            None if t < self.time => self.reset(),
            _ => {}
        }
        self.advance_to(t);
    }

    /// Dokładna wysokość materiału w dowolnym punkcie (nie tylko w węzłach siatki).
    pub fn height_exact(&self, p: Vec2) -> f64 {
        let m = &self.map;
        let mut h = m.top as f64;
        let bx = ((p.x - m.origin.x) / self.bucket).floor();
        let by = ((p.y - m.origin.y) / self.bucket).floor();
        if bx < 0.0 || by < 0.0 || bx as usize >= self.bnx || by as usize >= self.bny {
            return h;
        }
        for &id in &self.buckets[by as usize * self.bnx + bx as usize] {
            let (a, b, op) = self.segs[id as usize];
            if let Some(z) = seg_height(p, a, b, &self.tools[op as usize]) {
                h = h.min(z);
            }
        }
        h.max(m.bottom as f64)
    }

    /// Siatka materiału z dokładnymi ścianami wycięć.
    pub fn mesh(&self) -> SimMesh {
        build_mesh(self)
    }

    fn segment(&mut self, a: P3, b: P3, rapid: bool, op: usize) -> bool {
        if a.distance(b) < 1e-9 {
            return false;
        }
        let tool = &self.tools[op];
        let removed = self.map.sweep(a, b, tool);
        // każdy odcinek poniżej góry materiału może ciąć między węzłami — zapamiętaj
        // go w siatce kubełków (do dokładnych ścian)
        if (a.z.min(b.z) as f32) < self.map.top {
            let r = tool.radius;
            let o = self.map.origin;
            let bi = |x: f64, n: usize| (((x / self.bucket).floor()).max(0.0) as usize).min(n - 1);
            let (i0, i1) = (bi(a.x.min(b.x) - r - o.x, self.bnx), bi(a.x.max(b.x) + r - o.x, self.bnx));
            let (j0, j1) = (bi(a.y.min(b.y) - r - o.y, self.bny), bi(a.y.max(b.y) + r - o.y, self.bny));
            let id = self.segs.len() as u32;
            self.segs.push((a, b, op as u16));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    self.buckets[j * self.bnx + i].push(id);
                }
            }
        }
        if rapid && removed > 0.05 {
            if self.collisions.last().is_none_or(|c| c.at.distance(b) > tool.radius * 2.0) {
                self.collisions.push(Collision { at: b, op, depth: removed });
            }
        }
        removed > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Aabb2 {
        Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(40.0, 20.0) }
    }

    #[test]
    fn exact_wall_position_on_circle() {
        // kieszeń okrągła: narzędzie Ø6 po okręgu R=8 → krawędź na R=11 niezależnie od siatki
        let area = Aabb2 { min: Vec2::new(-20.0, -20.0), max: Vec2::new(20.0, 20.0) };
        let n = 200;
        let pts: Vec<(P3, bool, f64)> = (0..=n)
            .map(|k| {
                let a = k as f64 / n as f64 * std::f64::consts::TAU;
                (P3::new(8.0 * a.cos(), 8.0 * a.sin(), -2.0), false, 1000.0)
            })
            .collect();
        let op = SimOp { tool: ToolShape::flat(6.0), points: &pts };
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -10.0, 60), &[op], 3000.0);
        sim.advance_to(sim.duration());
        let mesh = sim.mesh();
        let walls: Vec<_> = mesh.verts.iter().filter(|v| v.kind == 1).collect();
        assert!(!walls.is_empty());
        for v in walls {
            let r = (v.pos[0].powi(2) + v.pos[1].powi(2)).sqrt();
            // ściany zewnętrzne na R=11, wewnętrzne (wyspa) na R=5
            let err = (r - 11.0).abs().min((r - 5.0).abs());
            assert!(err < 0.003, "ściana w r={r} (komórka {:.2} mm)", sim.map.cell);
        }
        // spód zamknięty: pole ścian dolnych ≈ pole półfabrykatu (nie ma przecięć na wylot)
        let mut area_bottom = 0.0f64;
        for t in mesh.indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.verts[i as usize]);
            if a.kind == 2 && a.normal[2] < -0.5 && b.normal[2] < -0.5 && c.normal[2] < -0.5 {
                let (ax, ay) = (a.pos[0] as f64, a.pos[1] as f64);
                let (bx, by) = (b.pos[0] as f64, b.pos[1] as f64);
                let (cx, cy) = (c.pos[0] as f64, c.pos[1] as f64);
                area_bottom += ((bx - ax) * (cy - ay) - (cx - ax) * (by - ay)).abs() * 0.5;
            }
        }
        let span = (sim.map.nx - 1) as f64 * sim.map.cell;
        assert!((area_bottom - span * span).abs() < 1e-3 * span * span, "spód {area_bottom} vs {}", span * span);
        assert!((sim.height_exact(Vec2::new(10.5, 0.0)) + 2.0).abs() < 1e-9);
        assert!(sim.height_exact(Vec2::new(11.5, 0.0)).abs() < 1e-9);
    }

    #[test]
    fn through_hole_leaves_opening_in_bottom() {
        let area = Aabb2 { min: Vec2::new(-10.0, -10.0), max: Vec2::new(10.0, 10.0) };
        let pts = [(P3::new(0.0, 0.0, 2.0), true, 0.0), (P3::new(0.0, 0.0, -12.0), false, 300.0)];
        let op = SimOp { tool: ToolShape::flat(8.0), points: &pts };
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -10.0, 80), &[op], 3000.0);
        sim.advance_to(sim.duration());
        let mesh = sim.mesh();
        let mut area_bottom = 0.0f64;
        for t in mesh.indices.chunks(3) {
            let v = [t[0], t[1], t[2]].map(|i| mesh.verts[i as usize]);
            if v.iter().all(|v| v.kind == 2 && v.normal[2] < -0.5) {
                let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
                area_bottom += (((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) as f64).abs() * 0.5;
            }
        }
        let span = (sim.map.nx - 1) as f64 * sim.map.cell;
        let hole = std::f64::consts::PI * 16.0;
        assert!((span * span - area_bottom - hole).abs() < 0.02 * hole, "otwór w spodzie: {}", span * span - area_bottom);
    }

    #[test]
    fn v_bit_slopes_have_no_vertical_walls() {
        // V 90° po okręgu R=8 na głębokość 3 mm: skośne ściany, bez pionowych uskoków
        let area = Aabb2 { min: Vec2::new(-15.0, -15.0), max: Vec2::new(15.0, 15.0) };
        let n = 200;
        let pts: Vec<(P3, bool, f64)> = (0..=n)
            .map(|k| {
                let a = k as f64 / n as f64 * std::f64::consts::TAU;
                (P3::new(8.0 * a.cos(), 8.0 * a.sin(), -3.0), false, 1000.0)
            })
            .collect();
        let tool = ToolShape::new(&ToolKind::VBit { angle: 90.0, tip_diameter: 0.0 }, 8.0);
        let op = SimOp { tool, points: &pts };
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -10.0, 100), &[op], 3000.0);
        sim.advance_to(sim.duration());
        let mesh = sim.mesh();
        assert_eq!(mesh.verts.iter().filter(|v| v.kind == 1).count(), 0, "skos nie może mieć pionowych ścian");
        // każdy wierzchołek powierzchni leży dokładnie na stożku: z = -3 + |r - 8| (lub 0 poza)
        for v in mesh.verts.iter().filter(|v| v.kind == 0) {
            let r = ((v.pos[0] as f64).powi(2) + (v.pos[1] as f64).powi(2)).sqrt();
            let expect = (-3.0 + (r - 8.0).abs()).min(0.0);
            assert!((v.pos[2] as f64 - expect).abs() < 0.03, "r={r} z={} oczekiwane {expect}", v.pos[2]);
        }
    }

    #[test]
    fn seg_height_matches_brute_force() {
        // prosty generator pseudolosowy (bez zależności)
        let mut seed = 12345u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / (1u64 << 31) as f64
        };
        let tools = [
            ToolShape::new(&ToolKind::VBit { angle: 90.0, tip_diameter: 0.0 }, 6.0),
            ToolShape::new(&ToolKind::Engraver { angle: 30.0, tip_diameter: 0.4 }, 3.0),
            ToolShape::new(&ToolKind::BallEnd, 6.0),
            ToolShape::new(&ToolKind::BullNose { corner_radius: 1.0 }, 6.0),
            ToolShape::new(&ToolKind::Drill { point_angle: 118.0 }, 5.0),
        ];
        for tool in &tools {
            for _ in 0..300 {
                let a = P3::new(rnd() * 10.0, rnd() * 10.0, -rnd() * 4.0);
                let b = P3::new(rnd() * 10.0, rnd() * 10.0, -rnd() * 4.0);
                let p = Vec2::new(rnd() * 12.0 - 1.0, rnd() * 12.0 - 1.0);
                let fast = seg_height(p, a, b, tool);
                // brute force: gęste próbkowanie odcinka
                let mut best: Option<f64> = None;
                for k in 0..=20000 {
                    let t = k as f64 / 20000.0;
                    let c = Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                    if let Some(off) = tool.profile(c.distance(p)) {
                        let v = a.z + (b.z - a.z) * t + off;
                        best = Some(best.map_or(v, |x: f64| x.min(v)));
                    }
                }
                match (fast, best) {
                    (Some(f), Some(bf)) => assert!(f <= bf + 1e-6 && bf - f < 5e-3, "{tool:?}: {f} vs {bf}"),
                    (None, None) => {}
                    (f, bf) => {
                        // różnice tylko na samej granicy zasięgu narzędzia
                        assert!(f.is_some() && bf.is_none(), "{tool:?}: {f:?} vs {bf:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn keyframes_give_same_result_as_full_run() {
        let area = Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(40.0, 20.0) };
        let pts: Vec<(P3, bool, f64)> = (0..=40)
            .map(|k| (P3::new(2.0 + k as f64 * 0.9, 10.0 + 6.0 * (k as f64 * 0.4).sin(), -1.0 - k as f64 * 0.05), false, 600.0))
            .collect();
        let mk = || {
            let op = SimOp { tool: ToolShape::flat(3.0), points: &pts };
            Simulator::new(Heightmap::new(area, 0.0, -10.0, 120), &[op], 3000.0)
        };
        let mut a = mk();
        let dur = a.duration();
        let mut keys = Vec::new();
        for k in 0..=10 {
            a.advance_to(dur * k as f64 / 10.0);
            keys.push(a.snapshot());
        }
        // przewijanie tam i z powrotem z klatkami == liczenie od zera
        let mut b = mk();
        for t in [0.9, 0.2, 0.55, 0.1, 0.77] {
            b.seek_with(dur * t, &keys);
            let mut c = mk();
            c.advance_to(dur * t);
            assert_eq!(b.map.z, c.map.z, "t={t}");
            assert!((b.height_exact(Vec2::new(20.0, 10.0)) - c.height_exact(Vec2::new(20.0, 10.0))).abs() < 1e-12);
        }
    }

    #[test]
    fn narrow_ledges_and_wall_normals() {
        // trzy poziomy blisko siebie: rowek Ø6 na -2 i węższy na -4 przesunięty o 0,3 mm
        let area = Aabb2 { min: Vec2::new(0.0, 0.0), max: Vec2::new(30.0, 20.0) };
        let p1 = [(P3::new(5.0, 10.0, -2.0), false, 600.0), (P3::new(25.0, 13.0, -2.0), false, 600.0)];
        let p2 = [
            (P3::new(25.0, 13.0, 5.0), true, 0.0),
            (P3::new(5.0, 9.7, 5.0), true, 0.0),
            (P3::new(5.0, 9.7, -4.0), false, 600.0),
            (P3::new(25.0, 12.7, -4.0), false, 600.0),
        ];
        let ops = [SimOp { tool: ToolShape::flat(6.0), points: &p1 }, SimOp { tool: ToolShape::flat(5.0), points: &p2 }];
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -10.0, 60), &ops, 3000.0);
        sim.advance_to(sim.duration());
        let mesh = sim.mesh();
        let walls: Vec<_> = mesh.verts.iter().filter(|v| v.kind == 1).collect();
        assert!(!walls.is_empty());
        for v in &walls {
            let p = Vec2::new(v.pos[0] as f64, v.pos[1] as f64);
            let n = Vec2::new(v.normal[0] as f64, v.normal[1] as f64);
            // ściana stoi na granicy: po stronie normalnej jest niżej niż po przeciwnej
            let lo = sim.height_exact(p + n * 0.05);
            let hi = sim.height_exact(p - n * 0.05);
            assert!(lo < hi - 0.5, "normalna ściany nie wskazuje niższej strony: {lo} vs {hi} w {p:?}");
        }
        // żaden trójkąt powierzchni nie przecina uskoku: wierzchołki trójkąta na jednym poziomie
        let mut bad = 0;
        for t in mesh.indices.chunks(3) {
            let v = [t[0], t[1], t[2]].map(|i| mesh.verts[i as usize]);
            if v.iter().all(|v| v.kind == 0) {
                let zs = v.map(|v| v.pos[2]);
                let span = zs.iter().cloned().fold(f32::MIN, f32::max) - zs.iter().cloned().fold(f32::MAX, f32::min);
                if span > 0.5 {
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0, "trójkąty powierzchni przechodzące przez uskok");
    }

    #[test]
    fn profiles() {
        let ball = ToolShape::new(&ToolKind::BallEnd, 6.0);
        assert!((ball.profile(3.0).unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(ball.profile(0.0), Some(0.0));
        let v = ToolShape::new(&ToolKind::VBit { angle: 90.0, tip_diameter: 0.0 }, 6.0);
        assert!((v.profile(2.0).unwrap() - 2.0).abs() < 1e-9);
        assert!(v.profile(3.1).is_none());
    }

    #[test]
    fn flat_slot_depth_and_width() {
        let mut m = Heightmap::new(area(), 0.0, -10.0, 400);
        let tool = ToolShape::flat(6.0);
        m.sweep(P3::new(5.0, 10.0, -2.0), P3::new(35.0, 10.0, -2.0), &tool);
        assert_eq!(m.height_at(Vec2::new(20.0, 10.0)), Some(-2.0));
        assert_eq!(m.height_at(Vec2::new(20.0, 12.8)), Some(-2.0));
        assert_eq!(m.height_at(Vec2::new(20.0, 13.3)), Some(0.0));
        assert_eq!(m.height_at(Vec2::new(38.0, 10.0)), Some(0.0));
    }

    #[test]
    fn cut_through_clamps_to_bottom() {
        let mut m = Heightmap::new(area(), 0.0, -5.0, 100);
        m.stamp(P3::new(20.0, 10.0, -8.0), &ToolShape::flat(4.0));
        assert_eq!(m.height_at(Vec2::new(20.0, 10.0)), Some(-5.0));
    }

    #[test]
    fn playback_and_rapid_collision() {
        let pts = [
            (P3::new(5.0, 10.0, 5.0), true, 0.0),
            (P3::new(5.0, 10.0, -1.0), false, 600.0),
            (P3::new(35.0, 10.0, -1.0), false, 600.0),
            // ruch szybki przez materiał — kolizja
            (P3::new(35.0, 2.0, -1.0), true, 0.0),
        ];
        let op = SimOp { tool: ToolShape::flat(3.0), points: &pts };
        let mut sim = Simulator::new(Heightmap::new(area(), 0.0, -10.0, 200), &[op], 3000.0);
        // zejście 6 mm @600 = 0.6 s, przejazd 30 mm @600 = 3 s, ruch szybki 8 mm @3000 = 0.16 s
        assert!((sim.path[2].t - 3.6).abs() < 1e-9);
        assert!((sim.duration() - 3.76).abs() < 1e-9);
        sim.advance_to(2.1);
        assert_eq!(sim.map.height_at(Vec2::new(10.0, 10.0)), Some(-1.0));
        assert_eq!(sim.map.height_at(Vec2::new(30.0, 10.0)), Some(0.0), "jeszcze nie dojechał");
        sim.advance_to(sim.duration());
        assert_eq!(sim.map.height_at(Vec2::new(30.0, 10.0)), Some(-1.0));
        assert_eq!(sim.collisions.len(), 1);
        // przewinięcie wstecz odtwarza stan od nowa
        sim.seek(1.0);
        assert_eq!(sim.map.height_at(Vec2::new(30.0, 10.0)), Some(0.0));
        assert!(sim.collisions.is_empty());
    }
}
