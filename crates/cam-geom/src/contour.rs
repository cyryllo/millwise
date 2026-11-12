use crate::math::{normalize_angle, Aabb2, Vec2};
use cavalier_contours::polyline::{PlineSource, PlineSourceMut, Polyline};

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub p: Vec2,
    /// Bulge segmentu zaczynającego się w tym wierzchołku.
    pub bulge: f64,
}

impl Vertex {
    pub fn new(x: f64, y: f64, bulge: f64) -> Self {
        Self { p: Vec2::new(x, y), bulge }
    }
}

/// Geometria łuku wyliczona z dwóch punktów i bulge.
#[derive(Debug, Clone, Copy)]
pub struct ArcGeom {
    pub center: Vec2,
    pub radius: f64,
    pub start_angle: f64,
    /// Kąt łuku ze znakiem (dodatni = CCW).
    pub sweep: f64,
}

impl ArcGeom {
    pub fn from_bulge(a: Vec2, b: Vec2, bulge: f64) -> ArcGeom {
        let sweep = 4.0 * bulge.atan();
        let chord = b - a;
        let c = chord.length();
        let radius = c / (2.0 * (sweep / 2.0).sin().abs());
        // odległość środka od środka cięciwy (ze znakiem)
        let mid = (a + b) * 0.5;
        let h = radius * (sweep / 2.0).cos().abs();
        let n = chord.perp().normalized();
        // dla |sweep| < π środek po stronie skrętu, dla > π po przeciwnej
        let side = if (sweep.abs() < PI) == (bulge > 0.0) { 1.0 } else { -1.0 };
        let center = mid + n * (h * side);
        ArcGeom { center, radius, start_angle: (a - center).angle(), sweep }
    }

    pub fn point_at(&self, t: f64) -> Vec2 {
        self.center + Vec2::from_angle(self.start_angle + self.sweep * t) * self.radius
    }

    /// Czy kąt `ang` leży w zakresie łuku.
    pub fn contains_angle(&self, ang: f64) -> bool {
        let rel = if self.sweep >= 0.0 {
            normalize_angle(ang - self.start_angle)
        } else {
            normalize_angle(self.start_angle - ang)
        };
        rel <= self.sweep.abs() + 1e-12
    }
}

/// Zamknięty lub otwarty kontur złożony z odcinków i łuków.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Contour {
    pub verts: Vec<Vertex>,
    pub closed: bool,
}

impl Contour {
    pub fn new(closed: bool) -> Self {
        Self { verts: Vec::new(), closed }
    }

    /// Okrąg jako dwa półłuki (CCW).
    pub fn circle(center: Vec2, r: f64) -> Self {
        Contour {
            verts: vec![
                Vertex::new(center.x - r, center.y, 1.0),
                Vertex::new(center.x + r, center.y, 1.0),
            ],
            closed: true,
        }
    }

    pub fn segment_count(&self) -> usize {
        match (self.verts.len(), self.closed) {
            (0 | 1, _) => 0,
            (n, true) => n,
            (n, false) => n - 1,
        }
    }

    /// Segment `i`: (początek, koniec, bulge).
    pub fn segment(&self, i: usize) -> (Vec2, Vec2, f64) {
        let a = self.verts[i];
        let b = self.verts[(i + 1) % self.verts.len()];
        (a.p, b.p, a.bulge)
    }

    pub fn start(&self) -> Option<Vec2> {
        self.verts.first().map(|v| v.p)
    }

    pub fn end(&self) -> Option<Vec2> {
        if self.closed { self.start() } else { self.verts.last().map(|v| v.p) }
    }

    /// Pole ze znakiem (dodatnie = CCW). Dla otwartych konturów 0.
    pub fn signed_area(&self) -> f64 {
        if !self.closed || self.verts.len() < 2 {
            return 0.0;
        }
        self.to_cavc().area()
    }

    pub fn length(&self) -> f64 {
        (0..self.segment_count())
            .map(|i| {
                let (a, b, bulge) = self.segment(i);
                if bulge == 0.0 {
                    a.distance(b)
                } else {
                    let arc = ArcGeom::from_bulge(a, b, bulge);
                    arc.radius * arc.sweep.abs()
                }
            })
            .sum()
    }

