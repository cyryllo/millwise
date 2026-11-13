//! Panel operacji: frez dla nowych operacji, lista operacji, edytor parametrów, eksport G-code.

use crate::doc::Document;
use crate::icons;
use crate::toolbar::Sub;
use crate::ui_kit::{self, big_num, num};
use crate::view2d::op_color;
use cam_ops::{Direction, EngraveSide, Entry, OpKind, PocketStrategy, ProfileSide, Tabs, ToolKind, ToolLibrary};
use eframe::egui::{self, Color32, RichText};

#[derive(Default, PartialEq)]
pub enum OpsAction {
    #[default]
    None,
    PreviewGcode,
    ExportGcode,
    /// Przejście do symulacji obróbki.
    Simulate,
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

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui_kit::title(ui, "Kolejność obróbki");
        if doc.ops.len() > 1 {
            ui.label(RichText::new("przeciągnij, aby zmienić").small().color(ui_kit::LABEL));
        }
    });
    if doc.ops.is_empty() {
        ui.label(RichText::new("Brak operacji.").weak());
    }

    // --- lista: nazwa operacji na górze, krótko narzędzie i głębokość pod spodem ---
    let mut select: Option<usize> = None;
    let mut changed = false;
    let mut reorder: Option<(usize, usize)> = None;
    egui::ScrollArea::vertical().id_salt("ops_list").max_height(190.0).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for i in 0..doc.ops.len() {
            ui.horizontal(|ui| {
                changed |= ui.checkbox(&mut doc.ops[i].enabled, "").on_hover_text("Włącz / wyłącz operację").changed();
                if let Some(r) = op_row(ui, doc, lib, i) {
                    if r.clicked() {
                        select = Some(i);
                    }
                    // przeciąganie: zmiana kolejności obróbki
                    if r.drag_started() {
                        r.dnd_set_drag_payload(i);
                    }
                    if r.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    let above = ui.ctx().pointer_interact_pos().is_some_and(|p| p.y < r.rect.center().y);
                    if let Some(src) = r.dnd_hover_payload::<usize>() {
                        if *src != i {
                            let y = if above { r.rect.top() - 2.0 } else { r.rect.bottom() + 2.0 };
                            ui.painter().hline(r.rect.x_range(), y, egui::Stroke::new(3.0, Color32::from_rgb(61, 127, 224)));
                        }
                    }
                    if let Some(src) = r.dnd_release_payload::<usize>() {
                        let src = *src;
                        let mut to = if above { i } else { i + 1 };
                        if src < to {
                            to -= 1;
                        }
                        reorder = Some((src, to));
                    }
                }
            });
        }
    });
    if let Some((from, to)) = reorder.filter(|(a, b)| a != b) {
        doc.move_op_to(from, to);
        action = OpsAction::StopPicking;
        select = None;
    }
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

    // --- ustawienia zaznaczonej operacji: osobna ramka ---
    if let Some(i) = doc.selected_op.filter(|&i| i < doc.ops.len()) {
        ui.add_space(6.0);
        let active = picking == Some(i);
        let bottom_reserve = 118.0;
        egui::Frame::new()
            .fill(ui_kit::CARD_FILL)
            .stroke(egui::Stroke::new(1.5, op_color(i).gamma_multiply(0.8)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt("op_editor")
                    .max_height((ui.available_height() - bottom_reserve).max(120.0))
                    .show(ui, |ui| {
                        if op_editor(ui, doc, i, lib, active, &mut action) {
                            doc.ops_dirty = true;
                        }
                    });
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
        // kolejny krok po operacjach: symulacja, potem eksport
        ui.add_space(2.0);
        if icons::wide_view_button(ui, icons::Icon::Preview3D, "Symulacja obróbki  (Tab)", Color32::from_rgb(70, 190, 110))
            .on_hover_text("Sprawdź ścieżki na symulacji 3D przed eksportem G-code")
            .clicked()
        {
            action = OpsAction::Simulate;
        }
        ui.separator();
    });
    action
}

