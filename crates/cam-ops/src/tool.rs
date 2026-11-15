//! Narzędzia (frezy, wiertła) i biblioteka narzędzi.

use cam_i18n::tr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ToolKind {
    /// Frez walcowo-czołowy (płaski).
    FlatEnd,
    /// Frez kulowy.
    BallEnd,
    /// Frez z promieniem naroża (torusowy).
    BullNose { corner_radius: f64 },
    /// Frez V (fazowanie, V-carve), kąt wierzchołkowy w stopniach.
    VBit { angle: f64, tip_diameter: f64 },
    /// Frez grawerski stożkowy z płaskim czubkiem.
    Engraver { angle: f64, tip_diameter: f64 },
    /// Frez stożkowy kulowy: boki jak w V (kąt wierzchołkowy w stopniach), na końcu
    /// kulka o średnicy `tip_diameter` przechodząca stycznie w stożek — do wykańczania 3D.
    TaperedBall { angle: f64, tip_diameter: f64 },
    /// Wiertło, kąt wierzchołkowy w stopniach.
    Drill { point_angle: f64 },
}

impl ToolKind {
    pub fn label(&self) -> &'static str {
        match self {
            ToolKind::FlatEnd => tr!("Płaski"),
            ToolKind::BallEnd => tr!("Kulowy"),
            ToolKind::BullNose { .. } => tr!("Z promieniem naroża"),
            ToolKind::VBit { .. } => tr!("V (fazowanie)"),
            ToolKind::Engraver { .. } => tr!("Grawerski"),
            ToolKind::TaperedBall { .. } => tr!("Kulowy stożkowy"),
            ToolKind::Drill { .. } => tr!("Wiertło"),
        }
    }
}

/// Wysokość powierzchni frezu stożkowego kulowego nad czubkiem w odległości `d` od osi:
/// kulka o promieniu `rb` do punktu styczności, dalej stożek o połowie kąta `half` (rad).
pub fn tapered_ball_height(d: f64, rb: f64, half: f64) -> f64 {
    let rb = rb.max(1e-6);
    let dt = rb * half.cos();
    if d <= dt {
        rb - (rb * rb - d * d).max(0.0).sqrt()
    } else {
        rb * (1.0 - half.sin()) + (d - dt) / half.tan().max(1e-9)
    }
}

/// Kierunek spirali ostrzy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FluteDir {
    #[default]
    Upcut,
    Downcut,
    Compression,
    Straight,
}

impl FluteDir {
    pub fn label(&self) -> &'static str {
        match self {
            FluteDir::Upcut => tr!("W górę (upcut)"),
            FluteDir::Downcut => tr!("W dół (downcut)"),
            FluteDir::Compression => tr!("Kompresyjny"),
            FluteDir::Straight => tr!("Prosty"),
        }
    }
}

/// Parametry skrawania dla konkretnego materiału.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutPreset {
    pub material: String,
    pub rpm: f64,
    pub feed: f64,
    pub plunge_feed: f64,
    pub step_down: f64,
    pub stepover: f64,
}

/// Nazwa zestawu podstawowego (parametry zapisane bezpośrednio w narzędziu).
pub const DEFAULT_MATERIAL: &str = "Domyślne";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tool {
    /// Unikalny identyfikator w bibliotece (nie zmienia się po edycji).
    pub id: u32,
    /// Numer narzędzia w programie (T…).
    pub number: u32,
    pub name: String,
    pub kind: ToolKind,
    /// Średnica robocza [mm].
    pub diameter: f64,
    /// Długość części tnącej [mm].
    pub flute_length: f64,
    pub flutes: u32,
    pub shank_diameter: f64,
    // --- domyślne parametry skrawania ---
    pub rpm: f64,
    /// Posuw roboczy [mm/min].
    pub feed: f64,
    /// Posuw wgłębny [mm/min].
    pub plunge_feed: f64,
    /// Domyślny krok w dół [mm].
    pub step_down: f64,
    /// Domyślny stepover jako ułamek średnicy (0.4 = 40%).
    pub stepover: f64,
    pub flute_dir: FluteDir,
    /// Parametry dla materiałów (nadpisują domyślne, gdy projekt używa danego materiału).
    pub presets: Vec<CutPreset>,
    pub notes: String,
}

impl Default for Tool {
    fn default() -> Self {
        Tool {
            id: 0,
            number: 1,
            name: tr!("Frez 6 mm").into(),
            kind: ToolKind::FlatEnd,
            diameter: 6.0,
            flute_length: 20.0,
            flutes: 2,
            shank_diameter: 6.0,
            rpm: 18000.0,
            feed: 1200.0,
            plunge_feed: 300.0,
            step_down: 2.0,
            stepover: 0.4,
            flute_dir: FluteDir::Upcut,
            presets: Vec::new(),
            notes: String::new(),
        }
    }
}

