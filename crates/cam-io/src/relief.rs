//! Płaskorzeźba ze zdjęcia: obraz (JPG, PNG, BMP) jako mapa wysokości —
//! jasne miejsca wyżej, ciemne niżej (albo odwrotnie). Wynik to zamknięta
//! bryła z trójkątów (góra z obrazu, boki i spód), obrabiana jak model STL.

use crate::stl::Mesh;
use anyhow::{bail, Context, Result};
use cam_i18n::tr;
use std::path::Path;

/// Najdłuższy bok zapamiętanego obrazu [px] — więcej nie ma sensu przy frezowaniu,
/// a projekt (który przechowuje obraz) zostaje mały.
pub const MAX_IMAGE_SIDE: u32 = 1600;
/// Najwięcej punktów siatki na dłuższym boku.
pub const MAX_GRID_SIDE: f64 = 1000.0;

/// Obraz w skali szarości; wiersz 0 to góra obrazu.
#[derive(Clone, Debug, PartialEq)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GrayImage {
    fn at(&self, x: u32, y: u32) -> f32 {
        self.pixels[(y * self.width + x) as usize] as f32 / 255.0
    }

    /// Jasność 0–1 w punkcie (x, y) w pikselach (środki pikseli w liczbach całkowitych).
    fn sample(&self, x: f64, y: f64) -> f32 {
        let x = x.clamp(0.0, (self.width - 1) as f64);
        let y = y.clamp(0.0, (self.height - 1) as f64);
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let top = self.at(x0, y0) * (1.0 - fx) + self.at(x1, y0) * fx;
        let bottom = self.at(x0, y1) * (1.0 - fx) + self.at(x1, y1) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

pub fn is_image_path(p: &Path) -> bool {
    p.extension().is_some_and(|e| {
        let e = e.to_string_lossy().to_ascii_lowercase();
        matches!(e.as_str(), "png" | "jpg" | "jpeg" | "bmp")
    })
}

pub fn load_image(path: &Path) -> Result<GrayImage> {
    let bytes = std::fs::read(path).with_context(|| tr!("Nie można wczytać pliku {}", path.display()))?;
    decode_image(&bytes)
}

/// Dekoduje obraz, zmniejsza do `MAX_IMAGE_SIDE` i zamienia na skalę szarości.
/// Przezroczyste miejsca są ciemne (najniżej).
pub fn decode_image(bytes: &[u8]) -> Result<GrayImage> {
    let img = image::load_from_memory(bytes).map_err(|e| anyhow::anyhow!(tr!("Nie można odczytać obrazu: {}", e)))?;
    let img = if img.width().max(img.height()) > MAX_IMAGE_SIDE {
        img.resize(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width < 2 || height < 2 {
        bail!(tr!("Obraz jest za mały"));
    }
    let pixels = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
            (luma * a as f32 / 255.0).round() as u8
        })
        .collect();
    Ok(GrayImage { width, height, pixels })
}

/// Ustawienia płaskorzeźby [mm].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReliefParams {
    /// Szerokość (X); wysokość wynika z proporcji obrazu.
    pub width: f64,
    /// Różnica wysokości między najciemniejszym a najjaśniejszym miejscem.
    pub depth: f64,
    /// Podstawa pod najniższym miejscem.
    pub base: f64,
    /// Wygładzanie (rozmycie) — usuwa szum i drobne ziarno zdjęcia.
    pub smooth: f64,
    /// Ciemne wyżej, jasne niżej.
    pub invert: bool,
    /// Odstęp punktów siatki.
    pub cell: f64,
}

impl Default for ReliefParams {
    fn default() -> Self {
        ReliefParams { width: 100.0, depth: 5.0, base: 2.0, smooth: 0.3, invert: false, cell: 0.25 }
    }
}

impl ReliefParams {
    /// Ustawienia dla nowego obrazu: szerokość 100 mm, siatka ok. 400 punktów na dłuższym boku.
    pub fn for_image(img: &GrayImage) -> Self {
        let mut p = ReliefParams::default();
        let long = p.width.max(p.height(img));
        p.cell = round_nice(long / 400.0);
        p
    }

    /// Wysokość (Y) z proporcji obrazu.
    pub fn height(&self, img: &GrayImage) -> f64 {
        self.width * img.height as f64 / img.width as f64
    }

    /// Odstęp punktów siatki po ograniczeniu liczby punktów.
    pub fn effective_cell(&self, img: &GrayImage) -> f64 {
        let long = self.width.max(self.height(img));
        self.cell.max(long / MAX_GRID_SIDE).max(0.01)
    }
}