/// Przejście wykańczające: włącznik i naddatek zostawiany przez obróbkę zgrubną.
fn finish_ui(ui: &mut egui::Ui, finish: &mut Option<f64>) -> bool {
    let mut changed = false;
    ui_kit::label(ui, "Przejście wykańczające");
    let mut on = finish.is_some();
    if ui
        .checkbox(&mut on, "")
        .on_hover_text("Zgrubnie zostaje mały naddatek, a na końcu jedno przejście na pełnej głębokości zbiera go do wymiaru — gładsza ścianka")
        .changed()
    {
        *finish = on.then_some(0.3);
        changed = true;
    }
    ui.end_row();
    if let Some(a) = finish {
        changed |= num(ui, "   zostaw do wykończenia", a, 0.05..=5.0, 0.01, " mm");
    }
    changed
}

fn entry_ui(ui: &mut egui::Ui, entry: &mut Entry) -> bool {
    let mut changed = false;
    ui_kit::label(ui, "Wejście");
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
    ui_kit::label(ui, "Kierunek");
    ui.horizontal(|ui| {
        changed |= ui.radio_value(d, Direction::Climb, "Współbieżny").changed();
        changed |= ui.radio_value(d, Direction::Conventional, "Przeciwbieżny").changed();
    });
    ui.end_row();
    changed
}

/// Wiersz listy operacji: pasek w kolorze operacji, ikona, nazwa, a pod nią narzędzie i głębokość.
fn op_row(ui: &mut egui::Ui, doc: &Document, lib: &ToolLibrary, i: usize) -> Option<egui::Response> {
    let op = doc.ops.get(i)?;
    let sel = doc.selected_op == Some(i);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let color = op_color(i);
    let (bg, stroke) = if sel {
        (Color32::from_rgb(31, 51, 80), egui::Stroke::new(1.5, color))
    } else if resp.hovered() {
        (Color32::from_rgb(45, 49, 56), egui::Stroke::new(1.0, Color32::from_gray(110)))
    } else {
        (ui_kit::CARD_FILL, egui::Stroke::new(1.0, ui_kit::CARD_STROKE))
    };
    painter.rect(rect, 6.0, bg, stroke, egui::StrokeKind::Inside);
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min + egui::vec2(3.0, 5.0), egui::vec2(4.0, rect.height() - 10.0)),
        2.0,
        color,
    );
    let mut ip = painter.clone();
    if !op.enabled {
        ip.set_opacity(0.4);
    }
    icons::draw(&ip, egui::Rect::from_min_size(rect.min + egui::vec2(11.0, 7.0), egui::vec2(26.0, 26.0)), Sub::for_op(&op.kind).icon());

    let cache = doc.op_cache.get(i).and_then(|c| c.as_ref());
    let warnings: Vec<String> = cache.map(|c| c.result.warnings.clone()).unwrap_or_default();
    let x = rect.left() + 44.0;
    let name_color = if op.enabled { Color32::WHITE } else { Color32::from_gray(120) };
    painter.text(
        egui::pos2(x, rect.top() + 12.0),
        egui::Align2::LEFT_CENTER,
        format!("{}. {}", i + 1, op.name),
        egui::FontId::proportional(14.0),
        name_color,
    );
    let tool = lib.get(op.tool_id).map(|t| format!("T{} Ø{}", t.number, t.diameter)).unwrap_or_else(|| "T?".into());
    let detail = format!("{tool}  ·  gł. {} mm  ·  {} krz.", fmt_mm(op.cut.depth), op.contours.len());
    painter.text(egui::pos2(x, rect.top() + 29.0), egui::Align2::LEFT_CENTER, detail, egui::FontId::proportional(11.0), Color32::from_gray(150));
    if let Some(c) = cache {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.top() + 12.0),
            egui::Align2::RIGHT_CENTER,
            fmt_time(c.minutes),
            egui::FontId::proportional(11.0),
            Color32::from_gray(150),
        );
    }
    let resp = if !warnings.is_empty() || cache.is_none() {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.top() + 29.0),
            egui::Align2::RIGHT_CENTER,
            "uwaga",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(230, 180, 80),
        );
        let text = if warnings.is_empty() { "Brak ścieżki".to_string() } else { warnings.join("\n") };
        resp.on_hover_text(text)
    } else {
        resp
    };
    Some(resp)
}

