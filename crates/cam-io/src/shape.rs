//! Wypukły kształt z zamkniętych krzywych 2D: obszar wewnątrz krzywych
//! (parzysto-nieparzyście — otwory w literach zostają otworami) podniesiony
//! nad podstawę z wybranym profilem krawędzi. Wysokość zależy od odległości od
//! krawędzi kształtu (dokładna transformata odległości na siatce).

use crate::relief::{heightfield_mesh, round_nice};
use crate::stl::Mesh;

/// Najwięcej punktów siatki na dłuższym boku.
pub const MAX_SHAPE_GRID: f64 = 1000.0;

/// Profil krawędzi kształtu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeProfile {
    /// Pionowe ścianki, płaska góra.
    Flat,
    /// Krawędź zaokrąglona promieniem `radius`, płaska góra.
    Round,
    /// Skos pod kątem `angle` od poziomu, płaska góra (gdy kształt dość szeroki).
    Bevel,
    /// Kopuła (poduszka): najwyżej w środku każdego kształtu.
    Dome,
}

impl ShapeProfile {
    pub const ALL: [ShapeProfile; 4] = [ShapeProfile::Flat, ShapeProfile::Round, ShapeProfile::Bevel, ShapeProfile::Dome];

    pub fn key(&self) -> &'static str {
        match self {
            ShapeProfile::Flat => "flat",
            ShapeProfile::Round => "round",
            ShapeProfile::Bevel => "bevel",
            ShapeProfile::Dome => "dome",
        }
    }

    pub fn from_key(k: &str) -> Self {
        Self::ALL.into_iter().find(|p| p.key() == k).unwrap_or(ShapeProfile::Round)
    }
}

/// Ustawienia kształtu [mm].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeParams {
    pub profile: ShapeProfile,
    /// Wysokość kształtu nad podstawą.
    pub height: f64,
    /// Promień zaokrąglenia krawędzi (profil `Round`).
    pub radius: f64,
    /// Kąt skosu od poziomu [°] (profil `Bevel`).
    pub angle: f64,
    /// Podstawa pod kształtem (i tło wokół niego).
    pub base: f64,
    /// Tło wokół kształtu (z każdej strony).
    pub margin: f64,
    /// Odstęp punktów siatki.
    pub cell: f64,
    /// Wklęsłe: kształt wgłębiony w płaskiej górze (np. misa, wgłębiony napis).
    pub inverted: bool,
}

impl Default for ShapeParams {
    fn default() -> Self {
        ShapeParams { profile: ShapeProfile::Round, height: 3.0, radius: 2.0, angle: 45.0, base: 2.0, margin: 5.0, cell: 0.2, inverted: false }
    }
}

impl ShapeParams {
    /// Ustawienia dla nowych krzywych: siatka ok. 400 punktów na dłuższym boku.
    pub fn for_shapes(polys: &[Vec<[f64; 2]>]) -> Self {
        let mut p = ShapeParams::default();
        if let Some((lo, hi)) = bounds(polys) {
            let long = (hi[0] - lo[0]).max(hi[1] - lo[1]) + 2.0 * p.margin;
            p.cell = round_nice(long / 400.0).max(0.05);
        }
        p
    }

    fn effective_cell(&self, w: f64, h: f64) -> f64 {
        self.cell.max(w.max(h) / MAX_SHAPE_GRID).max(0.01)
    }
}

