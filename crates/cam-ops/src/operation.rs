//! Definicje operacji i generowanie ścieżek.

use crate::tool::Tool;
use crate::toolpath::Toolpath;
use cam_geom::Contour;
use serde::{Deserialize, Serialize};

/// Wspólne parametry skrawania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CutParams {
    /// Z górnej powierzchni (zwykle 0 = góra materiału).
    pub top_z: f64,
    /// Głębokość obróbki od `top_z` [mm, dodatnia].
    pub depth: f64,
    pub step_down: f64,
    pub feed: f64,
    pub plunge_feed: f64,
    pub rpm: f64,
    /// Wysokość bezpieczna dla przejazdów [mm].
    pub safe_z: f64,
    /// Wysokość nad obrabianą powierzchnią, do której zjeżdża się szybkim ruchem [mm].
    pub retract_clearance: f64,
}

impl Default for CutParams {
    fn default() -> Self {
        CutParams {
            top_z: 0.0,
            depth: 3.0,
            step_down: 2.0,
            feed: 1200.0,
            plunge_feed: 300.0,
            rpm: 18000.0,
            safe_z: 5.0,
            retract_clearance: 1.0,
        }
    }
}

impl CutParams {
    pub fn bottom_z(&self) -> f64 {
        self.top_z - self.depth
    }

