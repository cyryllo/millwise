//! Napisy: tekst zamieniany na krzywe (zarysy liter) z czcionki TrueType.

use cam_geom::{Contour, Vec2, Vertex};
use std::path::PathBuf;

/// Czcionka napisu.
#[derive(Clone, PartialEq, Debug)]
pub enum FontChoice {
    /// Ubuntu Light (wbudowana; licencja Ubuntu Font Licence).
    Ubuntu,
    /// Hack — o stałej szerokości znaków (wbudowana; licencja MIT / Bitstream Vera).
    Hack,
    /// Plik .ttf / .otf z dysku.
    File(PathBuf),
}

impl FontChoice {
    pub fn label(&self) -> String {
        match self {
            FontChoice::Ubuntu => "Ubuntu".into(),
            FontChoice::Hack => "Hack (stała szerokość)".into(),
            FontChoice::File(p) => p.file_name().map_or("plik".into(), |n| n.to_string_lossy().into_owned()),
        }
    }

    pub fn bytes(&self) -> Result<std::borrow::Cow<'static, [u8]>, String> {
        Ok(match self {
            FontChoice::Ubuntu => epaint_default_fonts::UBUNTU_LIGHT.into(),
            FontChoice::Hack => epaint_default_fonts::HACK_REGULAR.into(),
            FontChoice::File(p) => std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?.into(),
        })
    }
}

/// Zbiera zarys litery jako łamane (krzywe Béziera dzielone na odcinki).
struct Outline {
    scale: f64,
    origin: Vec2,
    /// Liczba odcinków na krzywą.
    steps: usize,
    cur: Vec<Vec2>,
    out: Vec<Vec<Vec2>>,
}

impl Outline {
    fn pt(&self, x: f32, y: f32) -> Vec2 {
        self.origin + Vec2::new(x as f64 * self.scale, y as f64 * self.scale)
    }
    fn last(&self) -> Vec2 {
        *self.cur.last().unwrap_or(&self.origin)
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.cur.len() > 2 {
            self.out.push(std::mem::take(&mut self.cur));
        }
        self.cur = vec![self.pt(x, y)];
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cur.push(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.last(), self.pt(x1, y1), self.pt(x, y));
        for k in 1..=self.steps {
            let t = k as f64 / self.steps as f64;
            let u = 1.0 - t;
            self.cur.push(p0 * (u * u) + c * (2.0 * u * t) + p1 * (t * t));
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, c1, c2, p1) = (self.last(), self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        for k in 1..=self.steps {
            let t = k as f64 / self.steps as f64;
            let u = 1.0 - t;
            self.cur.push(p0 * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + p1 * (t * t * t));
        }
    }
    fn close(&mut self) {
        if self.cur.len() > 2 {
            self.out.push(std::mem::take(&mut self.cur));
        }
        self.cur.clear();
    }
}

/// Zarysy liter napisu jako zamknięte kontury. Początek (0, 0) to lewy koniec
/// linii bazowej pierwszego wiersza; `height` — wysokość wielkich liter [mm];
/// `spacing` — dodatkowy odstęp między znakami [mm]; kolejne wiersze niżej.
pub fn text_contours(font: &[u8], text: &str, height: f64, spacing: f64) -> Result<Vec<Contour>, String> {
    let face = ttf_parser::Face::parse(font, 0).map_err(|e| format!("Nie można odczytać czcionki: {e}"))?;
    let cap = face.capital_height().filter(|h| *h > 0).map_or(face.ascender() as f64 * 0.7, |h| h as f64);
    let scale = height.max(0.01) / cap;
    let line_h = (face.height() as f64 + face.line_gap() as f64) * scale;
    // gładkość łuków: krótsze odcinki dla większych liter
    let steps = ((height / 2.0).ceil() as usize).clamp(4, 24);
    let mut out = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let mut x = 0.0;
        let y = -(row as f64) * line_h;
        for ch in line.chars() {
            let Some(gid) = face.glyph_index(ch) else {
                x += height * 0.5 + spacing;
                continue;
            };
            let mut o = Outline { scale, origin: Vec2::new(x, y), steps, cur: Vec::new(), out: Vec::new() };
            face.outline_glyph(gid, &mut o);
            if o.cur.len() > 2 {
                let rest = std::mem::take(&mut o.cur);
                o.out.push(rest);
            }
            for mut pts in o.out {
                if pts.len() > 2 && pts.first().unwrap().distance(*pts.last().unwrap()) < 1e-9 {
                    pts.pop();
                }
                pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
                if pts.len() < 3 {
                    continue;
                }
                let mut c = Contour::new(true);
                c.verts = pts.into_iter().map(|p| Vertex::new(p.x, p.y, 0.0)).collect();
                out.push(c);
            }
            x += face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale + spacing;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capital_letters_have_the_set_height() {
        for font in [FontChoice::Ubuntu, FontChoice::Hack] {
            let bytes = font.bytes().unwrap();
            let cs = text_contours(&bytes, "H", 20.0, 0.0).unwrap();
            assert_eq!(cs.len(), 1, "{font:?}: H to jeden zarys");
            let bb = cs[0].bbox();
            assert!((bb.size().y - 20.0).abs() < 0.2, "{font:?}: wysokość {}", bb.size().y);
            assert!(bb.min.y.abs() < 0.2, "na linii bazowej");
        }
    }

    #[test]
    fn letters_with_holes_and_polish_characters() {
        let bytes = FontChoice::Ubuntu.bytes().unwrap();
        // O i A mają otwory (2 zarysy każda), „ł” i „ę” są w czcionce
        let o = text_contours(&bytes, "O", 10.0, 0.0).unwrap();
        assert_eq!(o.len(), 2);
        let pl = text_contours(&bytes, "łę", 10.0, 0.0).unwrap();
        assert!(pl.len() >= 3, "{}", pl.len());
        // dwa wiersze: drugi niżej
        let two = text_contours(&bytes, "I\nI", 10.0, 0.0).unwrap();
        assert_eq!(two.len(), 2);
        assert!(two[1].bbox().max.y < two[0].bbox().min.y);
        // odstęp między znakami wydłuża napis
        let w0 = text_contours(&bytes, "II", 10.0, 0.0).unwrap()[1].bbox().min.x;
        let w1 = text_contours(&bytes, "II", 10.0, 2.0).unwrap()[1].bbox().min.x;
        assert!((w1 - w0 - 2.0).abs() < 1e-6);
    }
}
