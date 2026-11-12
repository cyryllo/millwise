//! Łączenie luźnych odcinków/łuków/polilinii (np. z DXF) w ciągłe kontury.

use crate::contour::{Contour, Vertex};
use crate::math::Vec2;
use std::collections::HashMap;

/// Pojedynczy element do połączenia: otwarta polilinia (może mieć 1 segment).
pub type Segment = Contour;

struct Grid {
    cell: f64,
    map: HashMap<(i64, i64), Vec<(usize, bool)>>,
}

impl Grid {
    fn key(&self, p: Vec2) -> (i64, i64) {
        ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64)
    }
    fn insert(&mut self, p: Vec2, v: (usize, bool)) {
        let k = self.key(p);
        self.map.entry(k).or_default().push(v);
    }
    fn near(&self, p: Vec2) -> impl Iterator<Item = &(usize, bool)> {
        let (kx, ky) = self.key(p);
        (-1..=1)
            .flat_map(move |dx| (-1..=1).map(move |dy| (kx + dx, ky + dy)))
            .filter_map(|k| self.map.get(&k))
            .flatten()
    }
}

/// Łączy otwarte elementy, których końce leżą bliżej niż `tol`.
/// Elementy już zamknięte przechodzą bez zmian. Wynikowe kontury, których
/// początek pokrywa się z końcem, są oznaczane jako zamknięte.
pub fn chain_segments(pieces: Vec<Segment>, tol: f64) -> Vec<Contour> {
    let tol = tol.max(1e-9);
    let mut out = Vec::new();
    let mut open: Vec<Contour> = Vec::new();
    for mut p in pieces {
        cleanup(&mut p, tol);
        if p.verts.len() < 2 {
            continue;
        }
        if p.closed {
            out.push(p);
        } else {
            open.push(p);
        }
    }

    let mut grid = Grid { cell: tol * 2.0, map: HashMap::new() };
    for (i, p) in open.iter().enumerate() {
        grid.insert(p.start().unwrap(), (i, false));
        grid.insert(p.end().unwrap(), (i, true));
    }

    let mut used = vec![false; open.len()];
    for i in 0..open.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut cur = open[i].clone();
        // najpierw w przód, potem (po odwróceniu) w drugą stronę
        for pass in 0..2 {
            loop {
                let end = cur.end().unwrap();
                if pass == 0 && cur.verts.len() > 2 && end.distance(cur.start().unwrap()) <= tol {
                    break; // już zamknięty
                }
                let mut best: Option<(usize, bool, f64)> = None;
                for &(j, is_end) in grid.near(end) {
                    if used[j] {
                        continue;
                    }
                    let q = if is_end { open[j].end().unwrap() } else { open[j].start().unwrap() };
                    let d = q.distance(end);
                    if d <= tol && best.is_none_or(|b| d < b.2) {
                        best = Some((j, is_end, d));
                    }
                }
                let Some((j, is_end, _)) = best else { break };
                used[j] = true;
                let mut next = open[j].clone();
                if is_end {
                    next.reverse();
                }
                append(&mut cur, &next);
            }
            if pass == 0 {
                cur.reverse();
            }
        }
        cur.reverse(); // przywróć pierwotny kierunek pierwszego elementu

        let n = cur.verts.len();
        let closes = cur.start().unwrap().distance(cur.end().unwrap()) <= tol;
        let has_arc = cur.verts.iter().any(|v| v.bulge != 0.0);
        if closes && (n >= 4 || (n >= 3 && has_arc)) {
            cur.verts.pop();
            cur.closed = true;
        }
        out.push(cur);
    }
    out
}

fn append(cur: &mut Contour, next: &Contour) {
    let last = cur.verts.last_mut().unwrap();
    last.bulge = next.verts[0].bulge;
    cur.verts.extend_from_slice(&next.verts[1..]);
}

/// Usuwa zerowe segmenty (kolejne wierzchołki w tym samym miejscu).
fn cleanup(c: &mut Contour, tol: f64) {
    let eps = tol * 0.01;
    let mut v: Vec<Vertex> = Vec::with_capacity(c.verts.len());
    for &x in &c.verts {
        if let Some(prev) = v.last_mut() {
            if prev.p.distance(x.p) <= eps {
                prev.bulge = x.bulge;
                continue;
            }
        }
        v.push(x);
    }
    if c.closed && v.len() > 1 && v[0].p.distance(v[v.len() - 1].p) <= eps {
        v.pop();
    }
    c.verts = v;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> Contour {
        Contour { verts: vec![Vertex::new(a.0, a.1, 0.0), Vertex::new(b.0, b.1, 0.0)], closed: false }
    }

    #[test]
    fn chains_square_with_mixed_directions() {
        let pieces = vec![
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 10.0), (10.0, 0.0)), // odwrócony
            line((0.0, 10.0), (0.0, 0.0)),
            line((10.0, 10.0), (0.0, 10.0005)), // mała szczelina
        ];
        let out = chain_segments(pieces, 0.01);
        assert_eq!(out.len(), 1);
        assert!(out[0].closed);
        assert_eq!(out[0].verts.len(), 4);
        assert!((out[0].signed_area().abs() - 100.0).abs() < 0.01);
    }

    #[test]
    fn keeps_open_paths_open() {
        let out = chain_segments(vec![line((0.0, 0.0), (5.0, 0.0)), line((5.0, 0.0), (5.0, 5.0))], 0.01);
        assert_eq!(out.len(), 1);
        assert!(!out[0].closed);
        assert_eq!(out[0].verts.len(), 3);
    }

    #[test]
    fn chains_arc_slot() {
        // podłużny otwór: dwa odcinki + dwa półokręgi, łuk zapisany odwrotnie
        let arc1 = Contour { verts: vec![Vertex::new(10.0, 0.0, 1.0), Vertex::new(10.0, 10.0, 0.0)], closed: false };
        let mut arc2 = Contour { verts: vec![Vertex::new(0.0, 10.0, 1.0), Vertex::new(0.0, 0.0, 0.0)], closed: false };
        arc2.reverse();
        let out = chain_segments(
            vec![line((0.0, 0.0), (10.0, 0.0)), arc1, line((10.0, 10.0), (0.0, 10.0)), arc2],
            0.01,
        );
        assert_eq!(out.len(), 1);
        assert!(out[0].closed);
        let expected = 100.0 + std::f64::consts::PI * 25.0;
        assert!((out[0].signed_area().abs() - expected).abs() < 1e-6, "{}", out[0].signed_area());
    }
}