impl Tool {
    pub fn radius(&self) -> f64 {
        self.diameter * 0.5
    }

    /// Parametry podstawowe jako zestaw.
    pub fn base_preset(&self) -> CutPreset {
        CutPreset {
            material: DEFAULT_MATERIAL.into(),
            rpm: self.rpm,
            feed: self.feed,
            plunge_feed: self.plunge_feed,
            step_down: self.step_down,
            stepover: self.stepover,
        }
    }

    /// Parametry dla materiału (lub podstawowe, gdy brak zestawu).
    pub fn cutting_for(&self, material: &str) -> CutPreset {
        self.presets.iter().find(|p| p.material == material).cloned().unwrap_or_else(|| self.base_preset())
    }

    /// Typowe zestawy materiałowe wyliczone z parametrów podstawowych (drewno).
    /// To punkt wyjścia — wartości trzeba sprawdzić dla własnej maszyny i narzędzia.
    pub fn with_material_presets(mut self) -> Self {
        let b = self.base_preset();
        let mk = |m: &str, rpm: f64, feed: f64, plunge: f64, step: f64, so: f64| CutPreset {
            material: m.into(),
            rpm: (b.rpm * rpm).round(),
            feed: (b.feed * feed).round(),
            plunge_feed: (b.plunge_feed * plunge).round(),
            step_down: (b.step_down * step * 100.0).round() / 100.0,
            stepover: so.min(b.stepover.max(so * 0.5)),
        };
        self.presets = vec![
            mk("Drewno", 1.0, 1.0, 1.0, 1.0, b.stepover),
            mk("MDF", 1.0, 1.25, 1.3, 1.5, b.stepover),
            mk("Aluminium", 0.66, 0.5, 0.5, 0.25, 0.3),
            mk("Plexi", 0.9, 0.8, 0.8, 0.75, b.stepover),
        ];
        self
    }

    pub fn display_name(&self) -> String {
        format!("T{} · {} · Ø{} mm", self.number, self.name, fmt_num(self.diameter))
    }
}