pub fn bounds(polys: &[Vec<[f64; 2]>]) -> Option<([f64; 2], [f64; 2])> {
    let mut it = polys.iter().flatten();
    let f = *it.next()?;
    let (mut lo, mut hi) = (f, f);
    for p in it {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((lo, hi))
}

/// Siatka wysokości kształtu: (punkty X, punkty Y, odstęp, lewy dolny róg w
/// układzie krzywych, wysokości wierszami od dołu, Z od 0 = spód podstawy).
pub fn shape_grid(polys: &[Vec<[f64; 2]>], p: &ShapeParams) -> Option<(usize, usize, f64, [f64; 2], Vec<f32>)> {
    let (lo, hi) = bounds(polys)?;
    let m = p.margin.max(0.0);
    let origin = [lo[0] - m, lo[1] - m];
    let (w, h) = (hi[0] - lo[0] + 2.0 * m, hi[1] - lo[1] + 2.0 * m);
    let cell = p.effective_cell(w, h);
    let nx = (w / cell).ceil() as usize + 1;
    let ny = (h / cell).ceil() as usize + 1;
    if nx < 2 || ny < 2 {
        return None;
    }
    let inside = inside_mask(polys, origin, cell, nx, ny);
    // odległość [mm] od krawędzi kształtu dla węzłów wewnątrz (pół oczka mniej:
    // krawędź leży między węzłem wewnątrz a węzłem na zewnątrz)
    let d2 = edt(&inside, nx, ny);
    let dist: Vec<f32> = d2.iter().map(|&q| ((q.sqrt() - 0.5).max(0.0) * cell) as f32).collect();
    // kopuła: najdalszy punkt każdego kształtu (spójnego obszaru)
    let dmax = if p.profile == ShapeProfile::Dome { component_max(&inside, &dist, nx, ny) } else { Vec::new() };
    let (height, base) = (p.height.max(0.0) as f32, p.base.max(0.0) as f32);
    let radius = p.radius.max(1e-3) as f32;
    let slope = (p.angle.clamp(1.0, 89.0)).to_radians().tan() as f32;
    let quarter = |s: f32| (1.0 - (1.0 - s.clamp(0.0, 1.0)).powi(2)).sqrt();
    let mut z = vec![0f32; nx * ny];
    for k in 0..nx * ny {
        let t = if !inside[k] {
            0.0
        } else {
            let d = dist[k];
            match p.profile {
                ShapeProfile::Flat => 1.0,
                ShapeProfile::Round => quarter(d / radius),
                ShapeProfile::Bevel => (d * slope / height.max(1e-6)).min(1.0),
                ShapeProfile::Dome => quarter(d / dmax[k].max(1e-6)),
            }
        };
        let t = if p.inverted { 1.0 - t } else { t };
        z[k] = base + height * t;
    }
    Some((nx, ny, cell, origin, z))
}

/// Zamknięta bryła kształtu [mm] i lewy dolny róg spodu w układzie krzywych.
pub fn shape_mesh(polys: &[Vec<[f64; 2]>], p: &ShapeParams) -> Option<(Mesh, [f64; 2])> {
    let (nx, ny, cell, origin, z) = shape_grid(polys, p)?;
    Some((heightfield_mesh(nx, ny, cell, cell, &z), origin))
}

/// Węzły wewnątrz krzywych (reguła parzysto-nieparzysta, wiersz po wierszu).
fn inside_mask(polys: &[Vec<[f64; 2]>], origin: [f64; 2], cell: f64, nx: usize, ny: usize) -> Vec<bool> {
    let mut mask = vec![false; nx * ny];
    let mut xs: Vec<f64> = Vec::new();
    for j in 0..ny {
        let y = origin[1] + j as f64 * cell;
        xs.clear();
        for poly in polys {
            let n = poly.len();
            for k in 0..n {
                let (a, b) = (poly[k], poly[(k + 1) % n]);
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let i0 = ((pair[0] - origin[0]) / cell).ceil().max(0.0) as usize;
            let i1 = ((pair[1] - origin[0]) / cell).floor();
            if i1 < 0.0 {
                continue;
            }
            for i in i0..=(i1 as usize).min(nx - 1) {
                mask[j * nx + i] = true;
            }
        }
    }
    mask
}

/// Kwadrat odległości (w oczkach) każdego węzła wewnątrz do najbliższego węzła na
/// zewnątrz — dokładna transformata odległości (Felzenszwalb, Huttenlocher).
fn edt(inside: &[bool], nx: usize, ny: usize) -> Vec<f64> {
    const INF: f64 = 1e20;
    let mut g: Vec<f64> = inside.iter().map(|&i| if i { INF } else { 0.0 }).collect();
    let mut f = Vec::new();
    let mut out = Vec::new();
    // kolumny
    for i in 0..nx {
        f.clear();
        f.extend((0..ny).map(|j| g[j * nx + i]));
        dt1(&f, &mut out);
        for j in 0..ny {
            g[j * nx + i] = out[j];
        }
    }
    // wiersze
    for j in 0..ny {
        f.clear();
        f.extend_from_slice(&g[j * nx..(j + 1) * nx]);
        dt1(&f, &mut out);
        g[j * nx..(j + 1) * nx].copy_from_slice(&out);
    }
    g
}

/// Jednowymiarowa transformata: d[q] = min_p (q − p)² + f[p].
fn dt1(f: &[f64], d: &mut Vec<f64>) {
    let n = f.len();
    d.clear();
    d.resize(n, 0.0);
    let mut v = vec![0usize; n];
    let mut z = vec![0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        // z[0] = −∞, więc pętla zawsze zatrzyma się na k ≥ 0
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * q as f64 - 2.0 * p as f64);
            if s <= z[k] {
                k -= 1;
            } else {
                break;
            }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f64 - p as f64).powi(2) + f[p];
    }
}

