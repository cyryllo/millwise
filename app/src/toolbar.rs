//! Pasek narzędzi po lewej: grupy (kolumna 1) i ich warianty (kolumna 2).

use cam_i18n::tr;
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
    /// Obróbka modelu 3D (STL).
    Model3D,
    Zero,
}

impl Group {
    pub const ALL: [Group; 7] = [Group::Select, Group::Profile, Group::Pocket, Group::Drill, Group::Engrave, Group::Model3D, Group::Zero];

    pub fn label(&self) -> &'static str {
        match self {
            Group::Select => tr!("Zaznacz"),
            Group::Profile => tr!("Profil"),
            Group::Pocket => tr!("Kieszeń"),
            Group::Drill => tr!("Wiercenie"),
            Group::Engrave => tr!("Grawer"),
            Group::Model3D => "3D",
            Group::Zero => tr!("Zero"),
        }
    }

    pub fn subs(&self) -> &'static [Sub] {
        use Sub::*;
        match self {
            Group::Select => &[Select, Move, Rotate, FlipH, FlipV, Scale, Align, Array, Measure],
            Group::Profile => &[ProfileOut, ProfileIn, Corners],
            Group::Pocket => &[PocketOffset, PocketZigzag, PocketAdaptive],
            Group::Drill => &[DrillSimple, DrillPeck, DrillHelix],
            Group::Engrave => &[EngCenter, EngLeft, EngRight, EngVcarve, Text],
            Group::Model3D => &[Rough3D, Finish3D, Cutout3D],
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
            Group::Model3D => Color32::from_rgb(240, 200, 90),
            Group::Zero => Color32::WHITE,
        }
    }

    /// Czy grupa jest w zakładce „Model 3D” (inne potrzebują rysunku 2D).
    pub fn in_3d_view(&self) -> bool {
        matches!(self, Group::Model3D | Group::Zero)
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
    /// Pomiar odległości między dwoma punktami.
    Measure,
    ProfileOut,
    ProfileIn,
    /// Uszka w narożnikach istniejących operacji wewnętrznych (klik krzywej).
    Corners,
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
    /// Napis: tekst zamieniany na krzywe.
    Text,
    /// Obróbka zgrubna 3D warstwami Z.
    Rough3D,
    /// Obróbka wykańczająca 3D (przejścia równoległe).
    Finish3D,
    /// Wycięcie detalu po sylwetce modelu (profil zewnętrzny z mostkami).
    Cutout3D,
    ZeroBL,
    ZeroCenter,
    ZeroTL,
    ZeroPick,
}

