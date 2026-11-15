//! Model 3D dla obróbki: mapa wysokości z góry (najwyższy punkt modelu w
//! każdym oczku), obrysy obszarów powyżej zadanej wysokości (sylwetka,
//! przekroje, wyspy obróbki zgrubnej) i opuszczanie freza na model.

use crate::tool::{Tool, ToolKind};
use cam_geom::{Aabb2, Contour, Vec2, Vertex};

/// Mapa wysokości modelu: oczko `cell`, środek oczka (i, j) w `min + (i + ½, j + ½)·cell`.
#[derive(Debug, Clone)]
pub struct ModelHeights {
    pub min: Vec2,
    pub cell: f64,
    pub nx: usize,
    pub ny: usize,
    /// Wysokość w oczku; `NEG_INFINITY` = poza modelem.
    pub z: Vec<f32>,
    pub z_min: f64,
    pub z_max: f64,
}

impl ModelHeights {
    /// Rasteryzacja trójkątów: w każdym oczku najwyższy punkt powierzchni.
    pub fn from_triangles(tris: &[[[f32; 3]; 3]], cell: f64) -> Option<Self> {
        let mut bb = Aabb2::EMPTY;
        let (mut z_min, mut z_max) = (f64::MAX, f64::MIN);
        for t in tris {
            for p in t {
                bb.add_point(Vec2::new(p[0] as f64, p[1] as f64));
                z_min = z_min.min(p[2] as f64);
                z_max = z_max.max(p[2] as f64);
            }
        }
        if bb.is_empty() {
            return None;
        }
        let cell = cell.max(0.01);
        // siatka przesunięta o pół oczka: brzegi modelu wypadają na środkach oczek
        let nx = (bb.size().x / cell).ceil() as usize + 1;
        let ny = (bb.size().y / cell).ceil() as usize + 1;
        let mut z = vec![f32::NEG_INFINITY; nx * ny];
        let min = bb.min - Vec2::new(cell * 0.5, cell * 0.5);
        for t in tris {
            let p: Vec<[f64; 3]> = t.iter().map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect();
            let (x0, x1) = (p.iter().map(|v| v[0]).fold(f64::MAX, f64::min), p.iter().map(|v| v[0]).fold(f64::MIN, f64::max));
            let (y0, y1) = (p.iter().map(|v| v[1]).fold(f64::MAX, f64::min), p.iter().map(|v| v[1]).fold(f64::MIN, f64::max));
            let i0 = (((x0 - min.x) / cell - 0.5).ceil().max(0.0)) as usize;
            let i1 = ((((x1 - min.x) / cell - 0.5).floor()) as isize).min(nx as isize - 1);
            let j0 = (((y0 - min.y) / cell - 0.5).ceil().max(0.0)) as usize;
            let j1 = ((((y1 - min.y) / cell - 0.5).floor()) as isize).min(ny as isize - 1);
            // krawędzie trójkąta zawsze trafiają do oczka, w którym leżą (ściany pionowe, cienkie trójkąty)
            for k in 0..3 {
                let (a, b) = (p[k], p[(k + 1) % 3]);
                let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                let n = (len / (cell * 0.5)).ceil().max(1.0) as usize;
                for s in 0..=n {
                    let f = s as f64 / n as f64;
                    let (x, y, zz) = (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f);
                    let i = (((x - min.x) / cell).floor().max(0.0) as usize).min(nx - 1);
                    let j = (((y - min.y) / cell).floor().max(0.0) as usize).min(ny - 1);
                    let c = &mut z[j * nx + i];
                    *c = c.max(zz as f32);
                }
            }
            if i1 < i0 as isize || j1 < j0 as isize {
                continue;
            }
            let d = (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
            if d.abs() < 1e-12 {
                continue;
            }
            for j in j0..=j1 as usize {
                let y = min.y + (j as f64 + 0.5) * cell;
                for i in i0..=i1 as usize {
                    let x = min.x + (i as f64 + 0.5) * cell;
                    let l1 = ((p[1][1] - p[2][1]) * (x - p[2][0]) + (p[2][0] - p[1][0]) * (y - p[2][1])) / d;
                    let l2 = ((p[2][1] - p[0][1]) * (x - p[2][0]) + (p[0][0] - p[2][0]) * (y - p[2][1])) / d;
                    let l3 = 1.0 - l1 - l2;
                    if l1 < -1e-9 || l2 < -1e-9 || l3 < -1e-9 {
                        continue;
                    }
                    let zz = (l1 * p[0][2] + l2 * p[1][2] + l3 * p[2][2]) as f32;
                    let c = &mut z[j * nx + i];
                    *c = c.max(zz);
                }
            }
        }
        Some(ModelHeights { min, cell, nx, ny, z, z_min, z_max })
    }

    /// Najniższy punkt powierzchni widocznej z góry (np. wierzch płyty pod płaskorzeźbą).
    pub fn lowest_top(&self) -> Option<f64> {
        self.z.iter().filter(|z| z.is_finite()).map(|&z| z as f64).reduce(f64::min)
    }

    pub fn bounds(&self) -> Aabb2 {
        Aabb2 { min: self.min, max: self.min + Vec2::new(self.nx as f64 * self.cell, self.ny as f64 * self.cell) }
    }

    fn at(&self, i: isize, j: isize) -> f32 {
        if i < 0 || j < 0 || i >= self.nx as isize || j >= self.ny as isize {
            f32::NEG_INFINITY
        } else {
            self.z[j as usize * self.nx + i as usize]
        }
    }

    /// Wysokość modelu w punkcie (oczko, w którym leży); `None` poza modelem.
    pub fn height_at(&self, p: Vec2) -> Option<f64> {
        let i = ((p.x - self.min.x) / self.cell).floor() as isize;
        let j = ((p.y - self.min.y) / self.cell).floor() as isize;
        let z = self.at(i, j);
        z.is_finite().then_some(z as f64)
    }

    /// Obrysy obszarów, w których model jest wyżej niż `level` (z góry):
    /// zamknięte kontury, zewnętrzne CCW, otwory CW. `level` poniżej modelu = sylwetka.
    pub fn outline_above(&self, level: f64) -> Vec<Contour> {
        self.outline_where(|_, z| z as f64 > level)
    }

    /// Obrysy obszarów oczek spełniających `f(indeks oczka, wysokość)`
    /// (zewnętrzne CCW, otwory CW).
    pub fn outline_where(&self, f: impl Fn(usize, f32) -> bool) -> Vec<Contour> {
        self.outline_where_tol(f, self.cell * 0.5)
    }

    /// Jak `outline_where`, z tolerancją upraszczania `tol`.
    pub fn outline_where_tol(&self, f: impl Fn(usize, f32) -> bool, tol: f64) -> Vec<Contour> {
        let inside = |i: isize, j: isize| {
            if i < 0 || j < 0 || i >= self.nx as isize || j >= self.ny as isize {
                return false;
            }
            let k = j as usize * self.nx + i as usize;
            f(k, self.z[k])
        };
        // marching squares na środkach oczek, z ramką „na zewnątrz”
        // krawędzie między sąsiednimi środkami (punkty na połowie odległości)
        let cell = self.cell;
        let pt = |i2: isize, j2: isize| {
            // współrzędne w połówkach oczka względem środka oczka (0,0)
            Vec2::new(self.min.x + (i2 as f64 * 0.5 + 0.5) * cell, self.min.y + (j2 as f64 * 0.5 + 0.5) * cell)
        };
        // odcinki: kierunek tak, by wnętrze było po lewej
        let mut segs: Vec<((isize, isize), (isize, isize))> = Vec::new();
        for j in -1..self.ny as isize {
            for i in -1..self.nx as isize {
                // narożniki kwadratu: a=(i,j) b=(i+1,j) c=(i+1,j+1) d=(i,j+1)
                let (a, b, c, d) = (inside(i, j), inside(i + 1, j), inside(i + 1, j + 1), inside(i, j + 1));
                let code = (a as u8) | (b as u8) << 1 | (c as u8) << 2 | (d as u8) << 3;
                // punkty środków krawędzi kwadratu (w połówkach)
                let e_bottom = (2 * i + 1, 2 * j);
                let e_right = (2 * i + 2, 2 * j + 1);
                let e_top = (2 * i + 1, 2 * j + 2);
                let e_left = (2 * i, 2 * j + 1);
                // (tablica opisuje kierunek z wnętrzem po prawej — odwracamy)
                let mut add = |p: (isize, isize), q: (isize, isize)| segs.push((q, p));
                match code {
                    0 | 15 => {}
                    1 => add(e_left, e_bottom),
                    2 => add(e_bottom, e_right),
                    3 => add(e_left, e_right),
                    4 => add(e_right, e_top),
                    5 => {
                        add(e_left, e_top);
                        add(e_right, e_bottom);
                    }
                    6 => add(e_bottom, e_top),
                    7 => add(e_left, e_top),
                    8 => add(e_top, e_left),
                    9 => add(e_top, e_bottom),
                    10 => {
                        add(e_bottom, e_left);
                        add(e_top, e_right);
                    }
                    11 => add(e_top, e_right),
                    12 => add(e_right, e_left),
                    13 => add(e_right, e_bottom),
                    14 => add(e_bottom, e_left),
                    _ => {}
                }
            }
        }
        // łańcuchy: każdy punkt startowy jest początkiem dokładnie jednego odcinka
        let mut next: std::collections::HashMap<(isize, isize), (isize, isize)> = std::collections::HashMap::with_capacity(segs.len());
        for (p, q) in &segs {
            next.insert(*p, *q);
        }
        let mut out = Vec::new();
        while let Some((&start, _)) = next.iter().next() {
            let mut pts = vec![pt(start.0, start.1)];
            let mut cur = start;
            let mut guard = 0;
            while let Some(q) = next.remove(&cur) {
                guard += 1;
                if q == start || guard > 10_000_000 {
                    break;
                }
                pts.push(pt(q.0, q.1));
                cur = q;
            }
            if pts.len() < 3 {
                continue;
            }
            let pts = simplify_closed(&pts, tol);
            if pts.len() < 3 {
                continue;
            }
            let mut c = Contour::new(true);
            c.verts = pts.into_iter().map(|p| Vertex::new(p.x, p.y, 0.0)).collect();
            out.push(c);
        }
        out
    }

    /// Oczka, których środki leżą wewnątrz zamkniętych konturów (parzysto-nieparzyście,
    /// więc otwory w konturach są poza).
    pub fn inside_mask(&self, contours: &[Contour]) -> Vec<bool> {
        let polys: Vec<Vec<Vec2>> = contours.iter().filter(|c| c.closed).map(|c| c.flatten(self.cell * 0.25)).collect();
        let mut m = vec![false; self.nx * self.ny];
        let mut xs = Vec::new();
        for j in 0..self.ny {
            let y = self.min.y + (j as f64 + 0.5) * self.cell;
            xs.clear();
            for poly in &polys {
                let n = poly.len();
                for k in 0..n {
                    let (a, b) = (poly[k], poly[(k + 1) % n]);
                    if (a.y > y) != (b.y > y) {
                        xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                    }
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks_exact(2) {
                let i0 = ((pair[0] - self.min.x) / self.cell - 0.5).ceil().max(0.0) as usize;
                let i1 = ((pair[1] - self.min.x) / self.cell - 0.5).floor();
                if i1 < 0.0 {
                    continue;
                }
                for i in i0..=(i1 as usize).min(self.nx - 1) {
                    m[j * self.nx + i] = true;
                }
            }
        }
        m
    }

    /// Ta sama mapa na większej siatce (w tym samym rastrze), obejmującej też `b`.
    pub fn padded(&self, b: Aabb2) -> ModelHeights {
        let c = self.cell;
        let lo_i = ((b.min.x - self.min.x) / c).floor().min(0.0) as isize;
        let lo_j = ((b.min.y - self.min.y) / c).floor().min(0.0) as isize;
        let hi_i = ((b.max.x - self.min.x) / c).ceil().max(self.nx as f64) as isize;
        let hi_j = ((b.max.y - self.min.y) / c).ceil().max(self.ny as f64) as isize;
        let (nx, ny) = ((hi_i - lo_i) as usize, (hi_j - lo_j) as usize);
        let mut z = vec![f32::NEG_INFINITY; nx * ny];
        for j in 0..self.ny {
            let dst = (j as isize - lo_j) as usize * nx + (-lo_i) as usize;
            z[dst..dst + self.nx].copy_from_slice(&self.z[j * self.nx..(j + 1) * self.nx]);
        }
        ModelHeights {
            min: self.min + Vec2::new(lo_i as f64 * c, lo_j as f64 * c),
            cell: c,
            nx,
            ny,
            z,
            z_min: self.z_min,
            z_max: self.z_max,
        }
    }

    /// Mapa „pogrubiona” o `radius`: w każdym oczku najwyższy punkt modelu w kole
    /// o tym promieniu (filtr maksimum: wiersze koła × przesuwne maksimum w wierszu).
    pub fn dilated(&self, radius: f64) -> ModelHeights {
        let k = (radius / self.cell).ceil() as isize;
        if k <= 0 {
            return self.clone();
        }
        let (nx, ny) = (self.nx as isize, self.ny as isize);
        let mut out = vec![f32::NEG_INFINITY; self.z.len()];
        let mut row = vec![f32::NEG_INFINITY; self.nx];
        let mut dq: std::collections::VecDeque<isize> = std::collections::VecDeque::new();
        for dy in -k..=k {
            let w = ((radius / self.cell).powi(2) - (dy as f64).powi(2)).max(0.0).sqrt().floor() as isize;
            for j in 0..ny {
                let src = j + dy;
                if src < 0 || src >= ny {
                    continue;
                }
                let s = &self.z[(src * nx) as usize..((src + 1) * nx) as usize];
                // przesuwne maksimum okna [i − w, i + w]
                dq.clear();
                let mut next = 0isize;
                for i in 0..nx {
                    while next <= (i + w).min(nx - 1) {
                        while dq.back().is_some_and(|&b| s[b as usize] <= s[next as usize]) {
                            dq.pop_back();
                        }
                        dq.push_back(next);
                        next += 1;
                    }
                    while dq.front().is_some_and(|&f| f < i - w) {
                        dq.pop_front();
                    }
                    row[i as usize] = dq.front().map_or(f32::NEG_INFINITY, |&f| s[f as usize]);
                }
                let o = &mut out[(j * nx) as usize..((j + 1) * nx) as usize];
                for (a, b) in o.iter_mut().zip(&row) {
                    *a = a.max(*b);
                }
            }
        }
        ModelHeights { min: self.min, cell: self.cell, nx: self.nx, ny: self.ny, z: out, z_min: self.z_min, z_max: self.z_max }
    }

    /// Najniższe Z czubka narzędzia nad punktem `p`, przy którym narzędzie nie
    /// wcina się w model (z naddatkiem `allowance`); `None`, gdy pod narzędziem nie ma modelu.
    pub fn drop_tool(&self, p: Vec2, tool: &ToolProfile, allowance: f64) -> Option<f64> {
        let r = tool.radius + allowance;
        let k = (r / self.cell).ceil() as isize + 1;
        let ci = ((p.x - self.min.x) / self.cell - 0.5).round() as isize;
        let cj = ((p.y - self.min.y) / self.cell - 0.5).round() as isize;
        let half_diag = self.cell * std::f64::consts::FRAC_1_SQRT_2;
        let mut best: Option<f64> = None;
        for j in cj - k..=cj + k {
            for i in ci - k..=ci + k {
                let z = self.at(i, j);
                if !z.is_finite() {
                    continue;
                }
                let c = Vec2::new(self.min.x + (i as f64 + 0.5) * self.cell, self.min.y + (j as f64 + 0.5) * self.cell);
                // oczko to kwadrat, nie punkt: najbliższy jego punkt (ostrożnie — bez wcinania)
                let d = (c.distance(p) - half_diag).max(0.0);
                if d > r {
                    continue;
                }
                // narzędzie z naddatkiem: profil liczony dla odległości pomniejszonej o naddatek
                let tip = z as f64 + allowance - tool.height_at((d - allowance).max(0.0));
                if best.is_none_or(|b| tip > b) {
                    best = Some(tip);
                }
            }
        }
        best
    }
}

/// Profil narzędzia: wysokość powierzchni tnącej nad czubkiem w odległości `d` od osi.
#[derive(Debug, Clone, Copy)]
pub struct ToolProfile {
    pub radius: f64,
    kind: ProfileKind,
}

#[derive(Debug, Clone, Copy)]
enum ProfileKind {
    Flat,
    Ball,
    Bull { rc: f64 },
    Cone { tip_r: f64, slope: f64 },
}

impl ToolProfile {
    pub fn of(t: &Tool) -> Self {
        let r = t.radius();
        let kind = match t.kind {
            ToolKind::FlatEnd | ToolKind::Drill { .. } => ProfileKind::Flat,
            ToolKind::BallEnd => ProfileKind::Ball,
            ToolKind::BullNose { corner_radius } => ProfileKind::Bull { rc: corner_radius.clamp(0.0, r) },
            ToolKind::VBit { angle, tip_diameter } | ToolKind::Engraver { angle, tip_diameter } => {
                ProfileKind::Cone { tip_r: tip_diameter * 0.5, slope: 1.0 / (angle.clamp(1.0, 179.0) * 0.5).to_radians().tan() }
            }
        };
        ToolProfile { radius: r, kind }
    }

    pub fn height_at(&self, d: f64) -> f64 {
        let r = self.radius;
        match self.kind {
            ProfileKind::Flat => 0.0,
            ProfileKind::Ball => r - (r * r - d.min(r).powi(2)).max(0.0).sqrt(),
            ProfileKind::Bull { rc } => {
                let flat = r - rc;
                if d <= flat {
                    0.0
                } else {
                    let e = (d - flat).min(rc);
                    rc - (rc * rc - e * e).max(0.0).sqrt()
                }
            }
            ProfileKind::Cone { tip_r, slope } => (d - tip_r).max(0.0) * slope,
        }
    }
}

/// Upraszczanie zamkniętej łamanej (Douglas–Peucker) z tolerancją `tol`.
fn simplify_closed(pts: &[Vec2], tol: f64) -> Vec<Vec2> {
    if pts.len() < 4 {
        return pts.to_vec();
    }
    // podział w punkcie najdalszym od pierwszego
    let far = (1..pts.len()).max_by(|&a, &b| pts[a].distance(pts[0]).partial_cmp(&pts[b].distance(pts[0])).unwrap()).unwrap();
    let mut a: Vec<Vec2> = pts[..=far].to_vec();
    let mut b: Vec<Vec2> = pts[far..].to_vec();
    b.push(pts[0]);
    a = dp(&a, tol);
    b = dp(&b, tol);
    a.pop();
    b.pop();
    a.extend(b);
    a
}

fn dp(pts: &[Vec2], tol: f64) -> Vec<Vec2> {
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let ab = pb - pa;
        let l2 = ab.length_sq();
        let mut worst = (0.0, a);
        for (k, &q) in pts.iter().enumerate().take(b).skip(a + 1) {
            let t = if l2 > 0.0 { ((q - pa).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let d = q.distance(pa + ab * t);
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
    use super::*;

    /// Płyta 40 × 30 grubości 5 (od Z = −10 do −5) z kostką 10 × 10 do Z = 0 na środku —
    /// tylko powierzchnie z góry (wystarczą do mapy wysokości).
    fn stepped() -> Vec<[[f32; 3]; 3]> {
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32, z: f32| {
            [[[x0, y0, z], [x1, y0, z], [x1, y1, z]], [[x0, y0, z], [x1, y1, z], [x0, y1, z]]]
        };
        let mut t = Vec::new();
        t.extend(quad(0.0, 0.0, 40.0, 30.0, -5.0));
        t.extend(quad(15.0, 10.0, 25.0, 20.0, 0.0));
        t
    }

    #[test]
    fn heights_and_outlines() {
        let h = ModelHeights::from_triangles(&stepped(), 0.25).unwrap();
        assert_eq!(h.height_at(Vec2::new(20.0, 15.0)), Some(0.0));
        assert_eq!(h.height_at(Vec2::new(5.0, 5.0)), Some(-5.0));
        // sylwetka: prostokąt 40 × 30
        let sil = h.outline_above(-100.0);
        assert_eq!(sil.len(), 1);
        let bb = sil[0].bbox();
        assert!((bb.size().x - 40.0).abs() < 0.3 && (bb.size().y - 30.0).abs() < 0.3, "{:?}", bb.size());
        assert!(sil[0].signed_area() > 0.0, "zewnętrzny obrys CCW");
        // powyżej Z = −2: tylko kostka 10 × 10
        let top = h.outline_above(-2.0);
        assert_eq!(top.len(), 1);
        let bb = top[0].bbox();
        assert!((bb.size().x - 10.0).abs() < 0.3 && (bb.center().x - 20.0).abs() < 0.2, "{:?}", bb);
        // brzegi na środkach oczek: obrys najwyżej pół oczka na zewnątrz
        let a = top[0].signed_area();
        assert!(a >= 100.0 - 1.0 && a <= 10.25f64.powi(2) + 1.0, "{a}");
        // nad modelem: nic
        assert!(h.outline_above(1.0).is_empty());
    }

    #[test]
    fn dropping_tools_on_the_model() {
        let h = ModelHeights::from_triangles(&stepped(), 0.1).unwrap();
        let flat = ToolProfile::of(&Tool { diameter: 6.0, ..Default::default() });
        // płaski Ø6 tuż obok kostki (krawędź freza nad kostką): czubek na wysokości kostki
        let z = h.drop_tool(Vec2::new(12.5, 15.0), &flat, 0.0).unwrap();
        assert!(z.abs() < 1e-6, "{z}");
        // daleko od kostki: na płycie
        assert!((h.drop_tool(Vec2::new(5.0, 5.0), &flat, 0.0).unwrap() + 5.0).abs() < 1e-6);
        // naddatek 0,5: czubek o 0,5 wyżej
        assert!((h.drop_tool(Vec2::new(5.0, 5.0), &flat, 0.5).unwrap() + 4.5).abs() < 1e-6);
        // kulowy Ø6 na płaskiej płycie: czubek na płycie
        let ball = ToolProfile::of(&Tool { diameter: 6.0, kind: ToolKind::BallEnd, ..Default::default() });
        assert!((h.drop_tool(Vec2::new(5.0, 5.0), &ball, 0.0).unwrap() + 5.0).abs() < 1e-6);
        // kulowy przy krawędzi kostki: dotyka narożnika — czubek niżej niż kostka, wyżej niż płyta
        let zb = h.drop_tool(Vec2::new(13.0, 15.0), &ball, 0.0).unwrap();
        assert!(zb < 0.0 && zb > -5.0, "{zb}");
        // profile
        assert!((ball.height_at(3.0) - 3.0).abs() < 1e-9 && ball.height_at(0.0) == 0.0);
    }
}