/// Dla każdego węzła wewnątrz: największa odległość od krawędzi w jego
/// spójnym obszarze (sąsiedztwo 4) — wysokość kopuły liczona osobno dla każdej litery.
fn component_max(inside: &[bool], dist: &[f32], nx: usize, ny: usize) -> Vec<f32> {
    let mut label = vec![usize::MAX; nx * ny];
    let mut maxes: Vec<f32> = Vec::new();
    let mut stack = Vec::new();
    for s in 0..nx * ny {
        if !inside[s] || label[s] != usize::MAX {
            continue;
        }
        let id = maxes.len();
        let mut m = 0f32;
        label[s] = id;
        stack.push(s);
        while let Some(k) = stack.pop() {
            m = m.max(dist[k]);
            let (i, j) = (k % nx, k / nx);
            let mut visit = |n: usize| {
                if inside[n] && label[n] == usize::MAX {
                    label[n] = id;
                    stack.push(n);
                }
            };
            if i > 0 {
                visit(k - 1);
            }
            if i + 1 < nx {
                visit(k + 1);
            }
            if j > 0 {
                visit(k - nx);
            }
            if j + 1 < ny {
                visit(k + nx);
            }
        }
        maxes.push(m);
    }
    label.iter().map(|&l| if l == usize::MAX { 0.0 } else { maxes[l] }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kwadrat 20 × 20 z otworem 6 × 6 w środku (jak litera „O”).
    fn ring() -> Vec<Vec<[f64; 2]>> {
        vec![
            vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
            vec![[7.0, 7.0], [13.0, 7.0], [13.0, 13.0], [7.0, 13.0]],
        ]
    }

    fn at(g: &(usize, usize, f64, [f64; 2], Vec<f32>), x: f64, y: f64) -> f32 {
        let (nx, _, cell, o, z) = g;
        let i = ((x - o[0]) / cell).round() as usize;
        let j = ((y - o[1]) / cell).round() as usize;
        z[j * nx + i]
    }

    #[test]
    fn distance_transform_is_exact() {
        // pojedynczy węzeł na zewnątrz pośrodku 7 × 7: odległości euklidesowe
        let (nx, ny) = (7, 7);
        let mut inside = vec![true; 49];
        inside[3 * 7 + 3] = false;
        let d = edt(&inside, nx, ny);
        assert_eq!(d[3 * 7 + 3], 0.0);
        assert_eq!(d[3 * 7 + 5], 4.0);
        assert_eq!(d[0], 18.0);
        assert_eq!(d[6 * 7 + 1], 13.0);
    }

    #[test]
    fn profiles_heights_and_holes() {
        let polys = ring();
        let p = ShapeParams { profile: ShapeProfile::Flat, height: 3.0, base: 2.0, margin: 4.0, cell: 0.1, ..Default::default() };
        let g = shape_grid(&polys, &p).unwrap();
        assert_eq!(g.3, [-4.0, -4.0]);
        assert_eq!(at(&g, -2.0, -2.0), 2.0, "tło na podstawie");
        assert_eq!(at(&g, 3.0, 3.0), 5.0, "płaska góra");
        assert_eq!(at(&g, 10.0, 10.0), 2.0, "otwór zostaje otworem");
        // zaokrąglenie R 2: przy krawędzi nisko, od 2 mm w głąb — pełna wysokość
        let r = shape_grid(&polys, &ShapeParams { profile: ShapeProfile::Round, radius: 2.0, ..p }).unwrap();
        assert!(at(&r, 0.3, 10.0) < 4.0, "{}", at(&r, 0.3, 10.0));
        assert_eq!(at(&r, 3.0, 3.5), 5.0);
        // skos 45°, wysokość 3: pełna wysokość dopiero 3 mm od krawędzi
        let b = shape_grid(&polys, &ShapeParams { profile: ShapeProfile::Bevel, angle: 45.0, ..p }).unwrap();
        let mid = at(&b, 1.5, 10.0);
        assert!((mid - 3.5).abs() < 0.15, "{mid}");
        assert_eq!(at(&b, 3.5, 3.5), 5.0);
        // kopuła: w środku pierścienia (3,5 mm od obu krawędzi) najwyżej
        let d = shape_grid(&polys, &ShapeParams { profile: ShapeProfile::Dome, ..p }).unwrap();
        assert!((at(&d, 3.5, 10.0) - 5.0).abs() < 0.05);
        assert!(at(&d, 1.0, 10.0) < 4.9);
        // wklęsłe: tło na górze, kształt w dół
        let v = shape_grid(&polys, &ShapeParams { inverted: true, ..p }).unwrap();
        assert_eq!((at(&v, -2.0, -2.0), at(&v, 3.0, 3.0)), (5.0, 2.0));
        // bryła: wymiary z marginesem
        let (mesh, o) = shape_mesh(&polys, &p).unwrap();
        let (lo, hi) = mesh.bounds().unwrap();
        assert_eq!(o, [-4.0, -4.0]);
        assert_eq!(lo, [0.0, 0.0, 0.0]);
        assert!((hi[0] - 28.0).abs() < 0.11 && (hi[2] - 5.0).abs() < 1e-6, "{hi:?}");
    }
}
