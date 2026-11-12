//! Okno „Maszyny”: lista profili, podgląd pola roboczego, parametry w zakładkach.

use cam_post::{Controller, HomeCorner, Machine, MachineLibrary, SpindleKind, ToolChange};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, StrokeKind};

const ACCENT: Color32 = Color32::from_rgb(61, 127, 224);

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    Motion,
    Spindle,
    Gcode,
}

#[derive(Default)]
pub struct MachinesWindow {
    pub open: bool,
    selected: Option<u32>,
    tab: Tab,
}

impl MachinesWindow {
    /// `part` — wymiary półfabrykatu (do porównania z polem roboczym).
    /// Zwraca true, jeśli biblioteka maszyn się zmieniła.
    pub fn show(&mut self, ctx: &egui::Context, lib: &mut MachineLibrary, part: Option<(f64, f64)>) -> bool {
        if !self.open {
            return false;
        }
        if self.selected.is_none_or(|id| lib.get(id).is_none()) {
            self.selected = lib.active().map(|m| m.id);
        }
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("Maszyny")
            .open(&mut open)
            .default_size([1120.0, 640.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("+ Nowa").strong()).clicked() {
                        self.selected = Some(lib.add(Machine { name: "Nowa maszyna".into(), ..Default::default() }));
                        changed = true;
                    }
                    if let Some(id) = self.selected {
                        if ui.button("Duplikuj").clicked() {
                            if let Some(m) = lib.get(id).cloned() {
                                self.selected = Some(lib.add(Machine { name: format!("{} (kopia)", m.name), ..m }));
                                changed = true;
                            }
                        }
                        if ui.add_enabled(lib.machines.len() > 1, egui::Button::new("Usuń")).clicked() {
                            lib.remove(id);
                            self.selected = lib.active().map(|m| m.id);
                            changed = true;
                        }
                        let is_active = lib.active == id;
                        if ui
                            .add_enabled(!is_active, egui::Button::new(RichText::new("Ustaw jako aktywną").color(Color32::from_rgb(140, 230, 160))))
                            .clicked()
                        {
                            lib.active = id;
                            changed = true;
                        }
                    }
                    if let Some(p) = crate::storage::machines_path() {
                        ui.label(RichText::new(p.display().to_string()).small().weak());
                    }
                });
                ui.separator();

                ui.horizontal_top(|ui| {
                    // --- lista ---
                    ui.vertical(|ui| {
                        ui.set_width(240.0);
                        for m in &lib.machines {
                            if machine_row(ui, m, self.selected == Some(m.id), lib.active == m.id).clicked() {
                                self.selected = Some(m.id);
                            }
                        }
                    });
                    ui.separator();
                    let Some(m) = self.selected.and_then(|id| lib.get_mut(id)) else { return };
                    // --- pole robocze ---
                    ui.vertical(|ui| {
                        ui.set_width(330.0);
                        ui.strong("Pole robocze (widok z góry)");
                        work_area(ui, m, part);
                        ui.label(RichText::new("Położenie bazowania (home)").weak());
                        egui::Grid::new("home").spacing([6.0, 6.0]).show(ui, |ui| {
                            for (n, h) in HomeCorner::ALL.into_iter().enumerate() {
                                let b = egui::Button::new(h.label())
                                    .selected(m.home == h)
                                    .min_size(egui::vec2(150.0, 28.0));
                                if ui.add(b).clicked() {
                                    m.home = h;
                                    changed = true;
                                }
                                if n % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                    ui.separator();
                    // --- zakładki ---
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for (t, l) in [
                                (Tab::General, "Ogólne i pole robocze"),
                                (Tab::Motion, "Ruch"),
                                (Tab::Spindle, "Wrzeciono i narzędzia"),
                                (Tab::Gcode, "G-code"),
                            ] {
                                ui.selectable_value(&mut self.tab, t, RichText::new(l).strong());
                            }
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().id_salt("machine_params").show(ui, |ui| {
                            changed |= match self.tab {
                                Tab::General => general(ui, m),
                                Tab::Motion => motion(ui, m),
                                Tab::Spindle => spindle(ui, m),
                                Tab::Gcode => gcode(ui, m),
                            };
                        });
                    });
                });
            });
        self.open = open;
        changed
    }
}

fn machine_row(ui: &mut egui::Ui, m: &Machine, selected: bool, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
    let p = ui.painter();
    let bg = if selected { Color32::from_rgb(31, 51, 80) } else { Color32::from_rgb(37, 40, 45) };
    p.rect(rect, 8.0, bg, Stroke::new(1.0, if selected { ACCENT } else { Color32::from_rgb(52, 57, 64) }), StrokeKind::Inside);
    p.text(rect.min + egui::vec2(10.0, 9.0), egui::Align2::LEFT_TOP, &m.name, egui::FontId::proportional(13.0), Color32::from_gray(235));
    p.text(rect.min + egui::vec2(10.0, 29.0), egui::Align2::LEFT_TOP, m.summary() + " mm", egui::FontId::proportional(12.0), Color32::from_gray(155));
    if active {
        let r = Rect::from_min_size(Pos2::new(rect.right() - 66.0, rect.top() + 8.0), egui::vec2(58.0, 16.0));
        p.rect_filled(r, 4.0, Color32::from_rgb(90, 220, 120));
        p.text(r.center(), egui::Align2::CENTER_CENTER, "AKTYWNA", egui::FontId::proportional(10.0), Color32::from_rgb(15, 42, 18));
    }
    ui.add_space(4.0);
    resp
}

