//! Narzędzia (frezy, wiertła) i biblioteka narzędzi.

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
    /// Wiertło, kąt wierzchołkowy w stopniach.
    Drill { point_angle: f64 },
}

impl ToolKind {
    pub fn label(&self) -> &'static str {
        match self {
            ToolKind::FlatEnd => "Płaski",
            ToolKind::BallEnd => "Kulowy",
            ToolKind::BullNose { .. } => "Z promieniem naroża",
            ToolKind::VBit { .. } => "V (fazowanie)",
            ToolKind::Engraver { .. } => "Grawerski",
            ToolKind::Drill { .. } => "Wiertło",
        }
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
            FluteDir::Upcut => "W górę (upcut)",
            FluteDir::Downcut => "W dół (downcut)",
            FluteDir::Compression => "Kompresyjny",
            FluteDir::Straight => "Prosty",
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
            name: "Frez 6 mm".into(),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolLibrary {
    pub tools: Vec<Tool>,
}

impl ToolLibrary {
    /// Startowy zestaw typowych narzędzi.
    pub fn with_defaults() -> Self {
        let mut lib = ToolLibrary::default();
        let base = Tool::default();
        for t in [
            Tool { number: 1, name: "Frez płaski 6 mm".into(), ..base.clone() },
            Tool {
                number: 2,
                name: "Frez płaski 3 mm".into(),
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
                name: "Frez płaski 1/8\"".into(),
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
                name: "Frez kulowy 6 mm".into(),
                kind: ToolKind::BallEnd,
                feed: 1500.0,
                step_down: 1.0,
                stepover: 0.15,
                ..base.clone()
            },
            Tool {
                number: 7,
                name: "Grawerski 30° / 0,2".into(),
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
                name: "Wiertło 6 mm".into(),
                kind: ToolKind::Drill { point_angle: 118.0 },
                rpm: 8000.0,
                feed: 300.0,
                plunge_feed: 150.0,
                step_down: 3.0,
                ..base.clone()
            },
        ] {
            let t = if matches!(t.kind, ToolKind::FlatEnd | ToolKind::BallEnd) { t.with_material_presets() } else { t };
            lib.add(t);
        }
        lib
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
