//! Okno biblioteki narzędzi: lista, wizualny wybór rodzaju freza, rysunek z wymiarami,
//! parametry skrawania dla materiałów.

use crate::icons;
use cam_ops::{CutPreset, FluteDir, Tool, ToolKind, ToolLibrary};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, StrokeKind};

const ACCENT: Color32 = Color32::from_rgb(61, 127, 224);
const DIM_D: Color32 = Color32::from_rgb(90, 220, 120);
const DIM_L: Color32 = Color32::from_rgb(240, 180, 80);

#[derive(Default)]
pub struct ToolsWindow {
    pub open: bool,
    selected: Option<u32>,
    /// Edytowany zestaw parametrów: None = domyślne, Some(i) = presets[i].
    preset: Option<usize>,
}

fn kind_options() -> [ToolKind; 6] {
    [
        ToolKind::FlatEnd,
        ToolKind::BallEnd,
        ToolKind::BullNose { corner_radius: 1.0 },
        ToolKind::VBit { angle: 90.0, tip_diameter: 0.0 },
        ToolKind::Engraver { angle: 30.0, tip_diameter: 0.2 },
        ToolKind::Drill { point_angle: 118.0 },
    ]
}

fn kind_short(k: &ToolKind) -> &'static str {
    match k {
        ToolKind::FlatEnd => "Płaski",
        ToolKind::BallEnd => "Kulowy",
        ToolKind::BullNose { .. } => "Z promieniem",
        ToolKind::VBit { .. } => "V (fazowanie)",
        ToolKind::Engraver { .. } => "Grawerski",
        ToolKind::Drill { .. } => "Wiertło",
    }
}

fn same_kind(a: &ToolKind, b: &ToolKind) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

impl ToolsWindow {
    /// Zwraca true, jeśli biblioteka została zmieniona.
    pub fn show(&mut self, ctx: &egui::Context, lib: &mut ToolLibrary, used: &[u32]) -> bool {
        if !self.open {
            return false;
        }
        if self.selected.is_none_or(|id| lib.get(id).is_none()) {
            self.selected = lib.tools.first().map(|t| t.id);
            self.preset = None;
        }
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("Biblioteka narzędzi")
            .open(&mut open)
            .default_size([1120.0, 680.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("+ Nowy").strong()).clicked() {
                        let t = Tool { number: lib.next_number(), name: "Nowy frez".into(), ..Default::default() };
                        self.selected = Some(lib.add(t));
                        self.preset = None;
                        changed = true;
                    }
                    if let Some(id) = self.selected {
                        if ui.button("Duplikuj").clicked() {
                            if let Some(t) = lib.get(id).cloned() {
                                let t = Tool { number: lib.next_number(), name: format!("{} (kopia)", t.name), ..t };
                                self.selected = Some(lib.add(t));
                                changed = true;
                            }
                        }
                        let in_use = used.contains(&id);
                        if ui
                            .add_enabled(!in_use, egui::Button::new("Usuń"))
                            .on_disabled_hover_text("Narzędzie jest używane przez operację")
                            .clicked()
                        {
                            lib.remove(id);
                            self.selected = lib.tools.first().map(|t| t.id);
                            self.preset = None;
                            changed = true;
                        }
                    }
                    if let Some(p) = crate::storage::tools_path() {
                        ui.label(RichText::new(format!("{} · wspólna dla wszystkich projektów", p.display())).small().weak());
                    }
                });
                ui.separator();

