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

/// Ustawienia płaskorzeźby [mm]; filtry obrazu działają w kolejności pól
/// (kadrowanie, odszumianie, wygładzanie, wyostrzanie, poziomy, gamma, odwrócenie,
/// tło, schodki, owal, ramka).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReliefParams {
    /// Szerokość (X); wysokość wynika z proporcji (wykadrowanego) obrazu.
    pub width: f64,
    /// Różnica wysokości między najciemniejszym a najjaśniejszym miejscem.
    pub depth: f64,
    /// Podstawa pod najniższym miejscem.
    pub base: f64,
    /// Wygładzanie (rozmycie Gaussa) [mm] — usuwa drobne ziarno zdjęcia.
    pub smooth: f64,
    /// Ciemne wyżej, jasne niżej.
    pub invert: bool,
    /// Odstęp punktów siatki.
    pub cell: f64,
    /// Kadrowanie: części szerokości/wysokości obcięte z lewej, prawej, góry i dołu (0–0,45).
    pub crop: [f64; 4],
    /// Odbicie lustrzane (lewo ↔ prawo), np. do stempli.
    pub mirror: bool,
    /// Odszumianie (mediana) [mm] — usuwa pojedyncze plamki, zostawia krawędzie.
    pub denoise: f64,
    /// Wyostrzanie (0 = bez, 1 = mocne) — wyraźniejsze drobne szczegóły.
    pub sharpen: f64,
    /// Poziomy: jasność, od której jest najniżej (0–1), i od której najwyżej.
    pub black: f64,
    pub white: f64,
    /// Gamma: < 1 podnosi ciemne miejsca, > 1 obniża.
    pub gamma: f64,
    /// Płaskie tło: wszystko niżej niż ten próg (0–1) trafia na podstawę.
    pub background: f64,
    /// Schodki: liczba poziomów (0 = gładko).
    pub steps: u32,
    /// Owalna maska: poza owalem wpisanym w prostokąt (wewnątrz ramki) — podstawa.
    pub oval: bool,
    /// Ramka wokół: szerokość [mm] (0 = bez) i wysokość nad podstawą [mm].
    pub frame_width: f64,
    pub frame_height: f64,
}

