//! Model 3D (STL) w półfabrykacie: siatka z pliku i jej ustawienie —
//! jednostki, skala, obrót o wielokrotności 90°, położenie, naddatek nad modelem.

use cam_io::Mesh;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use cam_ops::model::ModelHeights;
use std::sync::{Arc, OnceLock};

/// Macierz obrotu o wielokrotności 90° (wiersze).
pub type Rot = [[i8; 3]; 3];
pub const IDENTITY: Rot = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];

#[derive(Clone, Debug)]
pub struct Model3D {
    pub name: String,
    /// Siatka jak w pliku (współdzielona — kopie w historii jej nie kopiują).
    pub mesh: Arc<Mesh>,
    /// Mnożnik jednostek pliku na milimetry (1 = mm, 25,4 = cale).
    pub units: f64,
    /// Skala (1 = 100 %).
    pub scale: f64,
    pub rot: Rot,
    /// Materiał zostawiony nad modelem [mm] (góra modelu na Z = −naddatek).
    pub top_allowance: f64,
    /// Lewy dolny róg modelu (z góry) w układzie rysunku [mm].
    pub pos: [f64; 2],
    /// Trójkąty po ustawieniu (liczone w `refresh`).
    placed: Arc<Vec<[[f32; 3]; 3]>>,
    /// Rozmiar po ustawieniu [mm]: X, Y, Z.
    size: [f64; 3],
    /// Pole podcięć widzianych z dołu [mm²] (powierzchnie skierowane w dół ponad spodem modelu).
    undercut: f64,
    /// Mapa wysokości ustawionego modelu (liczona przy pierwszym użyciu).
    heights: Arc<OnceLock<Option<Arc<ModelHeights>>>>,
}

impl PartialEq for Model3D {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name
            && Arc::ptr_eq(&self.mesh, &o.mesh)
            && self.units == o.units
            && self.scale == o.scale
            && self.rot == o.rot
            && self.top_allowance == o.top_allowance
            && self.pos == o.pos
    }
}