    pub fn bbox(&self) -> Aabb2 {
        let mut bb = Aabb2::EMPTY;
        for i in 0..self.segment_count() {
            let (a, b, bulge) = self.segment(i);
            bb.add_point(a);
            bb.add_point(b);
            if bulge != 0.0 {
                let arc = ArcGeom::from_bulge(a, b, bulge);
                for k in 0..4 {
                    let ang = k as f64 * PI / 2.0;
                    if arc.contains_angle(ang) {
                        bb.add_point(arc.center + Vec2::from_angle(ang) * arc.radius);
                    }
                }
            }
        }
        if self.segment_count() == 0 {
            for v in &self.verts {
                bb.add_point(v.p);
            }
        }
        bb
    }

    /// Odwraca kierunek konturu (bulge zmienia znak i przesuwa się o jeden wierzchołek).
    pub fn reverse(&mut self) {
        let n = self.verts.len();
        if n < 2 {
            return;
        }
        let bulges: Vec<f64> = self.verts.iter().map(|v| v.bulge).collect();
        self.verts.reverse();
        for i in 0..n {
            // nowy wierzchołek i (stary n-1-i) startuje segment, który
            // wcześniej szedł od starego n-2-i do n-1-i
            let old_seg = if i + 1 < n { n - 2 - i } else { n - 1 };
            self.verts[i].bulge = -bulges[old_seg];
        }
        if !self.closed {
            self.verts[n - 1].bulge = 0.0;
        }
    }

    /// Zamienia kontur na punkty (łuki aproksymowane z błędem cięciwy `tol`).
    pub fn flatten(&self, tol: f64) -> Vec<Vec2> {
        let mut out = Vec::with_capacity(self.verts.len() * 2);
        let n = self.segment_count();
        if n == 0 {
            return self.verts.iter().map(|v| v.p).collect();
        }
        for i in 0..n {
            let (a, b, bulge) = self.segment(i);
            if i == 0 {
                out.push(a);
            }
            if bulge != 0.0 {
                let arc = ArcGeom::from_bulge(a, b, bulge);
                let steps = arc_steps(arc.radius, arc.sweep, tol);
                for k in 1..steps {
                    out.push(arc.point_at(k as f64 / steps as f64));
                }
            }
            out.push(b);
        }
        out
    }

    /// Najmniejsza odległość punktu od konturu.
    pub fn distance_to(&self, p: Vec2) -> f64 {
        let mut best = f64::INFINITY;
        for i in 0..self.segment_count() {
            let (a, b, bulge) = self.segment(i);
            let d = if bulge == 0.0 {
                dist_point_segment(p, a, b)
            } else {
                let arc = ArcGeom::from_bulge(a, b, bulge);
                if arc.contains_angle((p - arc.center).angle()) {
                    (p.distance(arc.center) - arc.radius).abs()
                } else {
                    p.distance(a).min(p.distance(b))
                }
            };
            best = best.min(d);
        }
        if self.segment_count() == 0 {
            if let Some(v) = self.verts.first() {
                best = p.distance(v.p);
            }
        }
        best
    }

    /// Przekształcenie zachowujące okręgi: p' = M·p + t (obrót, skala jednorodna, odbicie, przesunięcie).
    pub fn transform(&mut self, m: [[f64; 2]; 2], t: Vec2) {
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        for v in &mut self.verts {
            let p = v.p;
            v.p = Vec2::new(m[0][0] * p.x + m[0][1] * p.y + t.x, m[1][0] * p.x + m[1][1] * p.y + t.y);
            if det < 0.0 {
                v.bulge = -v.bulge;
            }
        }
    }

    pub fn translate(&mut self, d: Vec2) {
        self.transform([[1.0, 0.0], [0.0, 1.0]], d);
    }

    /// Obrót o kąt `ang` [rad] wokół punktu `c`.
    pub fn rotate(&mut self, c: Vec2, ang: f64) {
        let (s, co) = ang.sin_cos();
        let m = [[co, -s], [s, co]];
        let t = c - Vec2::new(co * c.x - s * c.y, s * c.x + co * c.y);
        self.transform(m, t);
    }

