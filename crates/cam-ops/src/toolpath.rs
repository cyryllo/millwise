//! Ścieżka narzędzia: ciąg ruchów szybkich, liniowych i łukowych (także helikalnych).

use cam_geom::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct P3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl P3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
    pub fn from_xy(p: Vec2, z: f64) -> Self {
        Self::new(p.x, p.y, z)
    }
    pub fn distance(self, o: P3) -> f64 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2) + (self.z - o.z).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// G0
    Rapid(P3),
    /// G1
    Feed { to: P3, feed: f64 },
    /// G2 (ccw=false) / G3 (ccw=true); środek w XY, Z może się zmieniać (helisa).
    Arc { to: P3, center: Vec2, ccw: bool, feed: f64 },
}

impl Move {
    pub fn end(&self) -> P3 {
        match *self {
            Move::Rapid(p) => p,
            Move::Feed { to, .. } | Move::Arc { to, .. } => to,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Toolpath {
    pub moves: Vec<Move>,
}

impl Toolpath {
    pub fn pos(&self) -> Option<P3> {
        self.moves.last().map(|m| m.end())
    }

    pub fn rapid(&mut self, p: P3) {
        if self.pos().is_some_and(|c| c.distance(p) < 1e-9) {
            return;
        }
        self.moves.push(Move::Rapid(p));
    }

    pub fn feed(&mut self, p: P3, feed: f64) {
        if self.pos().is_some_and(|c| c.distance(p) < 1e-9) {
            return;
        }
        self.moves.push(Move::Feed { to: p, feed });
    }

    pub fn arc(&mut self, to: P3, center: Vec2, ccw: bool, feed: f64) {
        self.moves.push(Move::Arc { to, center, ccw, feed });
    }

    /// Bezpieczne przejście: podniesienie do `safe_z`, przejazd nad `p`, opuszczenie do `p.z` szybkim ruchem.
    pub fn travel_to(&mut self, p: P3, safe_z: f64) {
        match self.pos() {
            Some(c) if c.xy().distance(p.xy()) < 1e-9 => {
                if c.z < p.z {
                    self.rapid(P3 { z: p.z, ..c });
                }
            }
            Some(c) => {
                self.rapid(P3 { z: safe_z.max(c.z), ..c });
                self.rapid(P3::from_xy(p.xy(), safe_z.max(c.z)));
                self.rapid(p);
            }
            None => {
                self.rapid(P3::from_xy(p.xy(), safe_z));
                self.rapid(p);
            }
        }
    }

    pub fn retract(&mut self, safe_z: f64) {
        if let Some(c) = self.pos() {
            if c.z < safe_z {
                self.rapid(P3 { z: safe_z, ..c });
            }
        }
    }

    pub fn append(&mut self, other: Toolpath) {
        self.moves.extend(other.moves);
    }

    /// Szacowany czas [min] przy danej prędkości ruchów szybkich [mm/min].
    pub fn estimate_minutes(&self, rapid_rate: f64) -> f64 {
        let mut t = 0.0;
        let mut prev: Option<P3> = None;
        for m in &self.moves {
            let end = m.end();
            if let Some(p) = prev {
                let len = match *m {
                    Move::Arc { center, ccw, .. } => arc_length(p, end, center, ccw),
                    _ => p.distance(end),
                };
                let rate = match *m {
                    Move::Rapid(_) => rapid_rate,
                    Move::Feed { feed, .. } | Move::Arc { feed, .. } => feed,
                };
                t += len / rate.max(1.0);
            }
            prev = Some(end);
        }
        t
    }

    /// Spłaszczenie z posuwem: (punkt, ruch szybki, posuw [mm/min] ruchu kończącego się w punkcie).
    pub fn polyline_with_feed(&self, tol: f64) -> Vec<(P3, bool, f64)> {
        let mut out = Vec::with_capacity(self.moves.len() * 2);
        let mut prev: Option<P3> = None;
        for m in &self.moves {
            match *m {
                Move::Rapid(p) => out.push((p, true, 0.0)),
                Move::Feed { to, feed } => out.push((to, false, feed)),
                Move::Arc { to, center, ccw, feed } => match prev {
                    Some(s) => out.extend(arc_points(s, to, center, ccw, tol).into_iter().map(|p| (p, false, feed))),
                    None => out.push((to, false, feed)),
                },
            }
            prev = Some(m.end());
        }
        out
    }

    /// Spłaszczenie do punktów (łuki aproksymowane) — do wyświetlania i symulacji.
    /// Zwraca listę (punkt, czy ruch szybki).
    pub fn polyline(&self, tol: f64) -> Vec<(P3, bool)> {
        let mut out: Vec<(P3, bool)> = Vec::with_capacity(self.moves.len() * 2);
        let mut prev: Option<P3> = None;
        for m in &self.moves {
            match *m {
                Move::Rapid(p) => out.push((p, true)),
                Move::Feed { to, .. } => out.push((to, false)),
                Move::Arc { to, center, ccw, .. } => {
                    if let Some(s) = prev {
                        for p in arc_points(s, to, center, ccw, tol) {
                            out.push((p, false));
                        }
                    } else {
                        out.push((to, false));
                    }
                }
            }
            prev = Some(m.end());
        }
        out
    }
}

/// Kąt łuku (dodatni) od `s` do `e` wokół `c` w danym kierunku. Pełny okrąg gdy s == e.
pub fn arc_sweep(s: P3, e: P3, c: Vec2, ccw: bool) -> f64 {
    let a0 = (s.xy() - c).angle();
    let a1 = (e.xy() - c).angle();
    let tau = std::f64::consts::TAU;
    let mut d = if ccw { a1 - a0 } else { a0 - a1 };
    while d <= 1e-9 {
        d += tau;
    }
    while d > tau + 1e-9 {
        d -= tau;
    }
    d
}

pub fn arc_length(s: P3, e: P3, c: Vec2, ccw: bool) -> f64 {
    let r = s.xy().distance(c);
    let horiz = r * arc_sweep(s, e, c, ccw);
    (horiz * horiz + (e.z - s.z).powi(2)).sqrt()
}

/// Punkty łuku bez punktu startowego (z końcowym).
pub fn arc_points(s: P3, e: P3, c: Vec2, ccw: bool, tol: f64) -> Vec<P3> {
    let r = s.xy().distance(c);
    let sweep = arc_sweep(s, e, c, ccw);
    let n = cam_geom::contour::arc_steps(r.max(1e-6), sweep, tol);
    let a0 = (s.xy() - c).angle();
    let sign = if ccw { 1.0 } else { -1.0 };
    (1..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            if i == n {
                return e;
            }
            let a = a0 + sign * sweep * t;
            P3::new(c.x + r * a.cos(), c.y + r * a.sin(), s.z + (e.z - s.z) * t)
        })
        .collect()
}