/// Liczba bez zbędnych zer po przecinku.
fn fmt_mm(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Ustawienia operacji `i`. Zwraca true, jeśli zmieniono parametry wpływające na ścieżkę.
fn op_editor(ui: &mut egui::Ui, doc: &mut Document, i: usize, lib: &ToolLibrary, picking: bool, action: &mut OpsAction) -> bool {
    let mut changed = false;
    let n_contours = doc.ops[i].contours.len();
    let sel = doc.selected_indices();
    let material = doc.stock.material.clone();
    let color = op_color(i);

    // --- nagłówek: rodzaj i nazwa ---
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
        icons::draw(ui.painter(), r, Sub::for_op(&doc.ops[i].kind).icon());
        ui.vertical(|ui| {
            ui.label(RichText::new(doc.ops[i].kind.label().to_uppercase()).small().strong().color(color));
            ui.add(
                egui::TextEdit::singleline(&mut doc.ops[i].name)
                    .font(egui::FontId::proportional(16.0))
                    .desired_width(f32::INFINITY),
            );
        });
    });

    // --- krzywe ---
    ui.add_space(4.0);
    let (text, fill) = if picking {
        ("Dodawanie krzywych: WŁ.  —  Enter kończy", color.gamma_multiply(0.35))
    } else {
        ("Dodaj / usuń krzywe kliknięciem", ui.visuals().widgets.inactive.weak_bg_fill)
    };
    if ui.add(egui::Button::new(text).fill(fill).min_size(egui::vec2(ui.available_width(), 26.0))).clicked() {
        *action = if picking { OpsAction::StopPicking } else { OpsAction::EditCurves(i) };
    }
    ui.horizontal(|ui| {
        ui_kit::label(ui, &format!("Krzywe: {n_contours}"));
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

    // --- najważniejsze: głębokość, krok w dół / wycofanie ---
    ui.add_space(6.0);
    ui_kit::accent_card(ui, color, |ui| {
        egui::Grid::new("op_key").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            let c = &mut op.cut;
            changed |= big_num(ui, "Głębokość", &mut c.depth, 0.01..=500.0, 0.05, " mm");
            match &mut op.kind {
                OpKind::Drill(d) if !d.helix => {
                    ui.label(RichText::new("Wycofanie").size(15.0).strong().color(Color32::WHITE));
                    ui.horizontal(|ui| {
                        let mut on = d.peck.is_some();
                        if ui.checkbox(&mut on, "").on_hover_text("Wiercenie z wycofaniem co zadaną głębokość").changed() {
                            d.peck = if on { Some(3.0) } else { None };
                            changed = true;
                        }
                        if let Some(q) = &mut d.peck {
                            ui.label(RichText::new("co").color(ui_kit::LABEL));
                            ui.scope(|ui| {
                                ui.style_mut().override_font_id = Some(egui::FontId::proportional(16.0));
                                ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
                                changed |= ui.add(egui::DragValue::new(q).range(0.1..=100.0).speed(0.05).suffix(" mm")).changed();
                            });
                        }
                    });
                    ui.end_row();
                }
                _ => {
                    changed |= big_num(ui, "Krok w dół", &mut c.step_down, 0.01..=100.0, 0.05, " mm");
                }
            }
        });
        let passes = match &op.kind {
            OpKind::Drill(d) if !d.helix => d.peck.map(|q| (op.cut.depth / q.max(0.01)).ceil() as u32),
            _ => Some((op.cut.depth / op.cut.step_down.max(0.01)).ceil() as u32),
        };
        if let Some(n) = passes.filter(|&n| n > 0) {
            let word = match n {
                1 => "przejście",
                n if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) => "przejścia",
                _ => "przejść",
            };
            ui.label(RichText::new(format!("{n} {word} w Z")).small().color(ui_kit::LABEL));
        }
    });

    // --- ustawienia rodzaju operacji ---
    let tool_r = lib.get(op.tool_id).map_or(3.0, |t| t.radius());
    ui_kit::subtitle(ui, op.kind.label());
    egui::Grid::new("op_kind").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| match &mut op.kind {
        OpKind::Profile(p) => {
            ui_kit::label(ui, "Strona");
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut p.side, ProfileSide::Outside, "Zewn.").changed();
                changed |= ui.radio_value(&mut p.side, ProfileSide::Inside, "Wewn.").changed();
                changed |= ui.radio_value(&mut p.side, ProfileSide::On, "Po linii").changed();
            });
            ui.end_row();
            changed |= direction_ui(ui, &mut p.direction);
            changed |= entry_ui(ui, &mut p.entry);
            changed |= num(ui, "Naddatek", &mut p.stock_to_leave, -10.0..=50.0, 0.01, " mm");
            changed |= finish_ui(ui, &mut p.finish);
            ui_kit::label(ui, "Wejście po łuku");
            let mut on = p.lead.is_some();
            if ui
                .checkbox(&mut on, "")
                .on_hover_text("Zejście w odpadzie, obok ścianki; narzędzie dochodzi i odchodzi łukiem stycznym — bez śladu w miejscu wejścia")
                .changed()
            {
                p.lead = on.then_some(tool_r);
                changed = true;
            }
            ui.end_row();
            if let Some(l) = &mut p.lead {
                changed |= num(ui, "   promień łuku", l, 0.2..=100.0, 0.05, " mm");
                if matches!(p.entry, Entry::Ramp { .. }) {
                    ui.label("");
                    ui.label(RichText::new("z łukiem zejście jest pionowe, w odpadzie").small().color(ui_kit::LABEL));
                    ui.end_row();
                }
            }
            ui_kit::label(ui, "Mostki");
            let mut on = p.tabs.is_some();
            if ui.checkbox(&mut on, "").changed() {
                p.tabs = if on { Some(Tabs { count: 4, width: 6.0, height: 2.0 }) } else { None };
                changed = true;
            }
            ui.end_row();
            if let Some(t) = &mut p.tabs {
                ui_kit::label(ui, "   liczba");
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
            ui_kit::label(ui, "Strategia");
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut p.strategy, PocketStrategy::Offset, "Offsetowa").changed();
                changed |= ui.radio_value(&mut p.strategy, PocketStrategy::Zigzag, "Zygzak").changed();
            });
            ui.end_row();
            if p.strategy == PocketStrategy::Zigzag {
                changed |= num(ui, "Kąt przejść", &mut p.zigzag_angle, -180.0..=180.0, 1.0, "°");
            }
            let mut pct = p.stepover * 100.0;
            ui_kit::label(ui, "Stepover");
            if ui.add(egui::DragValue::new(&mut pct).range(5.0..=100.0).speed(0.5).suffix(" % Ø")).changed() {
                p.stepover = pct / 100.0;
                changed = true;
            }
            ui.end_row();
            changed |= direction_ui(ui, &mut p.direction);
            changed |= entry_ui(ui, &mut p.entry);
            changed |= num(ui, "Naddatek ścian", &mut p.stock_to_leave, 0.0..=50.0, 0.01, " mm");
            changed |= finish_ui(ui, &mut p.finish);
        }
        OpKind::Drill(p) => {
            ui_kit::label(ui, "Frezowanie po helisie");
            changed |= ui.checkbox(&mut p.helix, "").on_hover_text("Dla otworów większych niż narzędzie").changed();
            ui.end_row();
        }
        OpKind::Engrave(p) => {
            ui_kit::label(ui, "Strona");
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut p.side, EngraveSide::Center, "Po ścieżce").changed();
                changed |= ui.radio_value(&mut p.side, EngraveSide::Left, "Z lewej").changed();
                changed |= ui.radio_value(&mut p.side, EngraveSide::Right, "Z prawej").changed();
            });
            ui.end_row();
            changed |= entry_ui(ui, &mut p.entry);
        }
    });

    // --- narzędzie i skrawanie ---
    ui_kit::subtitle(ui, "Narzędzie i skrawanie");
    let current = lib.get(op.tool_id).map(|t| t.display_name()).unwrap_or_else(|| "— brak —".into());
    egui::ComboBox::from_id_salt("op_tool").width(ui.available_width() - 8.0).selected_text(current).show_ui(ui, |ui| {
        for t in &lib.tools {
            if ui.selectable_label(t.id == op.tool_id, t.display_name()).clicked() && t.id != op.tool_id {
                op.tool_id = t.id;
                op.cut.apply_tool(t, &material);
                changed = true;
            }
        }
    });
    if let Some(t) = lib.get(op.tool_id) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("{} · Ø{} · ostrza {} mm · korekcja r = {} mm", t.kind.label(), t.diameter, t.flute_length, t.radius()))
                    .small()
                    .color(ui_kit::LABEL),
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
    let is_drill = matches!(op.kind, OpKind::Drill(ref d) if !d.helix);
    egui::Grid::new("op_cut").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        let c = &mut op.cut;
        if !is_drill {
            changed |= num(ui, "Posuw", &mut c.feed, 1.0..=50000.0, 10.0, " mm/min");
        }
        let plunge_label = if is_drill { "Posuw wiercenia" } else { "Posuw wgłębny" };
        changed |= num(ui, plunge_label, &mut c.plunge_feed, 1.0..=50000.0, 5.0, " mm/min");
        changed |= num(ui, "Obroty", &mut c.rpm, 0.0..=100000.0, 100.0, " obr/min");
    });

    // --- wysokości: rzadko zmieniane, zwinięte ---
    ui.add_space(4.0);
    let c = &mut op.cut;
    let header = format!("Wysokości  (bezpieczna {} mm, góra {} mm)", fmt_mm(c.safe_z), fmt_mm(c.top_z));
    egui::CollapsingHeader::new(RichText::new(header).color(ui_kit::LABEL)).id_salt("op_heights").show(ui, |ui| {
        egui::Grid::new("op_heights_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            changed |= num(ui, "Wys. bezpieczna", &mut c.safe_z, 0.5..=200.0, 0.1, " mm");
            changed |= num(ui, "Góra (Z)", &mut c.top_z, -500.0..=500.0, 0.05, " mm");
        });
    });

    if let Some(Some(cache)) = doc.op_cache.get(i) {
        for w in &cache.result.warnings {
            ui.label(RichText::new(format!("Uwaga: {w}")).color(Color32::from_rgb(230, 180, 80)));
        }
        if cache.result.toolpath.moves.is_empty() {
            ui.label(RichText::new("Brak ścieżki — dodaj krzywe").color(Color32::LIGHT_RED));
        } else {
            ui.label(
                RichText::new(format!("Ruchów: {} · czas ≈ {}", cache.result.toolpath.moves.len(), fmt_time(cache.minutes)))
                    .small()
                    .color(ui_kit::LABEL),
            );
        }
    } else if lib.get(doc.ops[i].tool_id).is_none() {
        ui.label(RichText::new("Uwaga: narzędzie nie istnieje w bibliotece").color(Color32::LIGHT_RED));
    }
    changed
}
