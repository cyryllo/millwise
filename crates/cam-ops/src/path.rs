//! Przejście narzędzia po konturze 2D z zadanym profilem Z: rampy, mostki
//! (tabs), łuki helikalne. Wspólne dla profilu, kieszeni i graweru.

use crate::toolpath::{Toolpath, P3};
use cam_geom::contour::ArcGeom;
use cam_geom::{Contour, Vec2};

#[derive(Debug, Clone, Copy)]
pub enum Seg {
    Line { a: Vec2, b: Vec2 },
    Arc { a: Vec2, b: Vec2, c: Vec2, r: f64, a0: f64, sweep: f64 },
}

impl Seg {
    pub fn len(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => a.distance(b),
            Seg::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }

    pub fn start(&self) -> Vec2 {
        match *self {
            Seg::Line { a, .. } | Seg::Arc { a, .. } => a,
        }
    }

    pub fn end(&self) -> Vec2 {
        match *self {
            Seg::Line { b, .. } | Seg::Arc { b, .. } => b,
        }
    }

    pub fn point_at(&self, s: f64) -> Vec2 {
        let l = self.len();
        let t = if l > 0.0 { (s / l).clamp(0.0, 1.0) } else { 0.0 };
        match *self {
            Seg::Line { a, b } => a.lerp(b, t),
            Seg::Arc { c, r, a0, sweep, .. } => c + Vec2::from_angle(a0 + sweep * t) * r,
        }
    }

    /// Fragment segmentu od długości `s0` do `s1`.
    pub fn sub(&self, s0: f64, s1: f64) -> Seg {
        let l = self.len();
        if l <= 0.0 {
            return *self;
        }
        match *self {
            Seg::Line { .. } => Seg::Line { a: self.point_at(s0), b: self.point_at(s1) },
            Seg::Arc { c, r, a0, sweep, .. } => {
                let t0 = s0 / l;
                let t1 = s1 / l;
                Seg::Arc {
                    a: self.point_at(s0),
                    b: self.point_at(s1),
                    c,
                    r,
                    a0: a0 + sweep * t0,
                    sweep: sweep * (t1 - t0),
                }
            }
        }
    }
}

pub fn segments(c: &Contour) -> Vec<Seg> {
    (0..c.segment_count())
        .filter_map(|i| {
            let (a, b, bulge) = c.segment(i);
            if a.distance(b) < 1e-9 {
                return None;
            }
            Some(if bulge.abs() < 1e-9 {
                Seg::Line { a, b }
            } else {
                let g = ArcGeom::from_bulge(a, b, bulge);
                Seg::Arc { a, b, c: g.center, r: g.radius, a0: g.start_angle, sweep: g.sweep }
            })
        })
        .collect()
}

/// Kontur przygotowany do przejść: segmenty + skumulowane długości.
pub struct PathWalk {
    pub segs: Vec<Seg>,
    cum: Vec<f64>,
    pub total: f64,
    pub closed: bool,
}

/// Parametry jednego przejścia po konturze.
pub struct WalkSpec<'a> {
    /// Pozycja startowa (długość od początku konturu).
    pub s_start: f64,
    /// Długość do przejechania (może być > długości konturu dla zamkniętych).
    pub travel: f64,
    /// Z jako funkcja przejechanej drogi.
    pub z_of: &'a dyn Fn(f64) -> f64,
    /// Przedziały mostków (w długości od początku konturu) i Z ich wierzchu.
    pub tabs: &'a [(f64, f64)],
    pub tab_z: f64,
    pub feed: f64,
    pub plunge_feed: f64,
    /// Droga, na której kończy się rampa (punkt załamania `z_of`); 0 = brak rampy.
    pub ramp_end: f64,
    /// Kierunek odwrotny (tylko dla otwartych konturów: od końca do początku).
    pub reverse: bool,
}

impl PathWalk {
    pub fn new(c: &Contour) -> Self {
        let segs = segments(c);
        let mut cum = Vec::with_capacity(segs.len() + 1);
        let mut acc = 0.0;
        cum.push(0.0);
        for s in &segs {
            acc += s.len();
            cum.push(acc);
        }
        PathWalk { segs, cum, total: acc, closed: c.closed }
    }