impl Default for ReliefParams {
    fn default() -> Self {
        ReliefParams {
            width: 100.0,
            depth: 5.0,
            base: 2.0,
            smooth: 0.3,
            invert: false,
            cell: 0.25,
            crop: [0.0; 4],
            mirror: false,
            denoise: 0.0,
            sharpen: 0.0,
            black: 0.0,
            white: 1.0,
            gamma: 1.0,
            background: 0.0,
            steps: 0,
            oval: false,
            frame_width: 0.0,
            frame_height: 5.0,
        }
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

    /// Kadrowanie po ograniczeniu (zostaje co najmniej 10 % obrazu w każdym kierunku).
    fn crop_fractions(&self) -> [f64; 4] {
        self.crop.map(|v| v.clamp(0.0, 0.45))
    }

    /// Wykadrowany fragment obrazu w pikselach: (x0, x1, y0, y1), y od góry.
    fn crop_px(&self, img: &GrayImage) -> (f64, f64, f64, f64) {
        let [l, r, t, b] = self.crop_fractions();
        let (iw, ih) = ((img.width - 1) as f64, (img.height - 1) as f64);
        (l * iw, (1.0 - r) * iw, t * ih, (1.0 - b) * ih)
    }

    /// Wysokość (Y) z proporcji wykadrowanego obrazu.
    pub fn height(&self, img: &GrayImage) -> f64 {
        let [l, r, t, b] = self.crop_fractions();
        let cw = img.width as f64 * (1.0 - l - r);
        let ch = img.height as f64 * (1.0 - t - b);
        self.width * ch / cw.max(1e-9)
    }

    /// Odstęp punktów siatki po ograniczeniu liczby punktów.
    pub fn effective_cell(&self, img: &GrayImage) -> f64 {
        let long = self.width.max(self.height(img));
        self.cell.max(long / MAX_GRID_SIDE).max(0.01)
    }

    /// Najwyższe miejsce nad podstawą [mm] (płaskorzeźba albo ramka).
    pub fn top(&self) -> f64 {
        let frame = if self.frame_width > 0.0 { self.frame_height.max(0.0) } else { 0.0 };
        self.depth.max(0.0).max(frame)
    }
}

/// Poziomy automatycznie: jasność 1. i 99. percentyla wykadrowanego obrazu —
/// pełna głębokość bez pojedynczych skrajnych pikseli.
pub fn auto_levels(img: &GrayImage, p: &ReliefParams) -> (f64, f64) {
    let (x0, x1, y0, y1) = p.crop_px(img);
    let mut hist = [0u64; 256];
    let mut n = 0u64;
    for y in (y0.round() as u32)..=(y1.round() as u32).min(img.height - 1) {
        for x in (x0.round() as u32)..=(x1.round() as u32).min(img.width - 1) {
            hist[img.pixels[(y * img.width + x) as usize] as usize] += 1;
            n += 1;
        }
    }
    let pct = |q: f64| {
        let target = (n as f64 * q) as u64;
        let mut acc = 0;
        for (v, &c) in hist.iter().enumerate() {
            acc += c;
            if acc > target {
                return v as f64 / 255.0;
            }
        }
        1.0
    };
    let (lo, hi) = (pct(0.01), pct(0.99));
    if hi - lo < 0.02 { (0.0, 1.0) } else { (lo, hi) }
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
    let cell = p.effective_cell(img);
    relief_grid_with_cell(img, p, cell)
}

fn relief_grid_with_cell(img: &GrayImage, p: &ReliefParams, cell: f64) -> (usize, usize, f64, f64, Vec<f32>) {
    let (w, h) = (p.width.max(0.1), p.height(img).max(0.1));
    let nx = ((w / cell).round() as usize).max(2) + 1;
    let ny = ((h / cell).round() as usize).max(2) + 1;
    let (dx, dy) = (w / (nx - 1) as f64, h / (ny - 1) as f64);
    let d = dx.min(dy);
    let (x0, x1, y0, y1) = p.crop_px(img);
    let mut v = vec![0f32; nx * ny];
    for j in 0..ny {
        // j = 0 to dół płaskorzeźby, czyli dolny wiersz (wykadrowanego) obrazu
        let py = y0 + (1.0 - j as f64 / (ny - 1) as f64) * (y1 - y0);
        for i in 0..nx {
            let f = i as f64 / (nx - 1) as f64;
            let px = if p.mirror { x1 - f * (x1 - x0) } else { x0 + f * (x1 - x0) };
            v[j * nx + i] = img.sample(px, py);
        }
    }
    // odszumianie, wygładzanie, wyostrzanie
    let r = (p.denoise / d).round().min(6.0) as usize;
    if r >= 1 {
        median(&mut v, nx, ny, r);
    }
    let sigma = p.smooth / d;
    if sigma > 0.05 {
        blur(&mut v, nx, ny, sigma);
    }
    if p.sharpen > 0.0 {
        let mut b = v.clone();
        blur(&mut b, nx, ny, (1.0 / d).max(1.0));
        let k = (p.sharpen * 3.0) as f32;
        for (x, b) in v.iter_mut().zip(&b) {
            *x = (*x + k * (*x - b)).clamp(0.0, 1.0);
        }
    }
    // poziomy, gamma, odwrócenie, tło, schodki
    let (black, white) = (p.black.clamp(0.0, 1.0) as f32, p.white.clamp(0.0, 1.0) as f32);
    let span = (white - black).max(1e-3);
    let gamma = p.gamma.clamp(0.1, 10.0) as f32;
    let bg = p.background.clamp(0.0, 1.0) as f32;
    let steps = p.steps;
    for x in v.iter_mut() {
        let mut t = ((*x - black) / span).clamp(0.0, 1.0).powf(gamma);
        if p.invert {
            t = 1.0 - t;
        }
        if t < bg {
            t = 0.0;
        }
        if steps >= 2 {
            let n = (steps - 1) as f32;
            t = (t * n).round() / n;
        }
        *x = t;
    }
    // owal i ramka
    let (depth, base) = (p.depth.max(0.0) as f32, p.base.max(0.0) as f32);
    let fw = p.frame_width.max(0.0);
    let frame_z = base + p.frame_height.max(0.0) as f32;
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ax, ay) = ((w * 0.5 - fw).max(1e-6), (h * 0.5 - fw).max(1e-6));
    for j in 0..ny {
        let y = j as f64 * dy;
        for i in 0..nx {
            let x = i as f64 * dx;
            let k = j * nx + i;
            let mut t = v[k];
            if p.oval && ((x - cx) / ax).powi(2) + ((y - cy) / ay).powi(2) > 1.0 {
                t = 0.0;
            }
            let in_frame = fw > 0.0 && (x < fw || y < fw || x > w - fw || y > h - fw);
            v[k] = if in_frame { frame_z } else { base + depth * t };
        }
    }
    (nx, ny, dx, dy, v)
}

