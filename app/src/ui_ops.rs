//! Panel operacji: frez dla nowych operacji, lista operacji, edytor parametrów, eksport G-code.

use crate::doc::Document;
use crate::icons;
use crate::view2d::op_color;
use cam_ops::{Direction, EngraveSide, Entry, OpKind, PocketStrategy, ProfileSide, Tabs, ToolKind, ToolLibrary};
use eframe::egui::{self, Color32, RichText};

#[derive(Default, PartialEq)]
pub enum OpsAction {
    #[default]
    None,
    PreviewGcode,
    ExportGcode,
    /// Włącz dodawanie/usuwanie krzywych do operacji `i`.
    EditCurves(usize),
    /// Operacja usunięta / zmieniona kolejność — zakończ tryb dodawania.
    StopPicking,
}

/// Stan panelu operacji.
#[derive(Default)]
pub struct OpsUi {
    /// Frez wybrany dla nowych operacji.
    pub palette_tool: Option<u32>,
}

pub fn fmt_time(minutes: f64) -> String {
    let s = (minutes * 60.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// `picking` — indeks operacji w trybie dodawania krzywych.
pub fn ops_panel(
    ui: &mut egui::Ui,
    doc: &mut Document,
    lib: &ToolLibrary,
    state: &mut OpsUi,
    picking: Option<usize>,
    machine_label: &str,
) -> OpsAction {
    let mut action = OpsAction::None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Operacje");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !doc.ops.is_empty() {
                ui.label(RichText::new(format!("czas ≈ {}", fmt_time(doc.total_minutes()))).weak());
            }
        });
    });

    // --- frez dla nowych operacji ---
    if state.palette_tool.is_none_or(|id| lib.get(id).is_none()) {
        state.palette_tool = lib.tools.iter().find(|t| t.kind == ToolKind::FlatEnd).or(lib.tools.first()).map(|t| t.id);
    }
    egui::Frame::new()
        .fill(Color32::from_rgb(39, 42, 48))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(52, 57, 64)))
        .corner_radius(8.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(28.0, 68.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 4.0, Color32::from_rgb(29, 32, 36));
                if let Some(t) = state.palette_tool.and_then(|id| lib.get(id)) {
                    icons::draw_tool(ui.painter(), r.shrink(3.0), &t.kind);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("Frez dla nowych operacji").small().weak());
                    let current = state
                        .palette_tool
                        .and_then(|id| lib.get(id))
                        .map(|t| t.display_name())
                        .unwrap_or_else(|| "— brak —".into());
                    egui::ComboBox::from_id_salt("palette_tool")
                        .width(ui.available_width() - 4.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for t in &lib.tools {
                                ui.selectable_value(&mut state.palette_tool, Some(t.id), t.display_name());
                            }
                        });
                    if let Some(t) = state.palette_tool.and_then(|id| lib.get(id)) {
                        let c = t.cutting_for(&doc.stock.material);
                        ui.label(
                            RichText::new(format!(
                                "{} · {} ostrza · {} obr/min · {} mm/min",
                                t.kind.label(),
                                t.flutes,
                                c.rpm,
                                c.feed
                            ))
                            .small()
                            .weak(),
                        );
                    }
                });
            });
        });
    ui.label(RichText::new("Wybierz operację na pasku po lewej i klikaj krzywe w widoku 2D.").small().weak());

    ui.separator();
    if doc.ops.is_empty() {
        ui.label(RichText::new("Brak operacji.").weak());
    }

    // --- lista ---
    let mut select: Option<usize> = None;
    let mut changed = false;
    egui::ScrollArea::vertical().id_salt("ops_list").max_height(200.0).auto_shrink([false, true]).show(ui, |ui| {
        for i in 0..doc.ops.len() {
            let sel = doc.selected_op == Some(i);
            ui.horizontal(|ui| {
                changed |= ui.checkbox(&mut doc.ops[i].enabled, "").changed();
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, op_color(i));
                let op = &doc.ops[i];
                let tool = lib.get(op.tool_id).map(|t| format!("T{}", t.number)).unwrap_or_else(|| "T?".into());
                let cache = doc.op_cache.get(i).and_then(|c| c.as_ref());
                let warn = cache.is_some_and(|c| !c.result.warnings.is_empty()) || cache.is_none();
                let text = format!("{}  ·  {}  ·  {} krz.", op.name, tool, op.contours.len());
                let mut rt = RichText::new(text);
                if !op.enabled {
                    rt = rt.weak();
                }
                if ui.selectable_label(sel, rt).clicked() {
                    select = Some(i);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(c) = cache {
                        ui.label(RichText::new(fmt_time(c.minutes)).small().weak());
                    }
                    if warn {
                        ui.label(RichText::new("!").strong().color(Color32::from_rgb(230, 180, 80)))
                            .on_hover_text("Operacja ma ostrzeżenia");
                    }
                });
            });
        }
    });
    if let Some(i) = select {
        doc.selected_op = if doc.selected_op == Some(i) { None } else { Some(i) };
        if picking.is_some() {
            action = OpsAction::StopPicking;
        }
        doc.revision += 1;
    }
    if changed {
        doc.revision += 1;
    }

    if let Some(i) = doc.selected_op {
        ui.horizontal(|ui| {
            if ui.small_button("W górę").clicked() {
                doc.move_op(i, true);
                action = OpsAction::StopPicking;
            }
            if ui.small_button("W dół").clicked() {
                doc.move_op(i, false);
                action = OpsAction::StopPicking;
            }
            if ui.small_button("Duplikuj").clicked() {
                let mut op = doc.ops[i].clone();
                op.name.push_str(" (kopia)");
                doc.add_op(op);
            }
            if ui.small_button("Usuń").clicked() {
                doc.remove_op(i);
                action = OpsAction::StopPicking;
            }
        });
    }

    // --- edytor ---
    if let Some(i) = doc.selected_op.filter(|&i| i < doc.ops.len()) {
        ui.separator();
        let active = picking == Some(i);
        let (text, fill) = if active {
            ("Dodawanie krzywych: WŁ.  —  Enter kończy", op_color(i).gamma_multiply(0.35))
        } else {
            ("Dodaj / usuń krzywe kliknięciem", ui.visuals().widgets.inactive.weak_bg_fill)
        };
        if ui.add(egui::Button::new(text).fill(fill).min_size(egui::vec2(ui.available_width(), 26.0))).clicked() {
            action = if active { OpsAction::StopPicking } else { OpsAction::EditCurves(i) };
        }
        egui::ScrollArea::vertical().id_salt("op_editor").max_height(ui.available_height() - 90.0).show(ui, |ui| {
            if op_editor(ui, doc, i, lib) {
                doc.ops_dirty = true;
            }
        });
    }

    // --- G-code ---
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Postprocesor z maszyny: {machine_label}")).small().weak());
        ui.horizontal(|ui| {
            if ui.button("Podgląd G-code").clicked() {
                action = OpsAction::PreviewGcode;
            }
            if ui.button(RichText::new("Eksportuj G-code…").strong()).clicked() {
                action = OpsAction::ExportGcode;
            }
        });
        ui.separator();
    });
    action
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> bool {
    ui.label(label);
    let r = ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(3)).changed();
    ui.end_row();
    r
}