fn fmt_num(v: f64) -> String {
    let t = format!("{v:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Wersja zestawu startowego: 1 — pierwsze 7 narzędzi, 2 — zestaw podstawowy 3 / 4 / 6 mm,
/// 3 — frez stożkowy kulowy.
pub const DEFAULTS_VERSION: u32 = 3;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolLibrary {
    pub tools: Vec<Tool>,
    /// Którą wersję zestawu startowego biblioteka już dostała (0 = sprzed zapisywania wersji).
    /// Nowe narzędzia z zestawu dochodzą tylko raz — usunięte przez użytkownika nie wracają.
    #[serde(default)]
    pub defaults: u32,
}

impl ToolLibrary {
    /// Startowy zestaw typowych narzędzi.
    pub fn with_defaults() -> Self {
        let mut lib = ToolLibrary { defaults: DEFAULTS_VERSION, ..Default::default() };
        let base = Tool::default();
        for t in [
            Tool { number: 1, name: tr!("Frez płaski 6 mm").into(), ..base.clone() },
            Tool {
                number: 2,
                name: tr!("Frez płaski 3 mm").into(),
                diameter: 3.0,
                shank_diameter: 3.175,
                flute_length: 12.0,
                feed: 800.0,
                plunge_feed: 200.0,
                step_down: 1.0,
                ..base.clone()
            },
            Tool {
                number: 3,
                name: tr!("Frez płaski 1/8\"").into(),
                diameter: 3.175,
                shank_diameter: 3.175,
                flute_length: 12.0,
                feed: 800.0,
                plunge_feed: 200.0,
                step_down: 1.0,
                ..base.clone()
            },
            Tool {
                number: 4,
                name: tr!("Frez kulowy 6 mm").into(),
                kind: ToolKind::BallEnd,
                feed: 1500.0,
                step_down: 1.0,
                stepover: 0.15,
                ..base.clone()
            },
            Tool {
                number: 7,
                name: tr!("Grawerski 30° / 0,2").into(),
                kind: ToolKind::Engraver { angle: 30.0, tip_diameter: 0.2 },
                diameter: 3.175,
                shank_diameter: 3.175,
                flute_length: 5.0,
                flutes: 1,
                feed: 600.0,
                plunge_feed: 150.0,
                step_down: 0.3,
                flute_dir: FluteDir::Straight,
                ..base.clone()
            },
            Tool {
                number: 5,
                name: "V-bit 90°".into(),
                kind: ToolKind::VBit { angle: 90.0, tip_diameter: 0.2 },
                diameter: 6.0,
                feed: 800.0,
                plunge_feed: 200.0,
                step_down: 0.5,
                ..base.clone()
            },
            Tool {
                number: 6,
                name: tr!("Wiertło 6 mm").into(),
                kind: ToolKind::Drill { point_angle: 118.0 },
                rpm: 8000.0,
                feed: 300.0,
                plunge_feed: 150.0,
                step_down: 3.0,
                ..base.clone()
            },
            // --- zestaw podstawowy 3 / 4 / 6 mm ---
            Tool {
                number: 8,
                name: tr!("Frez płaski 4 mm").into(),
                diameter: 4.0,
                shank_diameter: 4.0,
                flute_length: 15.0,
                feed: 1000.0,
                plunge_feed: 250.0,
                step_down: 1.5,
                ..base.clone()
            },
            Tool {
                number: 9,
                name: tr!("Frez kulowy 3 mm").into(),
                kind: ToolKind::BallEnd,
                diameter: 3.0,
                shank_diameter: 3.175,
                flute_length: 12.0,
                feed: 1000.0,
                plunge_feed: 200.0,
                step_down: 0.5,
                stepover: 0.1,
                ..base.clone()
            },
            Tool {
                number: 10,
                name: tr!("Frez kulowy 4 mm").into(),
                kind: ToolKind::BallEnd,
                diameter: 4.0,
                shank_diameter: 4.0,
                flute_length: 15.0,
                feed: 1200.0,
                plunge_feed: 250.0,
                step_down: 0.8,
                stepover: 0.12,
                ..base.clone()
            },
            Tool {
                number: 11,
                name: tr!("V-bit 60° 3 mm").into(),
                kind: ToolKind::VBit { angle: 60.0, tip_diameter: 0.1 },
                diameter: 3.0,
                shank_diameter: 3.175,
                flute_length: 6.0,
                feed: 600.0,
                plunge_feed: 150.0,
                step_down: 0.3,
                ..base.clone()
            },
            Tool {
                number: 12,
                name: tr!("V-bit 60° 4 mm").into(),
                kind: ToolKind::VBit { angle: 60.0, tip_diameter: 0.1 },
                diameter: 4.0,
                shank_diameter: 4.0,
                flute_length: 8.0,
                feed: 700.0,
                plunge_feed: 180.0,
                step_down: 0.4,
                ..base.clone()
            },
            Tool {
                number: 13,
                name: tr!("V-bit 60° 6 mm").into(),
                kind: ToolKind::VBit { angle: 60.0, tip_diameter: 0.2 },
                diameter: 6.0,
                flute_length: 10.0,
                feed: 800.0,
                plunge_feed: 200.0,
                step_down: 0.5,
                ..base.clone()
            },
            Tool {
                number: 14,
                name: tr!("Wiertło 3 mm").into(),
                kind: ToolKind::Drill { point_angle: 118.0 },
                diameter: 3.0,
                shank_diameter: 3.0,
                flute_length: 30.0,
                rpm: 10000.0,
                feed: 200.0,
                plunge_feed: 120.0,
                step_down: 2.0,
                ..base.clone()
            },
            Tool {
                number: 16,
                name: tr!("Kulowy stożkowy R0,25 · 20°").into(),
                kind: ToolKind::TaperedBall { angle: 20.0, tip_diameter: 0.5 },
                diameter: 3.175,
                shank_diameter: 3.175,
                flute_length: 15.0,
                feed: 1000.0,
                plunge_feed: 200.0,
                step_down: 0.5,
                stepover: 0.1,
                ..base.clone()
            },
            Tool {
                number: 15,
                name: tr!("Wiertło 4 mm").into(),
                kind: ToolKind::Drill { point_angle: 118.0 },
                diameter: 4.0,
                shank_diameter: 4.0,
                flute_length: 40.0,
                rpm: 9000.0,
                feed: 250.0,
                plunge_feed: 130.0,
                step_down: 2.5,
                ..base.clone()
            },
        ] {
            let t = if matches!(t.kind, ToolKind::FlatEnd | ToolKind::BallEnd) { t.with_material_presets() } else { t };
            lib.add(t);
        }
        lib
    }

    /// Dodaje narzędzia z zestawu startowego, których w bibliotece nie ma (ten sam
    /// rodzaj, kąt i średnica). Numer z zestawu, jeśli wolny. Zwraca liczbę dodanych.
    pub fn add_missing_defaults(&mut self) -> usize {
        let same = |a: &Tool, b: &Tool| {
            let angle = |k: &ToolKind| match *k {
                ToolKind::VBit { angle, .. } | ToolKind::Engraver { angle, .. } | ToolKind::TaperedBall { angle, .. } => angle,
                _ => 0.0,
            };
            std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
                && (angle(&a.kind) - angle(&b.kind)).abs() < 1e-6
                && (a.diameter - b.diameter).abs() < 1e-6
        };
        let mut n = 0;
        for t in ToolLibrary::with_defaults().tools {
            if self.tools.iter().any(|o| same(o, &t)) {
                continue;
            }
            let number = if self.tools.iter().any(|o| o.number == t.number) { self.next_number() } else { t.number };
            self.add(Tool { number, ..t });
            n += 1;
        }
        n
    }

    /// Biblioteka zapisana przed nowszym zestawem startowym: dodaje (raz) brakujące
    /// narzędzia z zestawu. Zwraca `Some(liczba dodanych)`, gdy bibliotekę trzeba zapisać.
    pub fn upgrade_defaults(&mut self) -> Option<usize> {
        if self.defaults >= DEFAULTS_VERSION {
            return None;
        }
        let n = self.add_missing_defaults();
        self.defaults = DEFAULTS_VERSION;
        Some(n)
    }

    pub fn next_id(&self) -> u32 {
        self.tools.iter().map(|t| t.id).max().map_or(1, |m| m + 1)
    }

    pub fn next_number(&self) -> u32 {
        self.tools.iter().map(|t| t.number).max().map_or(1, |m| m + 1)
    }

    pub fn add(&mut self, mut t: Tool) -> u32 {
        t.id = self.next_id();
        let id = t.id;
        self.tools.push(t);
        id
    }

    pub fn get(&self, id: u32) -> Option<&Tool> {
        self.tools.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Tool> {
        self.tools.iter_mut().find(|t| t.id == id)
    }

    pub fn remove(&mut self, id: u32) {
        self.tools.retain(|t| t.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_set_and_adding_missing_tools() {
        let lib = ToolLibrary::with_defaults();
        let count = |f: &dyn Fn(&Tool) -> bool| lib.tools.iter().filter(|t| f(t)).count();
        // płaskie, kulowe, V (ostre) i wiertła w 3, 4 i 6 mm
        for d in [3.0, 4.0, 6.0] {
            for kind in [ToolKind::FlatEnd, ToolKind::BallEnd, ToolKind::VBit { angle: 60.0, tip_diameter: 0.0 }, ToolKind::Drill { point_angle: 118.0 }] {
                assert_eq!(
                    count(&|t| std::mem::discriminant(&t.kind) == std::mem::discriminant(&kind) && t.diameter == d),
                    if matches!(kind, ToolKind::VBit { .. }) && d == 6.0 { 2 } else { 1 },
                    "{kind:?} Ø{d}"
                );
            }
        }
        // numery bez powtórzeń
        let mut nums: Vec<u32> = lib.tools.iter().map(|t| t.number).collect();
        nums.sort();
        nums.dedup();
        assert_eq!(nums.len(), lib.tools.len());

        // pełna biblioteka: nic nie dochodzi
        let mut full = lib.clone();
        assert_eq!(full.add_missing_defaults(), 0);
        // stara biblioteka (bez nowych narzędzi, z zajętym numerem 8): dochodzą brakujące
        let mut old = ToolLibrary::default();
        for t in lib.tools.iter().filter(|t| t.number <= 7) {
            old.tools.push(t.clone());
        }
        old.add(Tool { number: 8, name: "mój frez".into(), diameter: 10.0, ..Default::default() });
        let added = old.add_missing_defaults();
        assert_eq!(added, lib.tools.len() - 7);
        assert!(old.tools.iter().any(|t| t.name == "mój frez" && t.number == 8));
        let mut nums: Vec<u32> = old.tools.iter().map(|t| t.number).collect();
        nums.sort();
        nums.dedup();
        assert_eq!(nums.len(), old.tools.len(), "numery bez powtórzeń");
        assert_eq!(old.add_missing_defaults(), 0);

        // automatycznie, raz: stara biblioteka (bez wersji) dostaje zestaw, potem już nie
        let mut saved = ToolLibrary::default();
        for t in lib.tools.iter().filter(|t| t.number <= 7) {
            saved.tools.push(t.clone());
        }
        let json = serde_json::to_string(&ToolLibrary { defaults: 0, ..saved.clone() }).unwrap().replace(",\"defaults\":0", "");
        let mut loaded: ToolLibrary = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.defaults, 0, "plik sprzed wersji zestawu");
        assert_eq!(loaded.upgrade_defaults(), Some(lib.tools.len() - 7));
        // użytkownik usuwa jeden z nowych — po ponownym uruchomieniu nie wraca
        let id = loaded.tools.iter().find(|t| t.number == 9).unwrap().id;
        loaded.remove(id);
        assert_eq!(loaded.upgrade_defaults(), None);
        assert!(!loaded.tools.iter().any(|t| t.number == 9));
        assert_eq!(ToolLibrary::with_defaults().clone().upgrade_defaults(), None);
    }
}
