//! Pasek narzędzi po lewej: grupy (kolumna 1) i ich warianty (kolumna 2).

use crate::icons::{self, Icon};
use cam_ops::{
    DrillParams, EngraveParams, EngraveSide, OpKind, PocketParams, PocketStrategy, ProfileParams, ProfileSide, Tabs,
};
use eframe::egui::{self, Color32, RichText};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Select,
    Profile,
    Pocket,
    Drill,
    Engrave,
    Zero,
}

impl Group {
    pub const ALL: [Group; 6] = [Group::Select, Group::Profile, Group::Pocket, Group::Drill, Group::Engrave, Group::Zero];

    pub fn label(&self) -> &'static str {
        match self {
            Group::Select => "Zaznacz",
            Group::Profile => "Profil",
            Group::Pocket => "Kieszeń",
            Group::Drill => "Wiercenie",
            Group::Engrave => "Grawer",
            Group::Zero => "Zero",
        }
    }

    pub fn subs(&self) -> &'static [Sub] {
        use Sub::*;
        match self {
            Group::Select => &[Select, Move, Rotate, FlipH, FlipV, Scale, Align, Array],
            Group::Profile => &[ProfileOut, ProfileIn],
            Group::Pocket => &[PocketOffset, PocketZigzag, PocketAdaptive],
            Group::Drill => &[DrillSimple, DrillPeck, DrillHelix],
            Group::Engrave => &[EngCenter, EngLeft, EngRight, EngVcarve],
            Group::Zero => &[ZeroBL, ZeroCenter, ZeroTL, ZeroPick],
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            Group::Select => Color32::from_rgb(61, 127, 224),
            Group::Profile => Color32::from_rgb(80, 210, 230),
            Group::Pocket => Color32::from_rgb(220, 110, 230),
            Group::Drill => Color32::from_rgb(90, 220, 120),
            Group::Engrave => Color32::from_rgb(230, 150, 70),
            Group::Zero => Color32::WHITE,
        }
    }

    fn index(&self) -> usize {
        Group::ALL.iter().position(|g| g == self).unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sub {
    Select,
    Move,
    Rotate,
    FlipH,
    FlipV,
    Scale,
    Align,
    Array,
    ProfileOut,
    ProfileIn,
    PocketOffset,
    PocketZigzag,
    PocketAdaptive,
    DrillSimple,
    DrillPeck,
    DrillHelix,
    EngCenter,
    EngLeft,
    EngRight,
    EngVcarve,
    ZeroBL,
    ZeroCenter,
    ZeroTL,
    ZeroPick,
}

impl Sub {
    pub fn label(&self) -> &'static str {
        use Sub::*;
        match self {
            Select => "Zaznacz",
            Move => "Przesuń",
            Rotate => "Obróć",
            FlipH => "Odbij poziomo",
            FlipV => "Odbij pionowo",
            Scale => "Skaluj",
            Align => "Wyrównaj",
            Array => "Szyk",
            ProfileOut => "Zewnętrzny",
            ProfileIn => "Wewnętrzny",
            PocketOffset => "Offsetowa",
            PocketZigzag => "Zygzak",
            PocketAdaptive => "Adaptacyjna",
            DrillSimple => "Pojedyncze",
            DrillPeck => "Z wycofaniem",
            DrillHelix => "Frezowanie otworu",
            EngCenter => "Po ścieżce",
            EngLeft => "Z lewej",
            EngRight => "Z prawej",
            EngVcarve => "V-carve",
            ZeroBL => "Lewy dolny",
            ZeroCenter => "Środek",
            ZeroTL => "Lewy górny",
            ZeroPick => "Wskaż punkt",
        }
    }

    pub fn icon(&self) -> Icon {
        use Sub::*;
        match self {
            Select => Icon::Select,
            Move => Icon::Move,
            Rotate => Icon::Rotate,
            FlipH => Icon::FlipH,
            FlipV => Icon::FlipV,
            Scale => Icon::Scale,
            Align => Icon::Align,
            Array => Icon::Array,
            ProfileOut => Icon::ProfileOut,
            ProfileIn => Icon::ProfileIn,
            PocketOffset => Icon::PocketOffset,
            PocketZigzag => Icon::PocketZigzag,
            PocketAdaptive => Icon::PocketAdaptive,
            DrillSimple => Icon::DrillSimple,
            DrillPeck => Icon::DrillPeck,
            DrillHelix => Icon::DrillHelix,
            EngCenter => Icon::EngCenter,
            EngLeft => Icon::EngLeft,
            EngRight => Icon::EngRight,
            EngVcarve => Icon::EngVcarve,
            ZeroBL => Icon::ZeroBL,
            ZeroCenter => Icon::ZeroCenter,
            ZeroTL => Icon::ZeroTL,
            ZeroPick => Icon::ZeroPick,
        }
    }

    /// Jeszcze nie zaimplementowane.
    pub fn soon(&self) -> bool {
        matches!(self, Sub::PocketAdaptive | Sub::EngVcarve)
    }

    /// Polecenia wykonywane od razu po kliknięciu (nie zmieniają trybu).
    pub fn is_instant(&self) -> bool {
        matches!(self, Sub::FlipH | Sub::FlipV | Sub::ZeroBL | Sub::ZeroCenter | Sub::ZeroTL)
    }

    pub fn group(&self) -> Group {
        Group::ALL.into_iter().find(|g| g.subs().contains(self)).unwrap()
    }

    /// Szablon operacji: (rodzaj, domyślna głębokość, nazwa). `None` dla narzędzi edycji.
    pub fn op_template(&self, thickness: f64) -> Option<(OpKind, f64, &'static str)> {
        use Sub::*;
        let profile = |side| {
            OpKind::Profile(ProfileParams {
                side,
                tabs: (side == ProfileSide::Outside).then(|| Tabs { count: 4, width: 6.0, height: 2.0_f64.min(thickness * 0.3) }),
                ..Default::default()
            })
        };
        let engrave = |side| OpKind::Engrave(EngraveParams { side, ..Default::default() });
        Some(match self {
            ProfileOut => (profile(ProfileSide::Outside), thickness, "Profil zewn."),
            ProfileIn => (profile(ProfileSide::Inside), thickness, "Profil wewn."),
            PocketOffset => (OpKind::Pocket(PocketParams::default()), 3.0_f64.min(thickness), "Kieszeń"),
            PocketZigzag => (
                OpKind::Pocket(PocketParams { strategy: PocketStrategy::Zigzag, ..Default::default() }),
                3.0_f64.min(thickness),
                "Kieszeń zygzak",
            ),
            DrillSimple => (OpKind::Drill(DrillParams { peck: None, helix: false }), thickness, "Wiercenie"),
            DrillPeck => (OpKind::Drill(DrillParams { peck: Some(3.0), helix: false }), thickness, "Wiercenie z wycofaniem"),
            DrillHelix => (OpKind::Drill(DrillParams { peck: None, helix: true }), thickness, "Frezowanie otworów"),
            EngCenter => (engrave(EngraveSide::Center), 0.5_f64.min(thickness), "Grawer"),
            EngLeft => (engrave(EngraveSide::Left), 0.5_f64.min(thickness), "Grawer z lewej"),
            EngRight => (engrave(EngraveSide::Right), 0.5_f64.min(thickness), "Grawer z prawej"),
            _ => return None,
        })
    }

    /// Wariant paska odpowiadający istniejącej operacji.
    pub fn for_op(kind: &OpKind) -> Sub {
        match kind {
            OpKind::Profile(p) if p.side == ProfileSide::Inside => Sub::ProfileIn,
            OpKind::Profile(_) => Sub::ProfileOut,
            OpKind::Pocket(p) if p.strategy == PocketStrategy::Zigzag => Sub::PocketZigzag,
            OpKind::Pocket(_) => Sub::PocketOffset,
            OpKind::Drill(p) if p.helix => Sub::DrillHelix,
            OpKind::Drill(p) if p.peck.is_some() => Sub::DrillPeck,
            OpKind::Drill(_) => Sub::DrillSimple,
            OpKind::Engrave(p) => match p.side {
                EngraveSide::Center => Sub::EngCenter,
                EngraveSide::Left => Sub::EngLeft,
                EngraveSide::Right => Sub::EngRight,
            },
        }
    }
}