fn work_area(ui: &mut egui::Ui, m: &Machine, part: Option<(f64, f64)>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 260.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect(rect, 8.0, Color32::from_rgb(27, 29, 33), Stroke::new(1.0, Color32::from_rgb(44, 48, 54)), StrokeKind::Inside);
    let inner = rect.shrink2(egui::vec2(36.0, 34.0));
    let k = (inner.width() as f64 / m.travel_x.max(1.0)).min(inner.height() as f64 / m.travel_y.max(1.0)) as f32;
    let size = egui::vec2(m.travel_x as f32 * k, m.travel_y as f32 * k);
    let area = Rect::from_center_size(inner.center(), size);
    p.rect(area, 0.0, Color32::from_rgb(33, 36, 41), Stroke::new(2.0, ACCENT), StrokeKind::Middle);
    // detal w lewym przednim rogu, w skali
    if let Some((w, h)) = part {
        let pr = Rect::from_min_size(
            Pos2::new(area.left() + 6.0, area.bottom() - 6.0 - h as f32 * k),
            egui::vec2(w as f32 * k, h as f32 * k),
        );
        let fits = w <= m.travel_x && h <= m.travel_y;
        let c = if fits { Color32::from_rgb(52, 120, 220) } else { Color32::from_rgb(220, 80, 70) };
        p.rect(pr, 3.0, c.gamma_multiply(0.35), Stroke::new(1.0, c), StrokeKind::Middle);
    }
    let home = match m.home {
        HomeCorner::FrontLeft => area.left_bottom(),
        HomeCorner::FrontRight => area.right_bottom(),
        HomeCorner::BackLeft => area.left_top(),
        HomeCorner::BackRight => area.right_top(),
    };
    p.circle_filled(home, 7.0, Color32::from_rgb(240, 180, 80));
    p.circle_stroke(home, 7.0, Stroke::new(1.5, Color32::from_rgb(17, 19, 22)));
    let dim = Color32::from_gray(190);
    p.text(Pos2::new(area.center().x, area.top() - 8.0), egui::Align2::CENTER_BOTTOM, format!("X {} mm", m.travel_x), egui::FontId::monospace(12.0), dim);
    p.text(Pos2::new(area.left() - 8.0, area.center().y), egui::Align2::RIGHT_CENTER, format!("Y\n{}", m.travel_y), egui::FontId::monospace(12.0), dim);
    p.text(rect.right_bottom() - egui::vec2(10.0, 8.0), egui::Align2::RIGHT_BOTTOM, format!("Z {} mm", m.travel_z), egui::FontId::proportional(11.0), Color32::from_gray(150));
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> bool {
    ui.label(label);
    let c = ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(3)).changed();
    ui.end_row();
    c
}

fn general(ui: &mut egui::Ui, m: &mut Machine) -> bool {
    let mut ch = false;
    egui::Grid::new("m_general").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Nazwa");
        ch |= ui.add(egui::TextEdit::singleline(&mut m.name).desired_width(260.0)).changed();
        ui.end_row();
        ui.label("Sterownik (postprocesor)");
        ui.horizontal(|ui| {
            for c in Controller::ALL {
                if ui.selectable_value(&mut m.controller, c, c.label()).changed() {
                    m.extension = c.dialect().extension().into();
                    ch = true;
                }
            }
        });
        ui.end_row();
        ch |= num(ui, "Zakres X", &mut m.travel_x, 1.0..=10000.0, 1.0, " mm");
        ch |= num(ui, "Zakres Y", &mut m.travel_y, 1.0..=10000.0, 1.0, " mm");
        ch |= num(ui, "Zakres Z", &mut m.travel_z, 1.0..=2000.0, 1.0, " mm");
        ch |= num(ui, "Maks. wysunięcie narzędzia", &mut m.max_tool_length, 1.0..=500.0, 1.0, " mm");
        ch |= num(ui, "Wys. bezpieczna (domyślna)", &mut m.safe_z, 0.5..=200.0, 0.1, " mm");
        ui.label("Układ współrzędnych");
        egui::ComboBox::from_id_salt("wcs").selected_text(m.wcs.clone()).show_ui(ui, |ui| {
            for w in ["G54", "G55", "G56", "G57", "G58", "G59"] {
                ch |= ui.selectable_value(&mut m.wcs, w.to_string(), w).changed();
            }
        });
        ui.end_row();
    });
    ch
}