    /// Przepisuje parametry skrawania z narzędzia (dla danego materiału).
    pub fn apply_tool(&mut self, t: &Tool, material: &str) {
        let p = t.cutting_for(material);
        self.step_down = p.step_down;
        self.feed = p.feed;
        self.plunge_feed = p.plunge_feed;
        self.rpm = p.rpm;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSide {
    Outside,
    Inside,
    On,
}

/// Kierunek frezowania (dla wrzeciona obracającego się w prawo, M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Współbieżne.
    Climb,
    /// Przeciwbieżne.
    Conventional,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    /// Wejście pionowe.
    Plunge,
    /// Rampa wzdłuż ścieżki pod kątem [°].
    Ramp { angle: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tabs {
    pub count: u32,
    /// Szerokość mostka na detalu [mm].
    pub width: f64,
    /// Wysokość mostka od dna [mm].
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileParams {
    pub side: ProfileSide,
    pub direction: Direction,
    pub entry: Entry,
    /// Naddatek na ściance [mm].
    pub stock_to_leave: f64,
    pub tabs: Option<Tabs>,
    /// Przejście wykańczające: zgrubne przejścia zostawiają tyle [mm], a na końcu
    /// jedno przejście na pełnej głębokości zbiera to do wymiaru.
    pub finish: Option<f64>,
    /// Wejście i wyjście po łuku stycznym o tym promieniu [mm] (zejście w odpadzie, obok ścianki).
    pub lead: Option<f64>,
}

impl Default for ProfileParams {
    fn default() -> Self {
        Self {
            side: ProfileSide::Outside,
            direction: Direction::Climb,
            entry: Entry::Ramp { angle: 5.0 },
            stock_to_leave: 0.0,
            tabs: None,
            finish: None,
            lead: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PocketStrategy {
    /// Pętle równoległe do konturu, od środka na zewnątrz.
    #[default]
    Offset,
    /// Przejścia równoległe tam i z powrotem + przejście wykańczające po obrysie.
    Zigzag,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PocketParams {
    pub strategy: PocketStrategy,
    /// Kąt przejść zygzaka [°] (0 = wzdłuż X).
    pub zigzag_angle: f64,
    /// Stepover jako ułamek średnicy.
    pub stepover: f64,
    pub direction: Direction,
    pub entry: Entry,
    pub stock_to_leave: f64,
    /// Przejście wykańczające ścian (i wysp) na pełnej głębokości; zgrubna obróbka zostawia tyle [mm].
    pub finish: Option<f64>,
}

impl Default for PocketParams {
    fn default() -> Self {
        Self {
            strategy: PocketStrategy::Offset,
            zigzag_angle: 0.0,
            stepover: 0.4,
            direction: Direction::Climb,
            entry: Entry::Ramp { angle: 3.0 },
            stock_to_leave: 0.0,
            finish: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrillParams {
    /// Głębokość jednego wiercenia przy wierceniu z wycofaniem (None = jednym ruchem).
    pub peck: Option<f64>,
    /// Frezowanie otworu po helisie (dla otworów większych niż narzędzie).
    pub helix: bool,
}

impl Default for DrillParams {
    fn default() -> Self {
        Self { peck: Some(3.0), helix: false }
    }
}

/// Strona prowadzenia narzędzia względem linii przy grawerowaniu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EngraveSide {
    /// Środek narzędzia na linii.
    #[default]
    Center,
    /// Narzędzie po lewej stronie linii (patrząc w kierunku krzywej).
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngraveParams {
    pub entry: Entry,
    pub side: EngraveSide,
}

impl Default for EngraveParams {
    fn default() -> Self {
        Self { entry: Entry::Plunge, side: EngraveSide::Center }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OpKind {
    Profile(ProfileParams),
    Pocket(PocketParams),
    Drill(DrillParams),
    /// Prowadzenie po linii (grawer, rowki) — środkiem lub bokiem narzędzia.
    Engrave(EngraveParams),
}

impl OpKind {
    pub fn label(&self) -> &'static str {
        match self {
            OpKind::Profile(_) => "Profil",
            OpKind::Pocket(_) => "Kieszeń",
            OpKind::Drill(_) => "Wiercenie",
            OpKind::Engrave(_) => "Grawer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub enabled: bool,
    pub kind: OpKind,
    /// Identyfikator narzędzia w bibliotece.
    pub tool_id: u32,
    pub cut: CutParams,
    /// Indeksy konturów dokumentu, na których działa operacja.
    pub contours: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct OpResult {
    pub toolpath: Toolpath,
    pub warnings: Vec<String>,
}

/// Generuje ścieżkę dla operacji. `contours` to geometria w kolejności `op.contours`.
pub fn generate(op: &Operation, tool: &Tool, contours: &[Contour]) -> OpResult {
    let mut res = OpResult::default();
    if tool.diameter <= 0.0 {
        res.warnings.push("Średnica narzędzia musi być > 0".into());
        return res;
    }
    if op.cut.depth <= 0.0 {
        res.warnings.push("Głębokość musi być > 0".into());
        return res;
    }
    if op.cut.depth > tool.flute_length + 1e-9 && tool.flute_length > 0.0 {
        res.warnings.push(format!(
            "Głębokość {:.2} mm większa niż długość ostrzy narzędzia ({:.2} mm)",
            op.cut.depth, tool.flute_length
        ));
    }
    match &op.kind {
        OpKind::Profile(p) => crate::profile::generate(&op.cut, p, tool, contours, &mut res),
        OpKind::Pocket(p) => crate::pocket::generate(&op.cut, p, tool, contours, &mut res),
        OpKind::Drill(p) => crate::drill::generate(&op.cut, p, tool, contours, &mut res),
        OpKind::Engrave(p) => crate::engrave::generate(&op.cut, p, tool, contours, &mut res),
    }
    res.toolpath.retract(op.cut.safe_z);
    res
}

/// Długość rampy dla zejścia o `dz` przy danym wejściu (None = wejście pionowe).
pub(crate) fn ramp_length(entry: Entry, dz: f64) -> Option<f64> {
    match entry {
        Entry::Plunge => None,
        Entry::Ramp { angle } => {
            let a = angle.clamp(0.5, 89.0).to_radians();
            Some(dz.max(0.0) / a.tan())
        }
    }
}

/// Odległość od `pos` do miejsca, w którym narzędzie zacznie kontur: najbliższy
/// wierzchołek konturu zamkniętego albo bliższy koniec otwartego.
fn entry_distance(c: &Contour, pos: cam_geom::Vec2) -> f64 {
    if c.closed {
        c.verts.iter().map(|v| v.p.distance(pos)).fold(f64::MAX, f64::min)
    } else {
        let a = c.start().unwrap_or_default().distance(pos);
        let b = c.end().unwrap_or_default().distance(pos);
        a.min(b)
    }
}

/// Ustawia początek konturu blisko `pos`, żeby jałowy przejazd był krótki.
/// Kontur zamknięty: start w najbliższym wierzchołku (narożniku), a przy
/// `mid_edge` — w środku najbliższej krawędzi (tam mieści się wejście po łuku).
/// Kontur otwarty: odwrócony, jeśli koniec jest bliżej niż początek.
pub(crate) fn start_near(c: &mut Contour, pos: cam_geom::Vec2, mid_edge: bool) {
    if c.verts.len() < 2 {
        return;
    }
    if !c.closed {
        let a = c.start().unwrap_or_default().distance(pos);
        let b = c.end().unwrap_or_default().distance(pos);
        if b < a {
            c.reverse();
        }
        return;
    }
    if mid_edge {
        let segs = crate::path::segments(c);
        let longest = segs.iter().map(|s| s.len()).fold(0.0, f64::max);
        let best = segs
            .iter()
            .filter(|s| s.len() >= longest * 0.25)
            .map(|s| s.point_at(s.len() * 0.5))
            .min_by(|a, b| a.distance(pos).partial_cmp(&b.distance(pos)).unwrap());
        if let Some(q) = best {
            if let Some((seg, q)) = c.closest_point(q) {
                c.rotate_start_to(seg, q);
            }
        }
        return;
    }
    let (k, _) = c
        .verts
        .iter()
        .enumerate()
        .map(|(k, v)| (k, v.p.distance(pos)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();
    let q = c.verts[k].p;
    c.rotate_start_to(k, q);
}

/// Porządkuje kontury: najpierw najgłębiej zagnieżdżone (otwory przed obrysem),
/// w obrębie poziomu — najbliższy sąsiad (licząc do miejsca, w którym
/// narzędzie zacznie kontur).
pub(crate) fn order_inside_first(contours: &[Contour]) -> Vec<usize> {
    let depth: Vec<usize> = contours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p = c.start().unwrap_or_default();
            contours.iter().enumerate().filter(|(j, o)| *j != i && o.closed && o.contains_point(p)).count()
        })
        .collect();
    let mut remaining: Vec<usize> = (0..contours.len()).collect();
    let mut out = Vec::new();
    let mut pos = cam_geom::Vec2::ZERO;
    while !remaining.is_empty() {
        let max_d = remaining.iter().map(|&i| depth[i]).max().unwrap();
        let (k, _) = remaining
            .iter()
            .enumerate()
            .filter(|(_, i)| depth[**i] == max_d)
            .map(|(k, &i)| (k, entry_distance(&contours[i], pos)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        let i = remaining.remove(k);
        let mut c = contours[i].clone();
        start_near(&mut c, pos, false);
        pos = c.end().unwrap_or_default();
        if c.closed {
            pos = c.start().unwrap_or(pos);
        }
        out.push(i);
    }
    out
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use cam_geom::{Vec2, Vertex};

    fn square() -> Contour {
        Contour {
            verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(20.0, 0.0, 0.0), Vertex::new(20.0, 20.0, 0.0), Vertex::new(0.0, 20.0, 0.0)],
            closed: true,
        }
    }

    #[test]
    fn start_moves_to_the_nearest_corner_or_edge_middle() {
        let mut c = square();
        start_near(&mut c, Vec2::new(25.0, 24.0), false);
        assert_eq!(c.start(), Some(Vec2::new(20.0, 20.0)));
        let mut c = square();
        start_near(&mut c, Vec2::new(30.0, 8.0), true);
        assert_eq!(c.start(), Some(Vec2::new(20.0, 10.0)), "środek prawej krawędzi");
        assert!((c.signed_area() - 400.0).abs() < 1e-9, "kształt bez zmian");
        let mut l = Contour { verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 0.0)], closed: false };
        start_near(&mut l, Vec2::new(12.0, 0.0), false);
        assert_eq!(l.start(), Some(Vec2::new(10.0, 0.0)), "otwarta: od bliższego końca");
    }
}