                ui.horizontal_top(|ui| {
                    // --- lista ---
                    ui.vertical(|ui| {
                        ui.set_width(250.0);
                        egui::ScrollArea::vertical().id_salt("tool_list").max_height(ui.available_height()).show(ui, |ui| {
                            let mut sorted: Vec<&Tool> = lib.tools.iter().collect();
                            sorted.sort_by_key(|t| t.number);
                            for t in sorted {
                                if tool_row(ui, t, self.selected == Some(t.id)).clicked() {
                                    self.selected = Some(t.id);
                                    self.preset = None;
                                }
                            }
                        });
                    });
                    ui.separator();
                    let Some(t) = self.selected.and_then(|id| lib.get_mut(id)) else {
                        ui.label("Wybierz narzędzie z listy.");
                        return;
                    };
                    // --- rodzaj + rysunek ---
                    ui.vertical(|ui| {
                        ui.set_width(370.0);
                        ui.strong("Rodzaj freza");
                        egui::Grid::new("kinds").spacing([8.0, 8.0]).show(ui, |ui| {
                            for (n, k) in kind_options().into_iter().enumerate() {
                                let active = same_kind(&k, &t.kind);
                                if kind_button(ui, &k, active).clicked() && !active {
                                    t.kind = k;
                                    changed = true;
                                }
                                if n % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                        ui.add_space(8.0);
                        big_drawing(ui, t);
                    });
                    ui.separator();
                    // --- parametry ---
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical().id_salt("tool_params").show(ui, |ui| {
                            changed |= params(ui, t, &mut self.preset);
                        });
                    });
                });
            });
        self.open = open;
        changed
    }
}

fn tool_row(ui: &mut egui::Ui, t: &Tool, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), egui::Sense::click());
    let bg = if active {
        Color32::from_rgb(31, 51, 80)
    } else if resp.hovered() {
        Color32::from_rgb(40, 44, 50)
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect(rect, 6.0, bg, Stroke::new(1.0, if active { ACCENT } else { Color32::TRANSPARENT }), StrokeKind::Inside);
    let ir = Rect::from_min_size(rect.min + egui::vec2(6.0, 4.0), egui::vec2(22.0, 46.0));
    p.rect_filled(ir, 3.0, Color32::from_rgb(27, 29, 33));
    icons::draw_tool(p, ir.shrink(2.0), &t.kind);
    p.text(
        Pos2::new(ir.right() + 10.0, rect.top() + 9.0),
        egui::Align2::LEFT_TOP,
        format!("T{} · {}", t.number, t.name),
        egui::FontId::proportional(13.0),
        Color32::from_gray(232),
    );
    p.text(
        Pos2::new(ir.right() + 10.0, rect.top() + 29.0),
        egui::Align2::LEFT_TOP,
        format!("Ø {} mm · {}", trim(t.diameter), kind_short(&t.kind)),
        egui::FontId::proportional(12.0),
        Color32::from_gray(155),
    );
    resp
}

fn kind_button(ui: &mut egui::Ui, k: &ToolKind, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(114.0, 124.0), egui::Sense::click());
    let bg = if active {
        Color32::from_rgb(31, 51, 80)
    } else if resp.hovered() {
        Color32::from_rgb(45, 49, 56)
    } else {
        Color32::from_rgb(39, 42, 48)
    };
    let stroke = Stroke::new(if active { 1.5 } else { 1.0 }, if active { ACCENT } else { Color32::from_rgb(52, 57, 64) });
    let p = ui.painter();
    p.rect(rect, 8.0, bg, stroke, StrokeKind::Inside);
    let ir = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 50.0), egui::vec2(38.0, 88.0));
    icons::draw_tool(p, ir, k);
    p.text(
        Pos2::new(rect.center().x, rect.bottom() - 8.0),
        egui::Align2::CENTER_BOTTOM,
        kind_short(k),
        egui::FontId::proportional(12.0),
        if active { Color32::WHITE } else { Color32::from_gray(200) },
    );
    resp
}