/// Podgląd mapy wysokości (do miniatury): (szerokość, wysokość, piksele 0–255,
/// wiersz 0 u góry) — wyżej jaśniej.
pub fn relief_preview(img: &GrayImage, p: &ReliefParams, max_side: usize) -> (usize, usize, Vec<u8>) {
    let long = p.width.max(p.height(img));
    let cell = (long / max_side.max(8) as f64).max(p.effective_cell(img));
    // filtry w mm przeliczone na grubszą siatkę działają tak samo jak na pełnej
    let (nx, ny, _, _, z) = relief_grid_with_cell(img, p, cell);
    let lo = p.base.max(0.0) as f32;
    let span = (p.top() as f32).max(1e-6);
    let mut out = vec![0u8; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let t = ((z[j * nx + i] - lo) / span).clamp(0.0, 1.0);
            out[(ny - 1 - j) * nx + i] = (t * 255.0).round() as u8;
        }
    }
    (nx, ny, out)
}

/// Mediana w kwadracie (2r+1)² — odszumianie bez rozmywania krawędzi.
fn median(v: &mut [f32], nx: usize, ny: usize, r: usize) {
    let src = v.to_vec();
    let mut win = Vec::with_capacity((2 * r + 1) * (2 * r + 1));
    for j in 0..ny {
        for i in 0..nx {
            win.clear();
            for jj in j.saturating_sub(r)..=(j + r).min(ny - 1) {
                for ii in i.saturating_sub(r)..=(i + r).min(nx - 1) {
                    win.push(src[jj * nx + ii]);
                }
            }
            let m = win.len() / 2;
            win.select_nth_unstable_by(m, |a, b| a.total_cmp(b));
            v[j * nx + i] = win[m];
        }
    }
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
        let p = ReliefParams { width: 40.0, depth: 5.0, base: 2.0, smooth: 0.0, cell: 1.0, ..Default::default() };
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
        let p = ReliefParams { width: 40.0, depth: 5.0, base: 2.0, smooth: 0.0, cell: 2.0, ..Default::default() };
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

    /// Poziomy szarości od lewej (0) do prawej (255), 256 × 4.
    fn ramp() -> GrayImage {
        GrayImage { width: 256, height: 4, pixels: (0..4).flat_map(|_| 0..=255u8).collect() }
    }

    #[test]
    fn levels_gamma_background_and_steps() {
        let img = ramp();
        let p = ReliefParams { width: 100.0, depth: 10.0, base: 0.0, smooth: 0.0, cell: 1.0, ..Default::default() };
        let row = |p: &ReliefParams| {
            let (nx, _, _, _, z) = relief_grid(&img, p);
            z[..nx].to_vec()
        };
        // bez filtrów: liniowo od 0 do 10
        let z = row(&p);
        assert!((z[50] - 5.0).abs() < 0.1, "{}", z[50]);
        // poziomy 25–75 %: lewa ćwiartka na dnie, prawa na górze, środek bez zmian
        let lv = ReliefParams { black: 0.25, white: 0.75, ..p };
        let z = row(&lv);
        assert_eq!((z[10], z[90]), (0.0, 10.0));
        assert!((z[50] - 5.0).abs() < 0.1);
        // gamma 2 obniża środek
        let z = row(&ReliefParams { gamma: 2.0, ..p });
        assert!((z[50] - 2.5).abs() < 0.15, "{}", z[50]);
        // tło: poniżej 30 % — płasko na podstawie
        let z = row(&ReliefParams { background: 0.3, ..p });
        assert_eq!(z[25], 0.0);
        assert!(z[40] > 3.5);
        // schodki: tylko 3 wysokości
        let z = row(&ReliefParams { steps: 3, ..p });
        let mut levels: Vec<i32> = z.iter().map(|v| (v * 10.0).round() as i32).collect();
        levels.dedup();
        assert_eq!(levels, vec![0, 50, 100]);
        // automatyczne poziomy: pełny zakres już wykorzystany
        let (lo, hi) = auto_levels(&img, &p);
        assert!(lo < 0.02 && hi > 0.98);
    }

    #[test]
    fn crop_mirror_oval_and_frame() {
        let img = ramp();
        let p = ReliefParams { width: 100.0, depth: 10.0, base: 2.0, smooth: 0.0, cell: 1.0, ..Default::default() };
        // kadrowanie ćwiartki z lewej: proporcje i zakres jasności z reszty obrazu
        let c = ReliefParams { crop: [0.25, 0.0, 0.0, 0.0], ..p };
        assert!((c.height(&img) - 100.0 * 4.0 / 192.0).abs() < 1e-9);
        let (nx, _, _, _, z) = relief_grid(&img, &c);
        assert!(z[0] > 4.0 && z[0] < 5.0 && z[nx - 1] > 11.9, "{} {}", z[0], z[nx - 1]);
        // więcej niż 45 % z jednej strony nie da się obciąć
        let big = ReliefParams { crop: [0.9, 0.0, 0.0, 0.0], ..p };
        assert!((big.height(&img) - 100.0 * 4.0 / (256.0 * 0.55)).abs() < 1e-9);
        // lustro: najwyżej po lewej
        let (nx, _, _, _, z) = relief_grid(&img, &ReliefParams { mirror: true, ..p });
        assert!(z[0] > z[nx - 1]);
        // ramka 3 mm na wysokości 4 mm, owal wewnątrz: rogi wewnątrz ramki na podstawie
        let sq = GrayImage { width: 4, height: 4, pixels: vec![255; 16] };
        let f = ReliefParams { width: 40.0, frame_width: 3.0, frame_height: 4.0, oval: true, ..p };
        let (nx, ny, _, _, z) = relief_grid(&sq, &f);
        assert_eq!(z[0], 6.0, "ramka");
        assert_eq!(z[4 * nx + 4], 2.0, "róg poza owalem");
        assert_eq!(z[(ny / 2) * nx + nx / 2], 12.0, "środek");
        assert_eq!(f.top(), 10.0);
        // podgląd: środek najjaśniejszy, róg wewnątrz ramki ciemny
        let (w, h, px) = relief_preview(&sq, &f, 64);
        assert_eq!(px[(h / 2) * w + w / 2], 255);
        assert!(px[2 * w + 2] < 128 || px[5 * w + 5] == 0);
    }

    #[test]
    fn denoise_removes_a_speck() {
        // plamka 2 × 2 px — w oknie 3 × 3 mediany zawsze mniejszość
        let mut pixels = vec![0u8; 64];
        for k in [3 * 8 + 3, 3 * 8 + 4, 4 * 8 + 3, 4 * 8 + 4] {
            pixels[k] = 255;
        }
        let img = GrayImage { width: 8, height: 8, pixels };
        let p = ReliefParams { width: 8.0, depth: 10.0, base: 0.0, smooth: 0.0, cell: 1.0, ..Default::default() };
        let max = |p: &ReliefParams| relief_grid(&img, p).4.into_iter().fold(0f32, f32::max);
        assert!(max(&p) > 5.0);
        // po próbkowaniu plamka zajmuje 3 × 3 punkty siatki — okno 5 × 5 (2 mm) ją usuwa
        assert_eq!(max(&ReliefParams { denoise: 2.0, ..p }), 0.0);
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