    pub fn is_empty(&self) -> bool {
        self.segs.is_empty() || self.total <= 1e-9
    }

    pub fn point_at(&self, s: f64) -> Vec2 {
        let s = if self.closed { s.rem_euclid(self.total) } else { s.clamp(0.0, self.total) };
        let i = self.seg_index(s);
        self.segs[i].point_at(s - self.cum[i])
    }

    fn seg_index(&self, s: f64) -> usize {
        match self.cum.binary_search_by(|v| v.partial_cmp(&s).unwrap()) {
            Ok(i) => i.min(self.segs.len() - 1),
            Err(i) => (i - 1).min(self.segs.len() - 1),
        }
    }

    /// Generuje ruchy. Narzędzie powinno już stać nad punktem startowym
    /// (dowolne Z) — pierwszy ruch to zejście do `z_of(0)`.
    pub fn emit(&self, tp: &mut Toolpath, spec: &WalkSpec) {
        if self.is_empty() {
            return;
        }
        let eps = 1e-7;
        let in_tab = |s: f64| spec.tabs.iter().any(|&(a, b)| s > a + eps && s < b - eps);
        let clamp_z = |z: f64, s_mid: f64| if in_tab(s_mid) { z.max(spec.tab_z) } else { z };

        // lista kawałków: (segment, droga na początku, droga na końcu, środek w s)
        let mut pieces: Vec<(Seg, f64, f64, f64)> = Vec::new();
        if spec.reverse {
            // otwarty kontur od końca: odwracamy segmenty
            let mut traveled = 0.0;
            for seg in self.segs.iter().rev() {
                let l = seg.len();
                let rev = match *seg {
                    Seg::Line { a, b } => Seg::Line { a: b, b: a },
                    Seg::Arc { a, b, c, r, a0, sweep } => Seg::Arc { a: b, b: a, c, r, a0: a0 + sweep, sweep: -sweep },
                };
                // mostki dla otwartych konturów nie są obsługiwane — dzielenie niepotrzebne
                let s_mid = self.total - traveled - l * 0.5;
                pieces.push((rev, traveled, traveled + l, s_mid));
                traveled += l;
            }
        } else {
            let mut breaks: Vec<f64> = self.cum.clone();
            for &(a, b) in spec.tabs {
                breaks.push(a);
                breaks.push(b);
            }
            breaks.sort_by(|a, b| a.partial_cmp(b).unwrap());
            breaks.dedup_by(|a, b| (*a - *b).abs() < eps);

            let mut s = if self.closed { spec.s_start.rem_euclid(self.total) } else { spec.s_start };
            let mut traveled = 0.0;
            let mut guard = 0;
            while traveled < spec.travel - eps && guard < 1_000_000 {
                guard += 1;
                if s >= self.total - eps {
                    if !self.closed {
                        break;
                    }
                    s = 0.0;
                }
                let next_break = breaks.iter().copied().find(|&b| b > s + eps).unwrap_or(self.total);
                let mut s_end = next_break.min(s + (spec.travel - traveled)).min(self.total);
                if traveled < spec.ramp_end - eps {
                    s_end = s_end.min(s + (spec.ramp_end - traveled));
                }
                let i = self.seg_index(s + eps.min((s_end - s) * 0.5));
                let piece = self.segs[i].sub(s - self.cum[i], s_end - self.cum[i]);
                let l = s_end - s;
                pieces.push((piece, traveled, traveled + l, (s + s_end) * 0.5));
                traveled += l;
                s = s_end;
            }
        }

        for (seg, t0, t1, s_mid) in pieces {
            let z0 = clamp_z((spec.z_of)(t0), s_mid);
            let z1 = clamp_z((spec.z_of)(t1), s_mid);
            let start = seg.start();
            let cur = tp.pos();
            match cur {
                Some(c) if c.xy().distance(start) < 1e-6 => {
                    if (c.z - z0).abs() > 1e-6 {
                        let f = if z0 < c.z { spec.plunge_feed } else { spec.feed };
                        tp.feed(P3::from_xy(start, z0), f);
                    }
                }
                _ => tp.feed(P3::from_xy(start, z0), spec.plunge_feed),
            }
            let end = P3::from_xy(seg.end(), z1);
            let f = if z1 < z0 - 1e-9 && (seg.len() / (z0 - z1)) < 1.0 { spec.plunge_feed } else { spec.feed };
            match seg {
                Seg::Line { .. } => tp.feed(end, f),
                Seg::Arc { c, sweep, .. } => tp.arc(end, c, sweep > 0.0, f),
            }
        }
    }
}