/// 0,1 / 0,2 / 0,25 / 0,5 · 10^n — czytelne wartości.
fn round_nice(v: f64) -> f64 {
    let base = 10f64.powf(v.max(1e-3).log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if base * m >= v * 0.999 {
            return base * m;
        }
    }
    base * 10.0
}

/// Siatka wysokości płaskorzeźby: (punkty w X, punkty w Y, odstępy dx, dy, wysokości
/// wierszami od dołu, Z od 0 = spód podstawy).
pub fn relief_grid(img: &GrayImage, p: &ReliefParams) -> (usize, usize, f64, f64, Vec<f32>) {
    let (w, h) = (p.width.max(0.1), p.height(img).max(0.1));
    let cell = p.effective_cell(img);
    let nx = ((w / cell).round() as usize).max(2) + 1;
    let ny = ((h / cell).round() as usize).max(2) + 1;
    let (dx, dy) = (w / (nx - 1) as f64, h / (ny - 1) as f64);
    let (iw, ih) = ((img.width - 1) as f64, (img.height - 1) as f64);
    let mut v = vec![0f32; nx * ny];
    for j in 0..ny {
        // j = 0 to dół płaskorzeźby, czyli dolny wiersz obrazu
        let py = (1.0 - j as f64 / (ny - 1) as f64) * ih;
        for i in 0..nx {
            let px = i as f64 / (nx - 1) as f64 * iw;
            v[j * nx + i] = img.sample(px, py);
        }
    }
    let sigma = p.smooth / dx.min(dy);
    if sigma > 0.05 {
        blur(&mut v, nx, ny, sigma);
    }
    let (depth, base) = (p.depth.max(0.0) as f32, p.base.max(0.0) as f32);
    for z in v.iter_mut() {
        let t = if p.invert { 1.0 - *z } else { *z };
        *z = base + depth * t;
    }
    (nx, ny, dx, dy, v)
}

/// Rozmycie Gaussa (osobno w X i w Y), brzegi powielone.
fn blur(v: &mut [f32], nx: usize, ny: usize, sigma: f64) {
    let r = (sigma * 3.0).ceil().min(200.0) as isize;
    let mut k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp() as f32).collect();
    let sum: f32 = k.iter().sum();
    k.iter_mut().for_each(|x| *x /= sum);
    let mut tmp = vec![0f32; v.len()];
    for j in 0..ny {
        for i in 0..nx {
            let mut s = 0.0;
            for (o, &kw) in k.iter().enumerate() {
                let ii = (i as isize + o as isize - r).clamp(0, nx as isize - 1) as usize;
                s += v[j * nx + ii] * kw;
            }
            tmp[j * nx + i] = s;
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let mut s = 0.0;
            for (o, &kw) in k.iter().enumerate() {
                let jj = (j as isize + o as isize - r).clamp(0, ny as isize - 1) as usize;
                s += tmp[jj * nx + i] * kw;
            }
            v[j * nx + i] = s;
        }
    }
}