fn motion(ui: &mut egui::Ui, m: &mut Machine) -> bool {
    let mut ch = false;
    ui.label(RichText::new("Do szacowania czasu i ostrzeżeń o zbyt dużych posuwach.").weak());
    egui::Grid::new("m_motion").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("");
        ui.strong("X / Y");
        ui.strong("Z");
        ui.end_row();
        let mut pair = |ui: &mut egui::Ui, label: &str, a: &mut f64, b: &mut f64, suffix: &str| {
            ui.label(label);
            ch |= ui.add(egui::DragValue::new(a).range(1.0..=100000.0).speed(10.0).suffix(suffix)).changed();
            ch |= ui.add(egui::DragValue::new(b).range(1.0..=100000.0).speed(10.0).suffix(suffix)).changed();
            ui.end_row();
        };
        pair(ui, "Maks. posuw roboczy", &mut m.max_feed_xy, &mut m.max_feed_z, " mm/min");
        pair(ui, "Ruchy szybkie (G0)", &mut m.rapid_xy, &mut m.rapid_z, " mm/min");
        pair(ui, "Przyspieszenie", &mut m.accel_xy, &mut m.accel_z, " mm/s²");
    });
    ch
}

fn spindle(ui: &mut egui::Ui, m: &mut Machine) -> bool {
    let mut ch = false;
    ui.strong("Wrzeciono");
    ui.horizontal(|ui| {
        for s in [SpindleKind::Manual, SpindleKind::Pwm, SpindleKind::Vfd] {
            ch |= ui.selectable_value(&mut m.spindle, s, s.label()).changed();
        }
    });
    egui::Grid::new("m_spindle").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ch |= num(ui, "Obroty min.", &mut m.rpm_min, 0.0..=100000.0, 100.0, " obr/min");
        ch |= num(ui, "Obroty maks.", &mut m.rpm_max, 0.0..=100000.0, 100.0, " obr/min");
        ch |= num(ui, "Czas rozpędzania", &mut m.spindle_delay, 0.0..=60.0, 0.1, " s");
    });
    ui.add_space(6.0);
    ui.strong("Zmiana narzędzia");
    ui.horizontal(|ui| {
        for t in [ToolChange::ManualPause, ToolChange::Automatic, ToolChange::SeparateFiles] {
            ch |= ui.selectable_value(&mut m.tool_change, t, t.label()).changed();
        }
    });
    let mut use_pos = m.tool_change_pos.is_some();
    if ui.checkbox(&mut use_pos, "Odjazd do pozycji zmiany (G53, współrzędne maszyny)").changed() {
        m.tool_change_pos = if use_pos { Some([0.0, 0.0, -5.0]) } else { None };
        ch = true;
    }
    if let Some(pos) = &mut m.tool_change_pos {
        ui.horizontal(|ui| {
            for (l, v) in ["X", "Y", "Z"].iter().zip(pos.iter_mut()) {
                ui.label(*l);
                ch |= ui.add(egui::DragValue::new(v).speed(1.0).suffix(" mm")).changed();
            }
        });
    }
    ui.add_space(6.0);
    ui.strong("Wyposażenie");
    ch |= ui.checkbox(&mut m.coolant_mist, "Chłodzenie mgłą (M7)").changed();
    ch |= ui.checkbox(&mut m.coolant_flood, "Chłodzenie / odciąg (M8)").changed();
    ui.horizontal(|ui| {
        ch |= ui.checkbox(&mut m.probe, "Sonda Z, grubość płytki").changed();
        ch |= ui.add_enabled(m.probe, egui::DragValue::new(&mut m.probe_plate).range(0.0..=100.0).speed(0.1).suffix(" mm")).changed();
    });
    ch
}

fn gcode(ui: &mut egui::Ui, m: &mut Machine) -> bool {
    let mut ch = false;
    egui::Grid::new("m_gcode").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Rozszerzenie pliku");
        ch |= ui.add(egui::TextEdit::singleline(&mut m.extension).desired_width(80.0)).changed();
        ui.end_row();
        ui.label("Miejsca po przecinku");
        let mut d = m.decimals as f64;
        if ui.add(egui::DragValue::new(&mut d).range(1.0..=6.0).speed(0.05)).changed() {
            m.decimals = d as usize;
            ch = true;
        }
        ui.end_row();
        if m.controller == Controller::LinuxCnc {
            ch |= num(ui, "Tolerancja G64", &mut m.linuxcnc_path_tolerance, 0.0..=1.0, 0.001, " mm");
        }
    });
    ui.horizontal(|ui| {
        ch |= ui.checkbox(&mut m.arcs, "Łuki G2/G3").changed();
        ch |= ui.checkbox(&mut m.line_numbers, "Numery linii N").changed();
        ch |= ui.checkbox(&mut m.comments, "Komentarze").changed();
    });
    ui.add_space(6.0);
    ui.label("Początek programu (puste = domyślny)");
    ch |= ui
        .add(egui::TextEdit::multiline(&mut m.header).code_editor().desired_rows(3).desired_width(f32::INFINITY).hint_text("G17 G21 G90 G94\nG54"))
        .changed();
    ui.label("Koniec programu (puste = domyślny)");
    ch |= ui
        .add(egui::TextEdit::multiline(&mut m.footer).code_editor().desired_rows(3).desired_width(f32::INFINITY).hint_text("M5\nM30"))
        .changed();
    ch
}