/// Duży rysunek freza z wymiarami Ø i długości ostrzy.
fn big_drawing(ui: &mut egui::Ui, t: &Tool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 300.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect(rect, 8.0, Color32::from_rgb(27, 29, 33), Stroke::new(1.0, Color32::from_rgb(44, 48, 54)), StrokeKind::Inside);
    let draw_rect = Rect::from_center_size(rect.center() - egui::vec2(10.0, 12.0), egui::vec2(100.0, 250.0));
    let (cut, whole) = icons::draw_tool(p, draw_rect, &t.kind);
    // Ø
    let y = whole.bottom() + 10.0;
    let st = Stroke::new(1.2, DIM_D);
    p.line_segment([Pos2::new(cut.left(), y), Pos2::new(cut.right(), y)], st);
    p.line_segment([Pos2::new(cut.left(), y - 5.0), Pos2::new(cut.left(), y + 5.0)], st);
    p.line_segment([Pos2::new(cut.right(), y - 5.0), Pos2::new(cut.right(), y + 5.0)], st);
    p.text(Pos2::new(cut.center().x, y + 7.0), egui::Align2::CENTER_TOP, format!("Ø {}", trim(t.diameter)), egui::FontId::proportional(13.0), DIM_D);
    // L
    let x = cut.right() + 22.0;
    let st = Stroke::new(1.2, DIM_L);
    p.line_segment([Pos2::new(x, cut.top()), Pos2::new(x, cut.bottom())], st);
    p.line_segment([Pos2::new(x - 5.0, cut.top()), Pos2::new(x + 5.0, cut.top())], st);
    p.line_segment([Pos2::new(x - 5.0, cut.bottom()), Pos2::new(x + 5.0, cut.bottom())], st);
    p.text(Pos2::new(x + 8.0, cut.center().y), egui::Align2::LEFT_CENTER, format!("L {}", trim(t.flute_length)), egui::FontId::proportional(13.0), DIM_L);
    // kąt / promień
    let extra = match t.kind {
        ToolKind::VBit { angle, .. } | ToolKind::Engraver { angle, .. } => Some(format!("{}°", trim(angle))),
        ToolKind::Drill { point_angle } => Some(format!("{}°", trim(point_angle))),
        ToolKind::BullNose { corner_radius } => Some(format!("R {}", trim(corner_radius))),
        _ => None,
    };
    if let Some(e) = extra {
        p.text(Pos2::new(cut.left() - 12.0, cut.bottom() - 10.0), egui::Align2::RIGHT_CENTER, e, egui::FontId::proportional(13.0), Color32::from_rgb(127, 176, 255));
    }
}

fn row(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str, color: Option<Color32>) -> bool {
    match color {
        Some(c) => ui.label(RichText::new(label).color(c)),
        None => ui.label(label),
    };
    let c = ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(3)).changed();
    ui.end_row();
    c
}