    /// Skalowanie jednorodne względem punktu `c`.
    pub fn scale(&mut self, c: Vec2, k: f64) {
        self.transform([[k, 0.0], [0.0, k]], c - c * k);
    }

    /// Odbicie względem pionowej osi x = `cx` (poziome odbicie).
    pub fn mirror_x(&mut self, cx: f64) {
        self.transform([[-1.0, 0.0], [0.0, 1.0]], Vec2::new(2.0 * cx, 0.0));
    }

    /// Odbicie względem poziomej osi y = `cy` (pionowe odbicie).
    pub fn mirror_y(&mut self, cy: f64) {
        self.transform([[1.0, 0.0], [0.0, -1.0]], Vec2::new(0.0, 2.0 * cy));
    }

    /// Czy punkt leży wewnątrz zamkniętego konturu.
    pub fn contains_point(&self, p: Vec2) -> bool {
        self.closed && self.verts.len() >= 2 && self.to_cavc().winding_number(cavalier_contours::core::math::Vector2::new(p.x, p.y)) != 0
    }

    /// Jeśli kontur jest okręgiem (dwa półłuki), zwraca (środek, promień).
    pub fn as_circle(&self) -> Option<(Vec2, f64)> {
        if !self.closed || self.verts.len() != 2 {
            return None;
        }
        let (a, b) = (self.verts[0], self.verts[1]);
        let same_dir = a.bulge.signum() == b.bulge.signum();
        if same_dir && (a.bulge.abs() - 1.0).abs() < 1e-6 && (b.bulge.abs() - 1.0).abs() < 1e-6 {
            Some(((a.p + b.p) * 0.5, a.p.distance(b.p) * 0.5))
        } else {
            None
        }
    }

    /// Najbliższy punkt konturu: (indeks segmentu, punkt).
    pub fn closest_point(&self, p: Vec2) -> Option<(usize, Vec2)> {
        let mut best: Option<(usize, Vec2, f64)> = None;
        for i in 0..self.segment_count() {
            let (a, b, bulge) = self.segment(i);
            let q = if bulge == 0.0 {
                let ab = b - a;
                let l2 = ab.length_sq();
                let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
                a + ab * t
            } else {
                let arc = ArcGeom::from_bulge(a, b, bulge);
                let ang = (p - arc.center).angle();
                if arc.contains_angle(ang) && p.distance(arc.center) > 1e-12 {
                    arc.center + Vec2::from_angle(ang) * arc.radius
                } else if p.distance(a) < p.distance(b) {
                    a
                } else {
                    b
                }
            };
            let d = q.distance(p);
            if best.is_none_or(|bb| d < bb.2) {
                best = Some((i, q, d));
            }
        }
        best.map(|b| (b.0, b.1))
    }

    /// Przesuwa początek zamkniętego konturu do punktu `q` leżącego na segmencie `seg`.
    pub fn rotate_start_to(&mut self, seg: usize, q: Vec2) {
        if !self.closed || self.verts.is_empty() {
            return;
        }
        let n = self.verts.len();
        let (a, b, bulge) = self.segment(seg);
        let eps = 1e-9;
        let start = if q.distance(a) <= eps {
            seg
        } else if q.distance(b) <= eps {
            (seg + 1) % n
        } else {
            // podziel segment w punkcie q
            let (b1, b2) = if bulge == 0.0 {
                (0.0, 0.0)
            } else {
                let arc = ArcGeom::from_bulge(a, b, bulge);
                let aq = (q - arc.center).angle();
                let s1 = if arc.sweep > 0.0 {
                    normalize_angle(aq - arc.start_angle)
                } else {
                    -normalize_angle(arc.start_angle - aq)
                };
                let s2 = arc.sweep - s1;
                ((s1 / 4.0).tan(), (s2 / 4.0).tan())
            };
            self.verts[seg].bulge = b1;
            self.verts.insert(seg + 1, Vertex { p: q, bulge: b2 });
            seg + 1
        };
        self.verts.rotate_left(start);
    }

    pub fn to_cavc(&self) -> Polyline<f64> {
        let mut pl = if self.closed { Polyline::new_closed() } else { Polyline::new() };
        for v in &self.verts {
            pl.add(v.p.x, v.p.y, v.bulge);
        }
        pl
    }