fn mul(a: &Rot, b: &Rot) -> Rot {
    let mut r = [[0i8; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// Obrót o 90° wokół osi `axis` (0 = X, 1 = Y, 2 = Z).
pub fn quarter(axis: usize) -> Rot {
    match axis {
        0 => [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
        1 => [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
        _ => [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
    }
}

impl Model3D {
    pub fn new(name: String, mesh: Mesh, units: f64) -> Self {
        let mut m = Model3D {
            name,
            mesh: Arc::new(mesh),
            units,
            scale: 1.0,
            rot: IDENTITY,
            top_allowance: 0.0,
            pos: [0.0, 0.0],
            placed: Arc::new(Vec::new()),
            size: [0.0; 3],
            undercut: 0.0,
            heights: Arc::default(),
        };
        m.refresh();
        m
    }

    /// Obraca o 90° wokół osi świata.
    pub fn rotate(&mut self, axis: usize) {
        self.rot = mul(&quarter(axis), &self.rot);
        self.refresh();
    }

    /// Przelicza ustawione trójkąty po zmianie jednostek, skali, obrotu, położenia.
    pub fn refresh(&mut self) {
        let k = (self.units * self.scale) as f32;
        let r = self.rot.map(|row| row.map(|v| v as f32));
        let tf = |p: &[f32; 3]| -> [f32; 3] {
            let s = [p[0] * k, p[1] * k, p[2] * k];
            [
                r[0][0] * s[0] + r[0][1] * s[1] + r[0][2] * s[2],
                r[1][0] * s[0] + r[1][1] * s[1] + r[1][2] * s[2],
                r[2][0] * s[0] + r[2][1] * s[1] + r[2][2] * s[2],
            ]
        };
        let tris: Vec<[[f32; 3]; 3]> = self.mesh.tris.iter().map(|t| [tf(&t[0]), tf(&t[1]), tf(&t[2])]).collect();
        let b = Mesh { tris };
        let (lo, hi) = b.bounds().unwrap_or(([0.0; 3], [0.0; 3]));
        let h = hi[2] - lo[2];
        // lewy dolny róg w `pos`, góra na Z = −naddatek
        let d = [self.pos[0] as f32 - lo[0], self.pos[1] as f32 - lo[1], -(self.top_allowance as f32) - h - lo[2]];
        let mut tris: Vec<[[f32; 3]; 3]> =
            b.tris.into_iter().map(|t| t.map(|p| [p[0] + d[0], p[1] + d[1], p[2] + d[2]])).collect();
        // od najniższych: widok z góry rysuje wyższe na wierzchu
        tris.sort_by(|a, b| {
            let za = a[0][2] + a[1][2] + a[2][2];
            let zb = b[0][2] + b[1][2] + b[2][2];
            za.partial_cmp(&zb).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.size = [(hi[0] - lo[0]) as f64, (hi[1] - lo[1]) as f64, h as f64];
        self.undercut = undercut_area(&tris);
        self.placed = Arc::new(tris);
        self.heights = Arc::default();
    }

    /// Mapa wysokości z góry (oczko zależne od rozmiaru: 0,05–0,5 mm).
    pub fn heights(&self) -> Option<Arc<ModelHeights>> {
        self.heights
            .get_or_init(|| {
                let cell = (self.size[0].max(self.size[1]) / 2000.0).clamp(0.05, 0.5);
                ModelHeights::from_triangles(&self.placed, cell).map(Arc::new)
            })
            .clone()
    }

    pub fn placed(&self) -> &[[[f32; 3]; 3]] {
        &self.placed
    }

    /// Wymiary po ustawieniu [mm].
    pub fn size(&self) -> [f64; 3] {
        self.size
    }

    /// Zarys z góry (prostokąt) w układzie rysunku.
    pub fn footprint(&self) -> cam_geom::Aabb2 {
        cam_geom::Aabb2 {
            min: cam_geom::Vec2::new(self.pos[0], self.pos[1]),
            max: cam_geom::Vec2::new(self.pos[0] + self.size[0], self.pos[1] + self.size[1]),
        }
    }

    /// Przesuwa model (np. przy ustawianiu zera programu).
    pub fn translate(&mut self, d: cam_geom::Vec2) {
        self.pos = [self.pos[0] + d.x, self.pos[1] + d.y];
        self.refresh();
    }

    /// Pole podcięć [mm²]: miejsca, pod którymi jest powietrze, a nad nimi model —
    /// frez od góry ich nie obrobi.
    pub fn undercut_area(&self) -> f64 {
        self.undercut
    }

    /// Grubość półfabrykatu, w której mieści się model z naddatkiem.
    pub fn stock_thickness(&self) -> f64 {
        self.size[2] + self.top_allowance
    }
}

/// Pole rzutu z góry trójkątów skierowanych w dół (nie prawie pionowych), leżących
/// wyżej niż spód modelu. W bryle zamkniętej pod takim trójkątem jest powietrze,
/// a nad nim materiał — to podcięcie.
fn undercut_area(tris: &[[[f32; 3]; 3]]) -> f64 {
    let z_min = tris.iter().flatten().map(|p| p[2]).fold(f32::MAX, f32::min);
    let mut area = 0.0;
    for t in tris {
        let (a, b, c) = (t[0], t[1], t[2]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len <= 0.0 {
            continue;
        }
        let low = a[2].min(b[2]).min(c[2]);
        if n[2] / len < -0.05 && low > z_min + 0.05 {
            area += (-n[2] as f64) * 0.5; // pole rzutu na XY
        }
    }
    area
}

// ---------- zapis w projekcie: siatka jako binarny STL w base64 ----------

#[derive(Serialize, Deserialize)]
struct ModelFile {
    name: String,
    stl_base64: String,
    units: f64,
    scale: f64,
    rot: Rot,
    top_allowance: f64,
    pos: [f64; 2],
}

impl Serialize for Model3D {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ModelFile {
            name: self.name.clone(),
            stl_base64: b64_encode(&self.mesh.to_binary()),
            units: self.units,
            scale: self.scale,
            rot: self.rot,
            top_allowance: self.top_allowance,
            pos: self.pos,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Model3D {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = ModelFile::deserialize(d)?;
        let bytes = b64_decode(&f.stl_base64).ok_or_else(|| serde::de::Error::custom("base64"))?;
        let mesh = cam_io::parse_stl(&bytes).map_err(serde::de::Error::custom)?;
        let mut m = Model3D::new(f.name, mesh, f.units);
        m.scale = f.scale;
        m.rot = f.rot;
        m.top_allowance = f.top_allowance;
        m.pos = f.pos;
        m.refresh();
        Ok(m)
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut val = [255u8; 256];
    for (i, &c) in B64.iter().enumerate() {
        val[c as usize] = i as u8;
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for c in bytes.chunks(4) {
        let mut n = 0u32;
        let mut pad = 0;
        for &b in c {
            n <<= 6;
            if b == b'=' {
                pad += 1;
            } else {
                let v = val[b as usize];
                if v == 255 {
                    return None;
                }
                n |= v as u32;
            }
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Mesh {
        // prostopadłościan 30 × 20 × 10 (jako 2 trójkąty na każdym końcu wystarczą do wymiarów)
        Mesh { tris: vec![[[0.0, 0.0, 0.0], [30.0, 0.0, 0.0], [30.0, 20.0, 10.0]], [[0.0, 20.0, 10.0], [0.0, 0.0, 0.0], [30.0, 20.0, 0.0]]] }
    }

    /// Prostopadłościan od `lo` do `hi` z trójkątów skierowanych na zewnątrz.
    fn cuboid(lo: [f32; 3], hi: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
        let p = |i: usize| [if i & 1 != 0 { hi[0] } else { lo[0] }, if i & 2 != 0 { hi[1] } else { lo[1] }, if i & 4 != 0 { hi[2] } else { lo[2] }];
        let quads = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
        quads.iter().flat_map(|q| [[p(q[0]), p(q[1]), p(q[2])], [p(q[0]), p(q[2]), p(q[3])]]).collect()
    }

    #[test]
    fn undercuts_are_found() {
        // kostka: tylko spód skierowany w dół — bez podcięć
        let cube = Mesh { tris: cuboid([0.0; 3], [10.0, 10.0, 10.0]) };
        assert_eq!(Model3D::new("k".into(), cube, 1.0).undercut_area(), 0.0);
        // „T”: półka 30 × 10 na słupku 10 × 10 — spód półki wystaje (2 × 10 × 10 = 200 mm²)
        let mut t = cuboid([10.0, 0.0, 0.0], [20.0, 10.0, 10.0]);
        t.extend(cuboid([0.0, 0.0, 10.0], [30.0, 10.0, 15.0]));
        let m = Model3D::new("t".into(), Mesh { tris: t }, 1.0);
        // spód półki liczony w całości (300 mm²; środek nad słupkiem to tylko styk brył w tym teście)
        assert!(m.undercut_area() >= 200.0, "{}", m.undercut_area());
    }

    #[test]
    fn base64_round_trip() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0u8, 255, 128, 7, 9]] {
            assert_eq!(b64_decode(&b64_encode(data)).unwrap(), data);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
    }

    #[test]
    fn placement_rotation_units_and_save() {
        let mut m = Model3D::new("blok".into(), block(), 1.0);
        assert_eq!(m.size(), [30.0, 20.0, 10.0]);
        // góra modelu na Z = 0, dół na −10
        let zs: Vec<f32> = m.placed().iter().flatten().map(|p| p[2]).collect();
        assert_eq!(zs.iter().cloned().fold(f32::MIN, f32::max), 0.0);
        assert_eq!(zs.iter().cloned().fold(f32::MAX, f32::min), -10.0);
        // obrót wokół X o 90°: wysokość ↔ głębokość
        m.rotate(0);
        assert_eq!(m.size(), [30.0, 10.0, 20.0]);
        m.rotate(0);
        m.rotate(0);
        m.rotate(0);
        assert_eq!(m.rot, IDENTITY, "cztery obroty = bez obrotu");
        // cale, naddatek, położenie
        m.units = 25.4;
        m.top_allowance = 2.0;
        m.pos = [5.0, 5.0];
        m.refresh();
        assert!((m.size()[0] - 762.0).abs() < 1e-3);
        assert!((m.stock_thickness() - (254.0 + 2.0)).abs() < 1e-3);
        assert_eq!(m.footprint().min, cam_geom::Vec2::new(5.0, 5.0));
        // zapis i odczyt
        let json = serde_json::to_string(&m).unwrap();
        let back: Model3D = serde_json::from_str(&json).unwrap();
        assert_eq!(back.size(), m.size());
        assert_eq!(back.mesh.tris, m.mesh.tris);
        assert_eq!((back.units, back.top_allowance, back.pos, back.rot), (m.units, m.top_allowance, m.pos, m.rot));
    }
}