pub struct Toolbar {
    pub group: Group,
    chosen: [Sub; 6],
}

impl Default for Toolbar {
    fn default() -> Self {
        Self {
            group: Group::Select,
            chosen: [Sub::Select, Sub::ProfileOut, Sub::PocketOffset, Sub::DrillSimple, Sub::EngCenter, Sub::ZeroPick],
        }
    }
}

pub enum ToolbarAction {
    /// Zmienił się aktywny tryb.
    Changed,
    /// Polecenie natychmiastowe (odbicie, ustawienie zera).
    Command(Sub),
    Preview,
}

impl Toolbar {
    pub fn current(&self) -> Sub {
        self.chosen[self.group.index()]
    }

    pub fn set(&mut self, sub: Sub) {
        self.group = sub.group();
        self.chosen[self.group.index()] = sub;
    }

    pub fn reset(&mut self) {
        self.set(Sub::Select);
    }

    /// Kolumna 1 — grupy.
    pub fn groups_ui(&mut self, ui: &mut egui::Ui) -> Option<ToolbarAction> {
        let mut action = None;
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for g in Group::ALL {
            if g == Group::Zero {
                ui.separator();
            }
            let icon = self.chosen[g.index()].icon();
            let active = g == self.group;
            let r = icons::tool_button(ui, icon, g.label(), active, true, Color32::from_rgb(61, 127, 224));
            if active {
                // znacznik wskazujący kolumnę wariantów
                let c = egui::pos2(r.rect.right() + 1.0, r.rect.center().y);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![c + egui::vec2(0.0, -6.0), c + egui::vec2(6.0, 0.0), c + egui::vec2(0.0, 6.0)],
                    Color32::from_rgb(61, 127, 224),
                    egui::Stroke::NONE,
                ));
            }
            if r.clicked() {
                self.group = g;
                action = Some(ToolbarAction::Changed);
            }
        }
        ui.separator();
        if icons::tool_button(ui, Icon::Preview3D, "Podgląd 3D", false, true, Color32::WHITE)
            .on_hover_text("Wizualizacja 3D (Tab)")
            .clicked()
        {
            action = Some(ToolbarAction::Preview);
        }
        action
    }

    /// Kolumna 2 — warianty aktywnej grupy.
    pub fn subs_ui(&mut self, ui: &mut egui::Ui) -> Option<ToolbarAction> {
        let mut action = None;
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(self.group.label().to_uppercase()).small().strong().color(Color32::from_gray(160)));
        });
        ui.add_space(2.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        let accent = self.group.color();
        for &s in self.group.subs() {
            let active = self.current() == s && !s.is_instant();
            let r = icons::tool_button(ui, s.icon(), s.label(), active, !s.soon(), accent);
            if r.clicked() && !s.soon() {
                if s.is_instant() {
                    action = Some(ToolbarAction::Command(s));
                } else {
                    self.chosen[self.group.index()] = s;
                    action = Some(ToolbarAction::Changed);
                }
            }
        }
        action
    }
}