    pub fn from_cavc(pl: &Polyline<f64>) -> Self {
        Contour {
            verts: pl.iter_vertexes().map(|v| Vertex::new(v.x, v.y, v.bulge)).collect(),
            closed: pl.is_closed(),
        }
    }
}

/// Liczba kroków aproksymacji łuku przy zadanym błędzie cięciwy.
pub fn arc_steps(radius: f64, sweep: f64, tol: f64) -> usize {
    let tol = tol.max(1e-6).min(radius * 0.5);
    let max_step = 2.0 * (1.0 - tol / radius).clamp(-1.0, 1.0).acos();
    let max_step = max_step.clamp(0.5f64.to_radians(), 15f64.to_radians());
    ((sweep.abs() / max_step).ceil() as usize).max(2)
}

pub fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cavalier_contours::polyline::PlineSource;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn circle_area_and_length() {
        let c = Contour::circle(Vec2::new(5.0, 5.0), 10.0);
        assert!(approx(c.signed_area(), PI * 100.0));
        assert!(approx(c.length(), 2.0 * PI * 10.0));
        let bb = c.bbox();
        assert!(approx(bb.min.x, -5.0) && approx(bb.max.y, 15.0));
    }

    #[test]
    fn arc_geometry_quarter() {
        // ćwiartka CCW od (1,0) do (0,1) wokół (0,0)
        let arc = ArcGeom::from_bulge(Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), (PI / 8.0).tan());
        assert!(arc.center.length() < 1e-9);
        assert!(approx(arc.radius, 1.0));
        // ćwiartka CW od (1,0) do (0,-1) wokół (0,0)
        let arc = ArcGeom::from_bulge(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), -(PI / 8.0).tan());
        assert!(arc.center.length() < 1e-9);
        // łuk 270° CCW od (1,0) do (0,-1) wokół (0,0)
        let arc = ArcGeom::from_bulge(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), (3.0 * PI / 8.0).tan());
        assert!(arc.center.length() < 1e-9, "{:?}", arc.center);
        assert!(approx(arc.radius, 1.0));
    }

    #[test]
    fn reverse_keeps_shape() {
        let mut c = Contour {
            verts: vec![
                Vertex::new(0.0, 0.0, 0.0),
                Vertex::new(10.0, 0.0, 0.5),
                Vertex::new(10.0, 10.0, 0.0),
                Vertex::new(0.0, 10.0, 0.0),
            ],
            closed: true,
        };
        let area = c.signed_area();
        c.reverse();
        assert!(approx(c.signed_area(), -area));
        c.reverse();
        assert!(approx(c.signed_area(), area));
    }

    #[test]
    fn transforms_keep_shape() {
        let mut c = Contour::circle(Vec2::new(10.0, 0.0), 5.0);
        let a = c.signed_area();
        c.rotate(Vec2::ZERO, PI / 2.0);
        assert!((c.bbox().center().y - 10.0).abs() < 1e-9);
        c.mirror_x(1.0);
        // odbicie odwraca kierunek obiegu, kształt (okrąg) zostaje
        assert!((c.signed_area() + a).abs() < 1e-9);
        assert!((c.bbox().center().x - 2.0).abs() < 1e-9 && (c.bbox().size().x - 10.0).abs() < 1e-9);
        c.scale(Vec2::ZERO, 2.0);
        assert!((c.signed_area() + 4.0 * a).abs() < 1e-6);
        c.translate(Vec2::new(1.0, 1.0));
        assert!((c.bbox().center().y - 21.0).abs() < 1e-9);
    }

    #[test]
    fn cavalier_offset_works() {
        let mut c = Contour::new(true);
        c.verts = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(20.0, 0.0, 0.0),
            Vertex::new(20.0, 20.0, 0.0),
            Vertex::new(0.0, 20.0, 0.0),
        ];
        let off = c.to_cavc().parallel_offset(2.0);
        assert_eq!(off.len(), 1);
        let inner = Contour::from_cavc(&off[0]);
        assert!(approx(inner.signed_area().abs(), 16.0 * 16.0));
    }
}