fn entry_ui(ui: &mut egui::Ui, entry: &mut Entry) -> bool {
    let mut changed = false;
    ui.label("Wejście");
    ui.horizontal(|ui| {
        let is_ramp = matches!(entry, Entry::Ramp { .. });
        if ui.selectable_label(!is_ramp, "Pionowe").clicked() && is_ramp {
            *entry = Entry::Plunge;
            changed = true;
        }
        if ui.selectable_label(is_ramp, "Rampa").clicked() && !is_ramp {
            *entry = Entry::Ramp { angle: 3.0 };
            changed = true;
        }
        if let Entry::Ramp { angle } = entry {
            changed |= ui.add(egui::DragValue::new(angle).range(0.5..=45.0).speed(0.1).suffix("°")).changed();
        }
    });
    ui.end_row();
    changed
}

fn direction_ui(ui: &mut egui::Ui, d: &mut Direction) -> bool {
    let mut changed = false;
    ui.label("Kierunek");
    ui.horizontal(|ui| {
        changed |= ui.radio_value(d, Direction::Climb, "Współbieżny").changed();
        changed |= ui.radio_value(d, Direction::Conventional, "Przeciwbieżny").changed();
    });
    ui.end_row();
    changed
}

/// Zwraca true, jeśli zmieniono parametry wpływające na ścieżkę.
fn op_editor(ui: &mut egui::Ui, doc: &mut Document, i: usize, lib: &ToolLibrary) -> bool {
    let mut changed = false;
    let n_contours = doc.ops[i].contours.len();
    let sel = doc.selected_indices();
    let material = doc.stock.material.clone();

    ui.horizontal(|ui| {
        ui.label(RichText::new(doc.ops[i].kind.label()).strong().color(op_color(i)));
        ui.add(egui::TextEdit::singleline(&mut doc.ops[i].name).desired_width(f32::INFINITY));
    });

    ui.horizontal(|ui| {
        ui.label(format!("Krzywe: {n_contours}"));
        if ui.small_button("Pokaż").on_hover_text("Zaznacz krzywe tej operacji").clicked() {
            doc.clear_selection();
            for k in doc.ops[i].contours.clone() {
                if k < doc.contours.len() {
                    doc.set_selected(k, true);
                }
            }
        }
        if ui
            .add_enabled(!sel.is_empty(), egui::Button::new("Ustaw z zaznaczenia").small())
            .on_hover_text("Zastąp krzywe operacji aktualnie zaznaczonymi")
            .clicked()
        {
            let kind = doc.ops[i].kind.clone();
            doc.ops[i].contours = sel.iter().copied().filter(|&k| doc.contour_fits(&kind, k)).collect();
            changed = true;
        }
    });

    let op = &mut doc.ops[i];

    ui.horizontal(|ui| {
        ui.label("Narzędzie");
        let current = lib.get(op.tool_id).map(|t| t.display_name()).unwrap_or_else(|| "— brak —".into());
        egui::ComboBox::from_id_salt("op_tool").width(220.0).selected_text(current).show_ui(ui, |ui| {
            for t in &lib.tools {
                if ui.selectable_label(t.id == op.tool_id, t.display_name()).clicked() && t.id != op.tool_id {
                    op.tool_id = t.id;
                    op.cut.apply_tool(t, &material);
                    changed = true;
                }
            }
        });
    });
    if let Some(t) = lib.get(op.tool_id) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} · Ø{} · ostrza {} mm · korekcja r = {} mm",
                    t.kind.label(),
                    t.diameter,
                    t.flute_length,
                    t.radius()
                ))
                .small()
                .weak(),
            );
            if ui
                .small_button(format!("Parametry: {material}"))
                .on_hover_text("Wczytaj obroty i posuwy z narzędzia dla materiału półfabrykatu")
                .clicked()
            {
                op.cut.apply_tool(t, &material);
                changed = true;
            }
        });
    }

    egui::Grid::new("op_params").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        let is_drill = matches!(op.kind, OpKind::Drill(ref d) if !d.helix);
        let c = &mut op.cut;
        changed |= num(ui, "Głębokość", &mut c.depth, 0.01..=500.0, 0.05, " mm");
        if !is_drill {
            changed |= num(ui, "Krok w dół", &mut c.step_down, 0.01..=100.0, 0.05, " mm");
            changed |= num(ui, "Posuw", &mut c.feed, 1.0..=50000.0, 10.0, " mm/min");
        }
        let plunge_label = if is_drill { "Posuw wiercenia" } else { "Posuw wgłębny" };
        changed |= num(ui, plunge_label, &mut c.plunge_feed, 1.0..=50000.0, 5.0, " mm/min");
        changed |= num(ui, "Obroty", &mut c.rpm, 0.0..=100000.0, 100.0, " obr/min");
        changed |= num(ui, "Wys. bezpieczna", &mut c.safe_z, 0.5..=200.0, 0.1, " mm");
        changed |= num(ui, "Góra (Z)", &mut c.top_z, -500.0..=500.0, 0.05, " mm");

        match &mut op.kind {
            OpKind::Profile(p) => {
                ui.label("Strona");
                ui.horizontal(|ui| {
                    changed |= ui.radio_value(&mut p.side, ProfileSide::Outside, "Zewn.").changed();
                    changed |= ui.radio_value(&mut p.side, ProfileSide::Inside, "Wewn.").changed();
                    changed |= ui.radio_value(&mut p.side, ProfileSide::On, "Po linii").changed();
                });
                ui.end_row();
                changed |= direction_ui(ui, &mut p.direction);
                changed |= entry_ui(ui, &mut p.entry);
                changed |= num(ui, "Naddatek", &mut p.stock_to_leave, -10.0..=50.0, 0.01, " mm");
                ui.label("Mostki");
                let mut on = p.tabs.is_some();
                if ui.checkbox(&mut on, "").changed() {
                    p.tabs = if on { Some(Tabs { count: 4, width: 6.0, height: 2.0 }) } else { None };
                    changed = true;
                }
                ui.end_row();
                if let Some(t) = &mut p.tabs {
                    ui.label("   liczba");
                    let mut cnt = t.count as f64;
                    if ui.add(egui::DragValue::new(&mut cnt).range(1.0..=50.0).speed(0.1)).changed() {
                        t.count = cnt as u32;
                        changed = true;
                    }
                    ui.end_row();
                    changed |= num(ui, "   szerokość", &mut t.width, 0.5..=100.0, 0.1, " mm");
                    changed |= num(ui, "   wysokość", &mut t.height, 0.1..=100.0, 0.05, " mm");
                }
            }
            OpKind::Pocket(p) => {
                ui.label("Strategia");
                ui.horizontal(|ui| {
                    changed |= ui.radio_value(&mut p.strategy, PocketStrategy::Offset, "Offsetowa").changed();
                    changed |= ui.radio_value(&mut p.strategy, PocketStrategy::Zigzag, "Zygzak").changed();
                });
                ui.end_row();
                if p.strategy == PocketStrategy::Zigzag {
                    changed |= num(ui, "Kąt przejść", &mut p.zigzag_angle, -180.0..=180.0, 1.0, "°");
                }
                let mut pct = p.stepover * 100.0;
                ui.label("Stepover");
                if ui.add(egui::DragValue::new(&mut pct).range(5.0..=100.0).speed(0.5).suffix(" % Ø")).changed() {
                    p.stepover = pct / 100.0;
                    changed = true;
                }
                ui.end_row();
                changed |= direction_ui(ui, &mut p.direction);
                changed |= entry_ui(ui, &mut p.entry);
                changed |= num(ui, "Naddatek ścian", &mut p.stock_to_leave, 0.0..=50.0, 0.01, " mm");
            }
            OpKind::Drill(p) => {
                ui.label("Frezowanie po helisie");
                changed |= ui.checkbox(&mut p.helix, "").on_hover_text("Dla otworów większych niż narzędzie").changed();
                ui.end_row();
                if !p.helix {
                    ui.label("Wiercenie z wycofaniem");
                    let mut on = p.peck.is_some();
                    if ui.checkbox(&mut on, "").changed() {
                        p.peck = if on { Some(3.0) } else { None };
                        changed = true;
                    }
                    ui.end_row();
                    if let Some(q) = &mut p.peck {
                        changed |= num(ui, "   głębokość kroku", q, 0.1..=100.0, 0.05, " mm");
                    }
                }
            }
            OpKind::Engrave(p) => {
                ui.label("Strona");
                ui.horizontal(|ui| {
                    changed |= ui.radio_value(&mut p.side, EngraveSide::Center, "Po ścieżce").changed();
                    changed |= ui.radio_value(&mut p.side, EngraveSide::Left, "Z lewej").changed();
                    changed |= ui.radio_value(&mut p.side, EngraveSide::Right, "Z prawej").changed();
                });
                ui.end_row();
                changed |= entry_ui(ui, &mut p.entry);
            }
        }
    });

    if let Some(Some(cache)) = doc.op_cache.get(i) {
        for w in &cache.result.warnings {
            ui.label(RichText::new(format!("Uwaga: {w}")).color(Color32::from_rgb(230, 180, 80)));
        }
        if cache.result.toolpath.moves.is_empty() {
            ui.label(RichText::new("Brak ścieżki — dodaj krzywe").color(Color32::LIGHT_RED));
        } else {
            ui.label(
                RichText::new(format!(
                    "Ruchów: {} · czas ≈ {}",
                    cache.result.toolpath.moves.len(),
                    fmt_time(cache.minutes)
                ))
                .small()
                .weak(),
            );
        }
    } else if lib.get(doc.ops[i].tool_id).is_none() {
        ui.label(RichText::new("Uwaga: narzędzie nie istnieje w bibliotece").color(Color32::LIGHT_RED));
    }
    changed
}