impl Sub {
    pub fn label(&self) -> &'static str {
        use Sub::*;
        match self {
            Select => tr!("Zaznacz"),
            Move => tr!("Przesuń"),
            Rotate => tr!("Obróć"),
            FlipH => tr!("Odbij poziomo"),
            FlipV => tr!("Odbij pionowo"),
            Scale => tr!("Skaluj"),
            Align => tr!("Wyrównaj"),
            Array => tr!("Szyk"),
            Measure => tr!("Zmierz"),
            ProfileOut => tr!("Zewnętrzny"),
            ProfileIn => tr!("Wewnętrzny"),
            Corners => tr!("Narożniki"),
            PocketOffset => tr!("Offsetowa"),
            PocketZigzag => tr!("Zygzak"),
            PocketAdaptive => tr!("Adaptacyjna"),
            DrillSimple => tr!("Pojedyncze"),
            DrillPeck => tr!("Z wycofaniem"),
            DrillHelix => tr!("Frezowanie otworu"),
            EngCenter => tr!("Po ścieżce"),
            EngLeft => tr!("Z lewej"),
            EngRight => tr!("Z prawej"),
            EngVcarve => "V-carve",
            Text => tr!("Tekst"),
            Rough3D => tr!("Zgrubna"),
            Finish3D => tr!("Wykańczająca"),
            Cutout3D => tr!("Wycięcie"),
            ZeroBL => tr!("Lewy dolny"),
            ZeroCenter => tr!("Środek"),
            ZeroTL => tr!("Lewy górny"),
            ZeroPick => tr!("Wskaż punkt"),
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
            Measure => Icon::Measure,
            ProfileOut => Icon::ProfileOut,
            ProfileIn => Icon::ProfileIn,
            Corners => Icon::Corners,
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
            Text => Icon::Text,
            Rough3D => Icon::Rough3D,
            Finish3D => Icon::Finish3D,
            Cutout3D => Icon::Cutout3D,
            ZeroBL => Icon::ZeroBL,
            ZeroCenter => Icon::ZeroCenter,
            ZeroTL => Icon::ZeroTL,
            ZeroPick => Icon::ZeroPick,
        }
    }

    /// Jeszcze nie zaimplementowane.
    pub fn soon(&self) -> bool {
        false
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
            ProfileOut => (profile(ProfileSide::Outside), thickness, tr!("Profil zewn.")),
            ProfileIn => (profile(ProfileSide::Inside), thickness, tr!("Profil wewn.")),
            PocketOffset => (OpKind::Pocket(PocketParams::default()), 3.0_f64.min(thickness), tr!("Kieszeń")),
            PocketZigzag => (
                OpKind::Pocket(PocketParams { strategy: PocketStrategy::Zigzag, ..Default::default() }),
                3.0_f64.min(thickness),
                tr!("Kieszeń zygzak"),
            ),
            PocketAdaptive => (
                OpKind::Pocket(PocketParams { strategy: PocketStrategy::Adaptive, stepover: 0.15, ..Default::default() }),
                3.0_f64.min(thickness),
                tr!("Kieszeń HSM"),
            ),
            DrillSimple => (OpKind::Drill(DrillParams { peck: None, helix: false }), thickness, tr!("Wiercenie")),
            DrillPeck => (OpKind::Drill(DrillParams { peck: Some(3.0), helix: false }), thickness, tr!("Wiercenie z wycofaniem")),
            DrillHelix => (OpKind::Drill(DrillParams { peck: None, helix: true }), thickness, tr!("Frezowanie otworów")),
            EngCenter => (engrave(EngraveSide::Center), 0.5_f64.min(thickness), tr!("Grawer")),
            EngLeft => (engrave(EngraveSide::Left), 0.5_f64.min(thickness), tr!("Grawer z lewej")),
            EngRight => (engrave(EngraveSide::Right), 0.5_f64.min(thickness), tr!("Grawer z prawej")),
            EngVcarve => (OpKind::VCarve(Default::default()), 3.0_f64.min(thickness), "V-carving"),
            Rough3D => (OpKind::Rough3D(Default::default()), thickness, tr!("Zgrubna 3D")),
            Finish3D => (OpKind::Finish3D(Default::default()), thickness, tr!("Wykańczająca 3D")),
            Cutout3D => (profile(ProfileSide::Outside), thickness, tr!("Wycięcie modelu")),
            _ => return None,
        })
    }

    /// Wariant paska odpowiadający istniejącej operacji.
    pub fn for_op(kind: &OpKind) -> Sub {
        match kind {
            OpKind::Profile(p) if p.side == ProfileSide::Inside => Sub::ProfileIn,
            OpKind::Profile(_) => Sub::ProfileOut,
            OpKind::Pocket(p) if p.strategy == PocketStrategy::Zigzag => Sub::PocketZigzag,
            OpKind::Pocket(p) if p.strategy == PocketStrategy::Adaptive => Sub::PocketAdaptive,
            OpKind::Pocket(_) => Sub::PocketOffset,
            OpKind::Drill(p) if p.helix => Sub::DrillHelix,
            OpKind::Drill(p) if p.peck.is_some() => Sub::DrillPeck,
            OpKind::Drill(_) => Sub::DrillSimple,
            OpKind::Engrave(p) => match p.side {
                EngraveSide::Center => Sub::EngCenter,
                EngraveSide::Left => Sub::EngLeft,
                EngraveSide::Right => Sub::EngRight,
            },
            OpKind::VCarve(_) => Sub::EngVcarve,
            OpKind::Rough3D(_) => Sub::Rough3D,
            OpKind::Finish3D(_) => Sub::Finish3D,
        }
    }
}

pub struct Toolbar {
    pub group: Group,
    chosen: [Sub; 7],
}

impl Default for Toolbar {
    fn default() -> Self {
        Self {
            group: Group::Select,
            chosen: [Sub::Select, Sub::ProfileOut, Sub::PocketOffset, Sub::DrillSimple, Sub::EngCenter, Sub::Rough3D, Sub::ZeroPick],
        }
    }
}

pub enum ToolbarAction {
    /// Zmienił się aktywny tryb.
    Changed,
    /// Polecenie natychmiastowe (odbicie, ustawienie zera).
    Command(Sub),
    /// Wybrany wariant tworzy operację od razu (obróbka 3D).
    Create,
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

    /// Kolumna 1 — grupy (`only3d`: zakładka modelu — tylko to, co da się tam użyć).
    pub fn groups_ui(&mut self, ui: &mut egui::Ui, only3d: bool) -> Option<ToolbarAction> {
        let mut action = None;
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for g in Group::ALL {
            if only3d && !g.in_3d_view() {
                continue;
            }
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
                    // operacje 3D powstają od razu (krzywe są tylko opcjonalną granicą)
                    action = Some(if s.group() == Group::Model3D { ToolbarAction::Create } else { ToolbarAction::Changed });
                }
            }
        }
        action
    }
}