/// Zamknięta bryła płaskorzeźby [mm]: lewy dolny róg spodu w (0, 0, 0).
pub fn relief_mesh(img: &GrayImage, p: &ReliefParams) -> Mesh {
    let (nx, ny, dx, dy, z) = relief_grid(img, p);
    let (w, h) = ((nx - 1) as f64 * dx, (ny - 1) as f64 * dy);
    let pt = |i: usize, j: usize| [(i as f64 * dx) as f32, (j as f64 * dy) as f32, z[j * nx + i]];
    let bot = |i: usize, j: usize| [(i as f64 * dx) as f32, (j as f64 * dy) as f32, 0.0];
    let mut tris = Vec::with_capacity((nx - 1) * (ny - 1) * 2 + 4 * (nx + ny) + 2);
    // góra (normalne w górę)
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    // boki (normalne na zewnątrz)
    for i in 0..nx - 1 {
        let (b0, b1, t0, t1) = (bot(i, 0), bot(i + 1, 0), pt(i, 0), pt(i + 1, 0));
        tris.push([b0, b1, t1]);
        tris.push([b0, t1, t0]);
        let j = ny - 1;
        let (b0, b1, t0, t1) = (bot(i, j), bot(i + 1, j), pt(i, j), pt(i + 1, j));
        tris.push([b1, b0, t0]);
        tris.push([b1, t0, t1]);
    }
    for j in 0..ny - 1 {
        let (b0, b1, t0, t1) = (bot(0, j), bot(0, j + 1), pt(0, j), pt(0, j + 1));
        tris.push([b1, b0, t0]);
        tris.push([b1, t0, t1]);
        let i = nx - 1;
        let (b0, b1, t0, t1) = (bot(i, j), bot(i, j + 1), pt(i, j), pt(i, j + 1));
        tris.push([b0, b1, t1]);
        tris.push([b0, t1, t0]);
    }
    // spód (normalna w dół)
    let (w, h) = (w as f32, h as f32);
    tris.push([[0.0, 0.0, 0.0], [0.0, h, 0.0], [w, h, 0.0]]);
    tris.push([[0.0, 0.0, 0.0], [w, h, 0.0], [w, 0.0, 0.0]]);
    Mesh { tris }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Obraz 4 × 2: lewa połowa czarna, prawa biała.
    fn halves() -> GrayImage {
        GrayImage { width: 4, height: 2, pixels: vec![0, 0, 255, 255, 0, 0, 255, 255] }
    }

    fn normal(t: &[[f32; 3]; 3]) -> [f32; 3] {
        let (a, b, c) = (t[0], t[1], t[2]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
    }

    #[test]
    fn heights_follow_brightness() {
        let img = halves();
        let p = ReliefParams { width: 40.0, depth: 5.0, base: 2.0, smooth: 0.0, invert: false, cell: 1.0 };
        assert_eq!(p.height(&img), 20.0);
        let (nx, ny, dx, dy, z) = relief_grid(&img, &p);
        assert_eq!((nx, ny), (41, 21));
        assert!((dx - 1.0).abs() < 1e-9 && (dy - 1.0).abs() < 1e-9);
        // ciemne na lewo: podstawa; jasne na prawo: podstawa + głębokość
        assert_eq!(z[0], 2.0);
        assert_eq!(z[nx - 1], 7.0);
        let inv = ReliefParams { invert: true, ..p };
        let (_, _, _, _, zi) = relief_grid(&img, &inv);
        assert_eq!((zi[0], zi[nx - 1]), (7.0, 2.0));
        // wygładzanie łagodzi skok, ale nie zmienia brzegów daleko od niego
        let s = ReliefParams { smooth: 2.0, ..p };
        let (_, _, _, _, zs) = relief_grid(&img, &s);
        assert!((zs[0] - 2.0).abs() < 0.01 && (zs[nx - 1] - 7.0).abs() < 0.01);
        let mid = nx / 2;
        assert!(zs[mid] > 2.5 && zs[mid] < 6.5, "{}", zs[mid]);
    }

    #[test]
    fn closed_solid_with_outward_normals() {
        let img = halves();
        let p = ReliefParams { width: 40.0, depth: 5.0, base: 2.0, smooth: 0.0, invert: false, cell: 2.0 };
        let m = relief_mesh(&img, &p);
        let (lo, hi) = m.bounds().unwrap();
        assert_eq!(lo, [0.0, 0.0, 0.0]);
        assert_eq!(hi, [40.0, 20.0, 7.0]);
        // objętość z twierdzenia o dywergencji (dodatnia = normalne na zewnątrz)
        let vol: f64 = m
            .tris
            .iter()
            .map(|t| {
                let (a, b, c) = (t[0].map(f64::from), t[1].map(f64::from), t[2].map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
            })
            .sum();
        // z interpolacją przejścia pośrodku: między 40·20·2 + połowa·5 a pełnym blokiem
        assert!(vol > 40.0 * 20.0 * 2.0 + 20.0 * 20.0 * 5.0 * 0.8 && vol < 40.0 * 20.0 * 7.0, "{vol}");
        // góra skierowana w górę, spód w dół
        assert!(m.tris.iter().filter(|t| t.iter().all(|p| p[2] > 0.0)).all(|t| normal(t)[2] >= 0.0));
        assert!(normal(m.tris.last().unwrap())[2] < 0.0);
    }

    #[test]
    fn decode_png_and_grid_limit() {
        // PNG 3 × 2: biały, szary, czarny / przezroczysty, …
        let mut img = image::RgbaImage::new(3, 2);
        img.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 0, image::Rgba([128, 128, 128, 255]));
        img.put_pixel(2, 0, image::Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, image::Rgba([255, 255, 255, 0]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let g = decode_image(&png).unwrap();
        assert_eq!((g.width, g.height), (3, 2));
        assert_eq!(&g.pixels[..4], &[255, 128, 0, 0], "przezroczyste = ciemne");
        assert!(decode_image(b"nie obraz").is_err());
        // bardzo drobna siatka jest ograniczana
        let p = ReliefParams { width: 2000.0, cell: 0.01, ..Default::default() };
        let (nx, _, _, _, _) = relief_grid(&g, &p);
        assert!(nx <= MAX_GRID_SIDE as usize + 2, "{nx}");
        assert_eq!(ReliefParams::for_image(&g).cell, 0.25);
    }
}