fn params(ui: &mut egui::Ui, t: &mut Tool, preset: &mut Option<usize>) -> bool {
    let mut changed = false;
    egui::Grid::new("tool_general").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Numer (T)");
        let mut n = t.number as f64;
        if ui.add(egui::DragValue::new(&mut n).range(0.0..=999.0).speed(0.1).max_decimals(0)).changed() {
            t.number = n as u32;
            changed = true;
        }
        ui.end_row();
        ui.label("Nazwa");
        changed |= ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(240.0)).changed();
        ui.end_row();
    });

    ui.add_space(6.0);
    ui.strong("Geometria");
    egui::Grid::new("tool_geom").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        changed |= row(ui, "Średnica", &mut t.diameter, 0.01..=200.0, 0.01, " mm", Some(DIM_D));
        changed |= row(ui, "Długość ostrzy", &mut t.flute_length, 0.1..=300.0, 0.1, " mm", Some(DIM_L));
        changed |= row(ui, "Średnica chwytu", &mut t.shank_diameter, 0.1..=100.0, 0.01, " mm", None);
        ui.label("Liczba ostrzy");
        let mut f = t.flutes as f64;
        if ui.add(egui::DragValue::new(&mut f).range(1.0..=12.0).speed(0.05).max_decimals(0)).changed() {
            t.flutes = f as u32;
            changed = true;
        }
        ui.end_row();
        match &mut t.kind {
            ToolKind::BullNose { corner_radius } => {
                changed |= row(ui, "Promień naroża", corner_radius, 0.0..=50.0, 0.05, " mm", None);
            }
            ToolKind::VBit { angle, tip_diameter } | ToolKind::Engraver { angle, tip_diameter } => {
                changed |= row(ui, "Kąt", angle, 5.0..=170.0, 0.5, "°", None);
                changed |= row(ui, "Średnica czubka", tip_diameter, 0.0..=10.0, 0.01, " mm", None);
            }
            ToolKind::Drill { point_angle } => {
                changed |= row(ui, "Kąt wierzchołkowy", point_angle, 60.0..=180.0, 0.5, "°", None);
            }
            _ => {}
        }
        if matches!(t.kind, ToolKind::FlatEnd | ToolKind::BallEnd | ToolKind::BullNose { .. }) {
            ui.label("Kierunek spirali");
            ui.horizontal(|ui| {
                for d in [FluteDir::Upcut, FluteDir::Downcut, FluteDir::Compression] {
                    changed |= ui.selectable_value(&mut t.flute_dir, d, d.label()).changed();
                }
            });
            ui.end_row();
        }
    });

    ui.add_space(8.0);
    ui.strong("Parametry skrawania dla materiału");
    if preset.is_some_and(|k| k >= t.presets.len()) {
        *preset = None;
    }
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(preset.is_none(), "Domyślne").clicked() {
            *preset = None;
        }
        for (k, p) in t.presets.iter().enumerate() {
            if ui.selectable_label(*preset == Some(k), &p.material).clicked() {
                *preset = Some(k);
            }
        }
        if t.presets.is_empty()
            && ui
                .button("Typowe materiały")
                .on_hover_text("Utwórz zestawy Drewno / MDF / Aluminium / Plexi wyliczone z parametrów domyślnych — sprawdź je przed użyciem")
                .clicked()
        {
            *t = t.clone().with_material_presets();
            changed = true;
        }
        if ui.button("+ materiał").clicked() {
            let mut p = t.base_preset();
            p.material = format!("Materiał {}", t.presets.len() + 1);
            t.presets.push(p);
            *preset = Some(t.presets.len() - 1);
            changed = true;
        }
    });

    let (d, flutes) = (t.diameter, t.flutes);
    let mut remove = false;
    let p: &mut CutPreset;
    let mut base_tmp;
    let editing_base = preset.is_none();
    if let Some(k) = *preset {
        p = &mut t.presets[k];
        ui.horizontal(|ui| {
            ui.label("Nazwa materiału");
            changed |= ui.add(egui::TextEdit::singleline(&mut p.material).desired_width(160.0)).changed();
            if ui.small_button("Usuń zestaw").clicked() {
                remove = true;
            }
        });
    } else {
        base_tmp = t.base_preset();
        p = &mut base_tmp;
    }
    let mut edited = false;
    egui::Grid::new("tool_cut").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        edited |= row(ui, "Obroty", &mut p.rpm, 0.0..=100000.0, 100.0, " obr/min", None);
        edited |= row(ui, "Posuw (XY)", &mut p.feed, 1.0..=50000.0, 10.0, " mm/min", None);
        edited |= row(ui, "Posuw wgłębny (Z)", &mut p.plunge_feed, 1.0..=50000.0, 5.0, " mm/min", None);
        edited |= row(ui, "Krok w dół", &mut p.step_down, 0.01..=100.0, 0.05, " mm", None);
        let mut pct = p.stepover * 100.0;
        ui.label("Stepover");
        if ui.add(egui::DragValue::new(&mut pct).range(1.0..=100.0).speed(0.5).suffix(" % Ø")).changed() {
            p.stepover = pct / 100.0;
            edited = true;
        }
        ui.end_row();
    });
    let fz = if flutes > 0 && p.rpm > 0.0 { p.feed / (p.rpm * flutes as f64) } else { 0.0 };
    let vc = std::f64::consts::PI * d * p.rpm / 1000.0;
    ui.horizontal(|ui| {
        stat(ui, "Posuw na ząb (fz)", format!("{fz:.3} mm"));
        stat(ui, "Prędkość skrawania (Vc)", format!("{vc:.0} m/min"));
    });
    if editing_base && edited {
        let b = p.clone();
        t.rpm = b.rpm;
        t.feed = b.feed;
        t.plunge_feed = b.plunge_feed;
        t.step_down = b.step_down;
        t.stepover = b.stepover;
    }
    changed |= edited;
    if remove {
        if let Some(k) = preset.take() {
            t.presets.remove(k);
            changed = true;
        }
    }

    ui.add_space(8.0);
    ui.label("Notatki");
    changed |= ui.add(egui::TextEdit::multiline(&mut t.notes).desired_rows(3).desired_width(f32::INFINITY)).changed();
    changed
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::new().fill(Color32::from_rgb(27, 29, 33)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).small().weak());
            ui.label(RichText::new(value).monospace().size(15.0));
        });
    });
}

fn trim(v: f64) -> String {
    let t = format!("{v:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}