/// Poziomy Z od `top` do `bottom` w równych krokach nie większych niż `step`.
pub fn z_levels(top: f64, bottom: f64, step: f64) -> Vec<f64> {
    let depth = top - bottom;
    if depth <= 1e-9 {
        return vec![bottom];
    }
    let n = (depth / step.max(1e-3) - 1e-9).ceil().max(1.0) as usize;
    (1..=n).map(|i| top - depth * i as f64 / n as f64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::Move;
    use cam_geom::Vertex;

    fn square() -> Contour {
        Contour {
            verts: vec![
                Vertex::new(0.0, 0.0, 0.0),
                Vertex::new(10.0, 0.0, 0.0),
                Vertex::new(10.0, 10.0, 0.0),
                Vertex::new(0.0, 10.0, 0.0),
            ],
            closed: true,
        }
    }

    #[test]
    fn levels() {
        assert_eq!(z_levels(0.0, -6.0, 2.0), vec![-2.0, -4.0, -6.0]);
        assert_eq!(z_levels(0.0, -5.0, 2.0).len(), 3);
    }

    #[test]
    fn ramp_then_full_lap() {
        let w = PathWalk::new(&square());
        let mut tp = Toolpath::default();
        tp.rapid(P3::new(0.0, 0.0, 1.0));
        let ramp = 15.0;
        let z_of = |t: f64| -2.0 * (t / ramp).min(1.0);
        w.emit(
            &mut tp,
            &WalkSpec { s_start: 0.0, travel: ramp + w.total, z_of: &z_of, tabs: &[], tab_z: 0.0, feed: 1000.0, plunge_feed: 200.0, ramp_end: ramp, reverse: false },
        );
        let end = tp.pos().unwrap();
        // po rampie 15 mm + pełne okrążenie kończymy w s=15 → (10, 5)
        assert!((end.x - 10.0).abs() < 1e-9 && (end.y - 5.0).abs() < 1e-9 && (end.z + 2.0).abs() < 1e-9);
        // pierwszy ruch: zejście na Z=0, potem rampa
        assert!(matches!(tp.moves[1], Move::Feed { to, .. } if to.z == 0.0));
        // rampa kończy się dokładnie po 15 mm: punkt (10, 5, -2)
        assert!(tp.moves.iter().any(|m| {
            let p = m.end();
            (p.x - 10.0).abs() < 1e-9 && (p.y - 5.0).abs() < 1e-9 && (p.z + 2.0).abs() < 1e-9
        }));
        // przed końcem rampy Z nigdy nie spada poniżej linii rampy
        let mut traveled = 0.0;
        let mut prev = tp.moves[1].end();
        for m in &tp.moves[2..] {
            let e = m.end();
            traveled += prev.xy().distance(e.xy());
            assert!(e.z >= z_of(traveled) - 1e-9, "z={} przy drodze {}", e.z, traveled);
            prev = e;
        }
    }

    #[test]
    fn tabs_raise_tool() {
        let w = PathWalk::new(&square());
        let mut tp = Toolpath::default();
        tp.rapid(P3::new(0.0, 0.0, -3.0));
        let z_of = |_t: f64| -3.0;
        w.emit(
            &mut tp,
            &WalkSpec { s_start: 0.0, travel: w.total, z_of: &z_of, tabs: &[(4.0, 6.0)], tab_z: -1.0, feed: 1000.0, plunge_feed: 200.0, ramp_end: 0.0, reverse: false },
        );
        let zs: Vec<f64> = tp.moves.iter().map(|m| m.end().z).collect();
        assert!(zs.contains(&-1.0));
        assert_eq!(*zs.last().unwrap(), -3.0);
    }
}
