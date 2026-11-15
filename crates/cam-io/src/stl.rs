//! Import STL (binarny i tekstowy ASCII) — siatka trójkątów.

use anyhow::{bail, Context, Result};
use cam_i18n::tr;
use std::path::Path;

/// Siatka trójkątów (wierzchołki w jednostkach pliku).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub tris: Vec<[[f32; 3]; 3]>,
}

impl Mesh {
    /// Najmniejszy i największy punkt.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut it = self.tris.iter().flatten();
        let first = *it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        Some((lo, hi))
    }

    /// Zapis binarny STL (do przechowania modelu w projekcie).
    pub fn to_binary(&self) -> Vec<u8> {
        let mut out = vec![0u8; 80];
        out.extend_from_slice(&(self.tris.len() as u32).to_le_bytes());
        for t in &self.tris {
            out.extend_from_slice(&[0u8; 12]); // normalna — liczona przy wyświetlaniu
            for p in t {
                for c in p {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            out.extend_from_slice(&[0u8; 2]);
        }
        out
    }
}

pub fn load_stl(path: &Path) -> Result<Mesh> {
    let bytes = std::fs::read(path).with_context(|| tr!("Nie można wczytać pliku {}", path.display()))?;
    parse_stl(&bytes)
}

/// Rozpoznaje format: binarny, gdy rozmiar zgadza się z liczbą trójkątów z nagłówka.
pub fn parse_stl(bytes: &[u8]) -> Result<Mesh> {
    if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if bytes.len() == 84 + n * 50 {
            return parse_binary(bytes, n);
        }
    }
    let text = String::from_utf8_lossy(bytes);
    if text.trim_start().starts_with("solid") && text.contains("facet") {
        return parse_ascii(&text);
    }
    bail!(tr!("To nie jest plik STL (ani binarny, ani tekstowy)"))
}

fn parse_binary(bytes: &[u8], n: usize) -> Result<Mesh> {
    let f = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let mut tris = Vec::with_capacity(n);
    for i in 0..n {
        let base = 84 + i * 50 + 12; // pomijamy normalną
        let mut t = [[0f32; 3]; 3];
        for (v, tv) in t.iter_mut().enumerate() {
            for (c, tc) in tv.iter_mut().enumerate() {
                *tc = f(base + v * 12 + c * 4);
            }
        }
        if t.iter().flatten().all(|c| c.is_finite()) {
            tris.push(t);
        }
    }
    if tris.is_empty() {
        bail!(tr!("Plik STL nie zawiera trójkątów"));
    }
    Ok(Mesh { tris })
}

fn parse_ascii(text: &str) -> Result<Mesh> {
    let mut tris = Vec::new();
    let mut cur: Vec<[f32; 3]> = Vec::with_capacity(3);
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("vertex") => {
                let mut p = [0f32; 3];
                for c in p.iter_mut() {
                    *c = w.next().and_then(|s| s.parse().ok()).context(tr!("Błędny wierzchołek w pliku STL"))?;
                }
                cur.push(p);
            }
            Some("endfacet") => {
                if cur.len() == 3 {
                    tris.push([cur[0], cur[1], cur[2]]);
                }
                cur.clear();
            }
            _ => {}
        }
    }
    if tris.is_empty() {
        bail!(tr!("Plik STL nie zawiera trójkątów"));
    }
    Ok(Mesh { tris })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sześcian 10 mm: 12 trójkątów.
    fn cube() -> Mesh {
        let p = |x: f32, y: f32, z: f32| [x * 10.0, y * 10.0, z * 10.0];
        let quads = [
            [p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)],
            [p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)],
            [p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)],
            [p(0., 1., 0.), p(1., 1., 0.), p(1., 1., 1.), p(0., 1., 1.)],
            [p(0., 0., 0.), p(0., 1., 0.), p(0., 1., 1.), p(0., 0., 1.)],
            [p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.), p(1., 0., 1.)],
        ];
        Mesh { tris: quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect() }
    }

    #[test]
    fn binary_round_trip() {
        let m = cube();
        let back = parse_stl(&m.to_binary()).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.bounds(), Some(([0.0; 3], [10.0; 3])));
    }

    #[test]
    fn ascii_file() {
        let mut s = String::from("solid kostka\n");
        for t in cube().tris {
            s.push_str("  facet normal 0 0 0\n    outer loop\n");
            for v in t {
                s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str("endsolid kostka\n");
        let m = parse_stl(s.as_bytes()).unwrap();
        assert_eq!(m.tris.len(), 12);
        assert_eq!(m, cube());
    }

    #[test]
    fn not_stl() {
        assert!(parse_stl(b"to nie jest model").is_err());
    }
}
