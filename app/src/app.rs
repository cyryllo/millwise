use crate::doc::{AlignMode, Document};
use crate::history::History;
use crate::sim_panel::SimState;
use crate::storage;
use crate::toolbar::{Sub, Toolbar, ToolbarAction};
use crate::ui_kit;
use crate::ui_machines::MachinesWindow;
use crate::ui_ops::{self, OpsAction, OpsUi};
use crate::ui_tools::ToolsWindow;
use crate::view2d::{colors, op_color, Interaction, View2D, ViewEvent};
use crate::view3d::View3D;
use cam_geom::Vec2;
use cam_ops::{CutParams, Operation, ToolLibrary};
use cam_post::{Machine, MachineLibrary};
use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};

/// Właściciel praw autorskich wyświetlany w „O programie”.
const COPYRIGHT_HOLDER: &str = "Cyryl Sochacki";

/// Projekt 2D — praca na krzywych i operacjach; Wizualizacja 3D — tylko podgląd.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Project,
    Visual,
}

/// Parametry narzędzi edycji (panel nad rysunkiem).
struct EditParams {
    dx: f64,
    dy: f64,
    angle: f64,
    scale_pct: f64,
    cols: usize,
    rows: usize,
    gap_x: f64,
    gap_y: f64,
}

impl Default for EditParams {
    fn default() -> Self {
        Self { dx: 10.0, dy: 0.0, angle: 90.0, scale_pct: 100.0, cols: 2, rows: 1, gap_x: 5.0, gap_y: 5.0 }
    }
}

pub struct CamApp {
    doc: Document,
    tools: ToolLibrary,
    tools_window: ToolsWindow,
    machines: MachineLibrary,
    machines_window: MachinesWindow,
    toolbar: Toolbar,
    /// Operacja, do której kliknięcia dodają krzywe (None = zostanie utworzona przy pierwszym kliknięciu).
    pick_target: Option<usize>,
    edit: EditParams,
    view2d: View2D,
    view3d: View3D,
    sim: SimState,
    mode: ViewMode,
    ops_ui: OpsUi,
    error: Option<String>,
    info: Option<String>,
    gcode_preview: Option<Vec<String>>,
    about_open: bool,
    shortcuts_open: bool,
    logo: Option<egui::TextureHandle>,
    history: History,
    show_history: bool,
    /// Element projektu rozwinięty na liście po prawej.
    open_source: Option<usize>,
    /// Krok historii, do którego panel ostatnio przewinął listę.
    history_seen: u64,
    /// Schowek krzywych (Ctrl+C / Ctrl+X → Ctrl+V): kontur i nazwa warstwy.
    clipboard: Vec<crate::doc::ClipItem>,
}

impl CamApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let (tools, tools_warning) = storage::load_tools();
        let (machines, machines_warning) = storage::load_machines();
        let doc = Document::default();
        let history = History::new(&doc, "Nowy projekt");
        let mut app = Self {
            doc,
            tools,
            tools_window: ToolsWindow::default(),
            machines,
            machines_window: MachinesWindow::default(),
            toolbar: Toolbar::default(),
            pick_target: None,
            edit: EditParams::default(),
            view2d: View2D::default(),
            view3d: View3D::default(),
            sim: SimState::default(),
            mode: ViewMode::Project,
            ops_ui: OpsUi::default(),
            error: tools_warning.or(machines_warning),
            info: None,
            gcode_preview: None,
            about_open: false,
            shortcuts_open: false,
            logo: None,
            history,
            show_history: true,
            open_source: None,
            history_seen: u64::MAX,
            clipboard: Vec::new(),
        };
        let mut files = files.into_iter();
        if let Some(p) = files.next() {
            app.open_any(p);
        }
        let more: Vec<PathBuf> = files.collect();
        if !more.is_empty() {
            app.add_files(more);
            app.info = None;
        }
        // dla programisty / zrzutów ekranu: MILLWISE_OPEN=3d,tools,machines
        if let Ok(v) = std::env::var("MILLWISE_OPEN") {
            for w in v.split(',') {
                match w.trim() {
                    "3d" => app.mode = ViewMode::Visual,
                    "bottom" => app.view3d.bottom_view = true,
                    w if w.starts_with("simat=") => {
                        app.mode = ViewMode::Visual;
                        app.sim.jump_to_fraction = w[6..].parse().ok();
                    }
                    "simend" => {
                        app.mode = ViewMode::Visual;
                        app.sim.jump_to_end = true;
                    }
                    "tools" => app.tools_window.open = true,
                    "machines" => app.machines_window.open = true,
                    "about" => app.about_open = true,
                    "keys" => app.shortcuts_open = true,
                    w if w.starts_with("op=") => {
                        app.doc.selected_op = w[3..].parse().ok().filter(|&i: &usize| i < app.doc.ops.len());
                    }
                    _ => {}
                }
            }
        }
        app
    }

    fn machine(&self) -> Machine {
        self.machines.active().cloned().unwrap_or_default()
    }

    fn open_any(&mut self, path: PathBuf) {
        // .camsoft — projekty z wcześniejszej wersji (przed zmianą nazwy)
        let is_project = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("millwise") || e.eq_ignore_ascii_case("camsoft"));
        if is_project {
            self.open_project(&path);
        } else {
            self.open_dxf(path);
        }
    }

    fn open_dxf(&mut self, path: PathBuf) {
        match Document::load_dxf(&path) {
            Ok(mut doc) => {
                doc.stock = std::mem::take(&mut self.doc.stock);
                self.set_doc(doc);
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn open_project(&mut self, path: &Path) {
        match storage::load_project(path, &mut self.tools) {
            Ok((doc, lib_changed)) => {
                if lib_changed {
                    self.save_tools();
                    self.info = Some("Do biblioteki dodano narzędzia z projektu.".into());
                }
                self.set_doc(doc);
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn set_doc(&mut self, doc: Document) {
        self.doc = doc;
        self.open_source = None;
        self.history.reset(&self.doc, format!("Otwarto {}", self.doc.file_name()));
        self.stop_mode();
        self.gcode_preview = None;
        self.view2d.fit_requested = true;
        self.view3d.fit_requested = true;
    }

    fn save_tools(&mut self) {
        if let Err(e) = storage::save_tools(&self.tools) {
            self.error = Some(format!("Nie można zapisać biblioteki narzędzi: {e:#}"));
        }
    }

    fn save_machines(&mut self) {
        if let Err(e) = storage::save_machines(&self.machines) {
            self.error = Some(format!("Nie można zapisać maszyn: {e:#}"));
        }
    }

    fn open_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new()
            .add_filter("DXF i projekty", &["dxf", "DXF", "millwise", "camsoft"])
            .add_filter("DXF", &["dxf", "DXF"])
            .add_filter("Projekt Millwise", &["millwise", "camsoft"]);
        if let Some(dir) = self.doc.project_path.as_ref().or(self.doc.path.as_ref()).and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(p) = dlg.pick_file() {
            self.open_any(p);
        }
    }

    /// Plik → Dodaj: dokłada wybrane pliki DXF i projekty do bieżącego projektu.
    fn add_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new()
            .set_title("Dodaj do projektu")
            .add_filter("DXF i projekty", &["dxf", "DXF", "millwise", "camsoft"])
            .add_filter("DXF", &["dxf", "DXF"])
            .add_filter("Projekt Millwise", &["millwise", "camsoft"]);
        if let Some(dir) = self.doc.project_path.as_ref().or(self.doc.path.as_ref()).and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(paths) = dlg.pick_files() {
            self.add_files(paths);
        }
    }

    /// Dodaje pliki jako kolejne elementy; do pustego projektu pierwszy plik jest po prostu otwierany.
    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let mut added = Vec::new();
        for path in paths {
            if self.doc.contours.is_empty() && self.doc.ops.is_empty() {
                self.open_any(path);
                continue;
            }
            let is_project =
                path.extension().is_some_and(|e| e.eq_ignore_ascii_case("millwise") || e.eq_ignore_ascii_case("camsoft"));
            let loaded = if is_project {
                storage::load_project(&path, &mut self.tools).map(|(d, lib_changed)| {
                    if lib_changed {
                        self.save_tools();
                    }
                    d
                })
            } else {
                Document::load_dxf(&path)
            };
            match loaded {
                Ok(other) => {
                    let name = other.file_name();
                    let (n, ops) = self.doc.merge(other);
                    added.push(if ops > 0 { format!("{name}: {n} krzywych, {ops} operacji") } else { format!("{name}: {n} krzywych") });
                }
                Err(e) => self.error = Some(format!("{}: {e:#}", path.display())),
            }
        }
        if !added.is_empty() {
            self.open_source = self.doc.sources.len().checked_sub(1);
            self.stop_mode();
            self.view2d.fit_requested = true;
            self.info = Some(format!(
                "Dodano do projektu:\n{}\n\nNowe elementy stoją obok dotychczasowych i są zaznaczone — można je od razu przesunąć.",
                added.join("\n")
            ));
        }
    }

    fn file_stem(&self) -> String {
        self.doc
            .project_path
            .as_ref()
            .or(self.doc.path.as_ref())
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "program".into())
    }

    fn save_project(&mut self, ask: bool) {
        let path = match (&self.doc.project_path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("Projekt Millwise", &["millwise"])
                    .set_file_name(format!("{}.millwise", self.file_stem()));
                if let Some(dir) = self.doc.path.as_ref().and_then(|p| p.parent()) {
                    dlg = dlg.set_directory(dir);
                }
                let Some(mut p) = dlg.save_file() else { return };
                if p.extension().is_none() {
                    p.set_extension("millwise");
                }
                p
            }
        };
        match storage::save_project(&self.doc, &self.tools, &path) {
            Ok(()) => {
                self.doc.project_path = Some(path.clone());
                self.info = Some(format!("Zapisano projekt {}", path.display()));
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn export_gcode(&mut self) {
        self.ensure_generated();
        let machine = self.machine();
        let files = match self.doc.gcode(&self.tools, &machine) {
            Ok(f) => f,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let ext = machine.extension().to_string();
        let mut dlg = rfd::FileDialog::new()
            .add_filter("G-code", &[ext.as_str(), "nc", "ngc", "gcode", "tap"])
            .set_file_name(format!("{}.{ext}", self.file_stem()));
        if let Some(dir) = self.doc.project_path.as_ref().or(self.doc.path.as_ref()).and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        let Some(p) = dlg.save_file() else { return };
        let mut written = Vec::new();
        for (suffix, text) in &files {
            let path = if suffix.is_empty() {
                p.clone()
            } else {
                let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                p.with_file_name(format!("{stem}_{suffix}.{ext}"))
            };
            if let Err(e) = std::fs::write(&path, text) {
                self.error = Some(format!("Nie można zapisać {}: {e}", path.display()));
                return;
            }
            written.push(path.display().to_string());
        }
        self.info = Some(format!("Zapisano G-code:\n{}", written.join("\n")));
    }

    fn ensure_generated(&mut self) {
        if self.doc.ops_dirty {
            let machine = self.machine();
            self.doc.regenerate(&self.tools, Some(&machine));
            if self.gcode_preview.is_some() {
                self.gcode_preview = self.preview_lines().ok();
            }
        }
    }

    fn preview_lines(&self) -> Result<Vec<String>, String> {
        let files = self.doc.gcode(&self.tools, &self.machine())?;
        let mut lines = Vec::new();
        for (suffix, text) in files {
            if !suffix.is_empty() {
                lines.push(format!("(===== plik {suffix} =====)"));
            }
            lines.extend(text.lines().map(String::from));
        }
        Ok(lines)
    }

    // ---------- tryby paska narzędzi ----------

    fn stop_mode(&mut self) {
        self.toolbar.reset();
        self.pick_target = None;
    }

    fn now(ctx: &egui::Context) -> f64 {
        ctx.input(|i| i.time)
    }

    fn handle_toolbar(&mut self, action: ToolbarAction, ctx: &egui::Context) {
        match action {
            ToolbarAction::Changed => self.pick_target = None,
            ToolbarAction::Preview => self.mode = ViewMode::Visual,
            ToolbarAction::Command(sub) => {
                let ok = match sub {
                    Sub::FlipH => self.doc.flip_selected(true),
                    Sub::FlipV => self.doc.flip_selected(false),
                    Sub::ZeroBL | Sub::ZeroCenter | Sub::ZeroTL => {
                        if let Some(sb) = self.doc.stock_box() {
                            let p = match sub {
                                Sub::ZeroBL => sb.min,
                                Sub::ZeroTL => Vec2::new(sb.min.x, sb.max.y),
                                _ => sb.center(),
                            };
                            self.doc.set_zero(p);
                            self.view2d.fit_requested = true;
                            self.view2d.flash(format!("Zero programu: {}", sub.label().to_lowercase()), Self::now(ctx));
                        }
                        true
                    }
                    _ => true,
                };
                if !ok {
                    self.view2d.flash("Najpierw zaznacz krzywe", Self::now(ctx));
                }
            }
        }
    }

    /// Kliknięcie obok (w puste miejsce rysunku): koniec dodawania krzywych i
    /// zamknięcie ustawień operacji. Zmiany są już zapisane w projekcie; operację
    /// otwiera ponownie kliknięcie na liście po lewej albo w historii.
    fn close_op_editor(&mut self, ctx: &egui::Context) {
        let target = self.pick_target.filter(|&t| t < self.doc.ops.len());
        if self.toolbar.current().op_template(1.0).is_some() && target.is_none() {
            return; // wybrany rodzaj operacji, ale operacji jeszcze nie ma — czekamy na krzywą
        }
        let Some(i) = target.or(self.doc.selected_op) else { return };
        let name = self.doc.ops.get(i).map(|o| o.name.clone()).unwrap_or_default();
        if target.is_some() {
            self.stop_mode(); // koniec dodawania krzywych; inne tryby (np. Przesuń) zostają
        }
        self.doc.selected_op = None;
        self.doc.revision += 1;
        self.view2d.flash(format!("Zapisano: {name}"), Self::now(ctx));
    }

    /// Operacja docelowa dla klikania; tworzy nową z szablonu wariantu, jeśli trzeba.
    fn ensure_target(&mut self) -> Option<usize> {
        if let Some(t) = self.pick_target.filter(|&t| t < self.doc.ops.len()) {
            return Some(t);
        }
        let machine = self.machine();
        let (kind, depth, label) = self.toolbar.current().op_template(self.doc.stock.thickness)?;
        let tool = self.ops_ui.palette_tool.and_then(|id| self.tools.get(id)).or(self.tools.tools.first())?;
        let mut cut = CutParams { depth, safe_z: machine.safe_z, ..Default::default() };
        cut.apply_tool(tool, &self.doc.stock.material);
        let n = self.doc.ops.iter().filter(|o| o.name.starts_with(label)).count() + 1;
        let op = Operation { name: format!("{label} {n}"), enabled: true, kind, tool_id: tool.id, cut, contours: vec![] };
        let i = self.doc.add_op(op);
        self.pick_target = Some(i);
        Some(i)
    }

    fn handle_view_events(&mut self, events: Vec<ViewEvent>, ctx: &egui::Context) {
        for ev in events {
            match ev {
                ViewEvent::Contour(i) => {
                    if let Some(t) = self.ensure_target() {
                        if let Err(e) = self.doc.toggle_op_contour(t, i) {
                            self.view2d.flash(e, Self::now(ctx));
                        }
                    } else {
                        self.error = Some("Biblioteka narzędzi jest pusta — dodaj narzędzie.".into());
                    }
                }
                ViewEvent::Box(found) => {
                    if let Some(t) = self.ensure_target() {
                        self.doc.add_op_contours(t, &found);
                    }
                }
                ViewEvent::Point(p) => {
                    if self.toolbar.current() == Sub::ZeroPick {
                        self.doc.set_zero(p);
                        self.view2d.fit_requested = true;
                        self.view2d.flash(format!("Nowe zero w punkcie X {:.3} Y {:.3}", p.x, p.y), Self::now(ctx));
                        self.stop_mode();
                    }
                }
                ViewEvent::Moved(d) => {
                    self.doc.move_selected(d);
                }
                ViewEvent::Empty => self.close_op_editor(ctx),
            }
        }
    }

    /// Panel opcji aktywnego narzędzia nad rysunkiem 2D.
    fn tool_options(&mut self, ctx: &egui::Context, view: egui::Rect) {
        let sub = self.toolbar.current();
        if sub == Sub::Select {
            return;
        }
        let color = if sub.op_template(1.0).is_some() {
            self.pick_target.map_or(sub.group().color(), op_color)
        } else {
            sub.group().color()
        };
        let mut close = false;
        let now = Self::now(ctx);
        let mut nothing_selected = false;
        egui::Area::new(egui::Id::unique("tool_options"))
            .fixed_pos(view.min + egui::vec2(10.0, 10.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(17, 19, 22, 235))
                    .stroke(egui::Stroke::new(1.5, color))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_max_width((view.width() - 40.0).max(200.0));
                        let e = &mut self.edit;
                        let doc = &mut self.doc;
                        let mut apply = |ok: bool| {
                            if !ok {
                                nothing_selected = true;
                            }
                        };
                        match sub {
                            Sub::Move => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Przesuń");
                                    ui.label(RichText::new("przeciągnij zaznaczone myszą lub:").weak());
                                    ui.label("ΔX");
                                    ui.add(egui::DragValue::new(&mut e.dx).speed(0.1).suffix(" mm"));
                                    ui.label("ΔY");
                                    ui.add(egui::DragValue::new(&mut e.dy).speed(0.1).suffix(" mm"));
                                    if ui.button("Przesuń").clicked() {
                                        apply(doc.move_selected(Vec2::new(e.dx, e.dy)));
                                    }
                                });
                            }
                            Sub::Rotate => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Obróć zaznaczone wokół ich środka");
                                    ui.add(egui::DragValue::new(&mut e.angle).speed(0.5).suffix("°"));
                                    if ui.button("Obróć").clicked() {
                                        apply(doc.rotate_selected(e.angle));
                                    }
                                    for a in [-90.0, 90.0, 180.0] {
                                        if ui.button(format!("{a:+.0}°")).clicked() {
                                            apply(doc.rotate_selected(a));
                                        }
                                    }
                                });
                            }
                            Sub::Scale => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Skaluj zaznaczone");
                                    ui.add(egui::DragValue::new(&mut e.scale_pct).range(0.1..=10000.0).speed(0.5).suffix(" %"));
                                    if ui.button("Skaluj").clicked() {
                                        apply(doc.scale_selected(e.scale_pct / 100.0));
                                    }
                                });
                            }
                            Sub::Align => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Wyrównaj zaznaczone");
                                    for (m, l) in [
                                        (AlignMode::Left, "Do lewej"),
                                        (AlignMode::CenterX, "Środek X"),
                                        (AlignMode::Right, "Do prawej"),
                                        (AlignMode::Top, "Do góry"),
                                        (AlignMode::CenterY, "Środek Y"),
                                        (AlignMode::Bottom, "Do dołu"),
                                    ] {
                                        if ui.button(l).clicked() {
                                            apply(doc.align_selected(m));
                                        }
                                    }
                                });
                            }
                            Sub::Array => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Szyk");
                                    ui.label("kolumny");
                                    ui.add(egui::DragValue::new(&mut e.cols).range(1..=100));
                                    ui.label("wiersze");
                                    ui.add(egui::DragValue::new(&mut e.rows).range(1..=100));
                                    ui.label("odstęp X");
                                    ui.add(egui::DragValue::new(&mut e.gap_x).speed(0.1).suffix(" mm"));
                                    ui.label("Y");
                                    ui.add(egui::DragValue::new(&mut e.gap_y).speed(0.1).suffix(" mm"));
                                    if ui.button("Utwórz szyk").clicked() {
                                        match doc.selection_bbox() {
                                            Some(bb) => {
                                                let s = bb.size();
                                                doc.array_selected(e.cols, e.rows, s.x + e.gap_x, s.y + e.gap_y);
                                            }
                                            None => nothing_selected = true,
                                        }
                                    }
                                });
                                ui.label(RichText::new("Odstęp liczony między krawędziami kopii; kopie trafiają też do operacji oryginału.").small().weak());
                            }
                            Sub::ZeroPick => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong("Zero programu:");
                                    ui.label("kliknij punkt (przyciąga do środków okręgów i wierzchołków)");
                                    if ui.button("Anuluj").clicked() {
                                        close = true;
                                    }
                                });
                            }
                            s if s.op_template(1.0).is_some() => {
                                let target = self.pick_target.filter(|&t| t < doc.ops.len());
                                ui.horizontal_wrapped(|ui| {
                                    let title = match target {
                                        Some(t) => format!("Dodawanie do: {}", doc.ops[t].name),
                                        None => format!("{} › {} — kliknij krzywą, aby utworzyć operację", s.group().label(), s.label()),
                                    };
                                    ui.label(RichText::new(title).strong().color(Color32::WHITE));
                                    ui.label(RichText::new("klik = dodaj/usuń · ramka = wiele").weak());
                                    if ui.button("Zakończ (Enter)").clicked() {
                                        close = true;
                                    }
                                });
                            }
                            _ => {}
                        }
                    });
            });
        if nothing_selected {
            self.view2d.flash("Najpierw zaznacz krzywe", now);
        }
        if close {
            self.stop_mode();
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if pressed(cmd(Key::O)) {
            self.open_dialog();
        }
        if pressed(cmd(Key::I)) {
            self.add_dialog();
        }
        // z Shiftem najpierw: Ctrl+S pasuje też do Ctrl+Shift+S
        if pressed(cmd_shift(Key::S)) {
            self.save_project(true);
        }
        if pressed(cmd(Key::S)) {
            self.save_project(false);
        }
        if pressed(cmd(Key::E)) {
            self.export_gcode();
        }
        // w polu tekstowym skróty edycji należą do pola (cofanie tekstu, kopiowanie)
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let now = Self::now(ctx);
        if pressed(cmd_shift(Key::Z)) || pressed(cmd(Key::Y)) {
            self.redo(now);
        }
        if pressed(cmd(Key::Z)) {
            self.undo(now);
        }
        if self.mode == ViewMode::Project {
            if pressed(cmd(Key::A)) {
                self.select_all();
            }
            if pressed(cmd(Key::C)) {
                self.copy(now);
            }
            if pressed(cmd(Key::X)) {
                self.cut(now);
            }
            if pressed(cmd(Key::V)) {
                self.paste(now);
            }
        }
        let (esc, enter, tab, f, del) = ctx.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Tab),
                i.key_pressed(Key::F),
                i.key_pressed(Key::Delete),
            )
        });
        if self.mode == ViewMode::Visual {
            if esc || tab {
                self.mode = ViewMode::Project;
                self.sim.playing = false;
            }
            if ctx.input(|i| i.key_pressed(Key::Space)) {
                self.sim.playing = !self.sim.playing;
            }
        } else {
            if esc || enter {
                if self.toolbar.current() != Sub::Select || self.pick_target.is_some() {
                    self.stop_mode();
                } else if esc {
                    self.doc.clear_selection();
                }
            }
            if tab {
                self.mode = ViewMode::Visual;
            }
        }
        if f {
            self.view2d.fit_requested = true;
            self.view3d.fit_requested = true;
        }
        if del && self.mode == ViewMode::Project {
            self.delete(now);
        }
    }

    // ---------- edycja: historia i schowek ----------

    fn undo(&mut self, now: f64) {
        match self.history.undo(&mut self.doc) {
            Some(label) => {
                self.stop_mode();
                self.view2d.flash(format!("Cofnięto: {label}"), now);
            }
            None => self.view2d.flash("Nie ma czego cofnąć", now),
        }
    }

    fn redo(&mut self, now: f64) {
        match self.history.redo(&mut self.doc) {
            Some(label) => {
                self.stop_mode();
                self.view2d.flash(format!("Ponowiono: {label}"), now);
            }
            None => self.view2d.flash("Nie ma czego ponowić", now),
        }
    }

    fn select_all(&mut self) {
        for i in 0..self.doc.contours.len() {
            if self.doc.is_visible(i) {
                self.doc.set_selected(i, true);
            }
        }
    }

    fn copy(&mut self, now: f64) -> bool {
        let items = self.doc.copy_selected();
        if items.is_empty() {
            self.view2d.flash("Najpierw zaznacz krzywe", now);
            return false;
        }
        self.view2d.flash(format!("Skopiowano krzywe: {}", items.len()), now);
        self.clipboard = items;
        true
    }

    fn cut(&mut self, now: f64) {
        if self.copy(now) {
            let n = self.doc.delete_selected();
            self.doc.label(crate::history::Mark::new(crate::icons::Icon::Cut), format!("Wycięto krzywe ({n})"));
            self.stop_mode();
            self.view2d.flash(format!("Wycięto krzywe: {n}"), now);
        }
    }

    /// Wkleja schowek: środkiem w miejsce kursora, gdy jest nad rysunkiem, inaczej w to samo miejsce.
    fn paste(&mut self, now: f64) {
        if self.clipboard.is_empty() {
            self.view2d.flash("Schowek jest pusty", now);
            return;
        }
        let d = match (self.view2d.cursor_world, self.clipboard.iter().map(|c| c.contour.bbox()).reduce(|a, b| a.union(&b))) {
            (Some(p), Some(bb)) => p - bb.center(),
            _ => Vec2::new(0.0, 0.0),
        };
        self.stop_mode();
        self.doc.add_contours(&self.clipboard, d);
        self.view2d.flash(format!("Wklejono krzywe: {}", self.clipboard.len()), now);
    }

    /// Delete: zaznaczone krzywe, a gdy żadnej nie ma — zaznaczona operacja.
    fn delete(&mut self, now: f64) {
        if self.doc.selected_count() > 0 {
            let n = self.doc.delete_selected();
            self.stop_mode();
            self.view2d.flash(format!("Usunięto krzywe: {n}"), now);
        } else if let Some(i) = self.doc.selected_op {
            self.doc.remove_op(i);
            self.stop_mode();
        }
    }

    /// Historia zmian: rząd ikon od najstarszej do najnowszej; opis po najechaniu, klik = powrót.
    fn history_panel(&mut self, ui: &mut egui::Ui) {
        let now = Self::now(ui.ctx());
        let serial = self.history.serial;
        let mut jump = None;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(RichText::new("HISTORIA").small().strong().color(Color32::from_gray(150)));
            if ui.add_enabled(self.history.can_undo(), egui::Button::new("Cofnij")).on_hover_text("Ctrl+Z").clicked() {
                self.undo(now);
            }
            if ui.add_enabled(self.history.can_redo(), egui::Button::new("Ponów")).on_hover_text("Ctrl+Y").clicked() {
                self.redo(now);
            }
            ui.separator();
            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .max_height(32.0)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let pos = self.history.pos();
                    for g in self.history.groups() {
                        let current = (g.first..=g.last).contains(&pos);
                        let undone = g.first > pos;
                        let count = g.last - g.first + 1;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
                        let painter = ui.painter();
                        let accent = Color32::from_rgb(61, 127, 224);
                        let (bg, stroke) = if current {
                            (Color32::from_rgb(31, 51, 80), egui::Stroke::new(1.5, accent))
                        } else if resp.hovered() {
                            (Color32::from_rgb(52, 57, 64), egui::Stroke::new(1.0, Color32::from_gray(120)))
                        } else {
                            (Color32::from_rgb(35, 38, 43), egui::Stroke::new(1.0, Color32::from_rgb(52, 57, 64)))
                        };
                        painter.rect(rect, 6.0, bg, stroke, egui::StrokeKind::Inside);
                        if let Some(op) = g.mark.op {
                            let bar = egui::Rect::from_min_max(
                                egui::pos2(rect.left() + 5.0, rect.bottom() - 4.0),
                                egui::pos2(rect.right() - if count > 1 { 15.0 } else { 5.0 }, rect.bottom() - 2.0),
                            );
                            painter.rect_filled(bar, 1.0, op_color(op));
                        }
                        let mut p = painter.clone();
                        if undone {
                            p.set_opacity(0.35);
                        }
                        crate::icons::draw(&p, rect.shrink(6.0), g.mark.icon);
                        draw_badge(&p, rect, g.mark.badge);
                        if count > 1 {
                            let c = egui::pos2(rect.right() - 7.0, rect.bottom() - 7.0);
                            p.circle(c, 6.5, Color32::from_rgb(17, 19, 22), egui::Stroke::new(1.0, Color32::from_gray(120)));
                            p.text(c, egui::Align2::CENTER_CENTER, count.to_string(), egui::FontId::proportional(9.5), Color32::WHITE);
                        }
                        let mut tip = String::new();
                        for (k, label) in g.labels.iter().enumerate() {
                            let n = g.first + k;
                            tip.push_str(&format!("{}. {label}", n + 1));
                            if n > pos {
                                tip.push_str("  (cofnięte)");
                            }
                            tip.push('\n');
                        }
                        let open = g.mark.op.is_some() && g.mark.badge != crate::history::Badge::Remove;
                        tip.push_str(match (pos == g.last, undone || current, open) {
                            (true, _, true) => "(stan obecny) — kliknij, aby edytować operację",
                            (true, _, false) => "(stan obecny)",
                            (false, true, _) => "kliknij, aby przywrócić",
                            (false, false, _) => "kliknij, aby wrócić do tego stanu",
                        });
                        let resp = resp.on_hover_text(tip);
                        if current && self.history_seen != serial {
                            resp.scroll_to_me(Some(egui::Align::Center));
                            self.history_seen = serial;
                        }
                        if resp.clicked() {
                            jump = Some((g.last, g.mark.op.filter(|_| g.mark.badge != crate::history::Badge::Remove)));
                        }
                    }
                });
            });
        });
        // klik: powrót do końca grupy (jeśli trzeba) i otwarcie ustawień jej operacji
        if let Some((step, op)) = jump {
            if step != self.history.pos() && self.history.jump(&mut self.doc, step) {
                self.stop_mode();
            }
            if let Some(op) = op.filter(|&o| o < self.doc.ops.len()) {
                self.stop_mode();
                self.doc.selected_op = Some(op);
                self.doc.revision += 1;
            }
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Plik", |ui| {
                if ui.button("Otwórz DXF / projekt…  (Ctrl+O)").clicked() {
                    self.open_dialog();
                }
                if ui
                    .button("Dodaj DXF / projekt…  (Ctrl+I)")
                    .on_hover_text("Dołóż kolejne pliki do bieżącego projektu (można wybrać kilka naraz)")
                    .clicked()
                {
                    self.add_dialog();
                }
                ui.separator();
                if ui.button("Zapisz projekt  (Ctrl+S)").clicked() {
                    self.save_project(false);
                }
                if ui.button("Zapisz projekt jako…  (Ctrl+Shift+S)").clicked() {
                    self.save_project(true);
                }
                ui.separator();
                if ui.button("Eksportuj G-code…  (Ctrl+E)").clicked() {
                    self.export_gcode();
                }
                ui.separator();
                if ui.button("Zakończ").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edycja", |ui| {
                let now = Self::now(ui.ctx());
                let sel = self.doc.selected_count() > 0;
                let project = self.mode == ViewMode::Project;
                if ui.add_enabled(self.history.can_undo(), egui::Button::new("Cofnij  (Ctrl+Z)")).clicked() {
                    self.undo(now);
                }
                if ui.add_enabled(self.history.can_redo(), egui::Button::new("Ponów  (Ctrl+Y)")).clicked() {
                    self.redo(now);
                }
                ui.separator();
                if ui.add_enabled(project && sel, egui::Button::new("Wytnij  (Ctrl+X)")).clicked() {
                    self.cut(now);
                }
                if ui.add_enabled(project && sel, egui::Button::new("Kopiuj  (Ctrl+C)")).clicked() {
                    self.copy(now);
                }
                if ui.add_enabled(project && !self.clipboard.is_empty(), egui::Button::new("Wklej  (Ctrl+V)")).clicked() {
                    self.paste(now);
                }
                if ui.add_enabled(project && sel, egui::Button::new("Usuń krzywe  (Delete)")).clicked() {
                    self.delete(now);
                }
                ui.separator();
                if ui.add_enabled(project, egui::Button::new("Zaznacz wszystko  (Ctrl+A)")).clicked() {
                    self.select_all();
                }
                if ui.add_enabled(project && sel, egui::Button::new("Odznacz  (Esc)")).clicked() {
                    self.doc.clear_selection();
                }
            });
            ui.menu_button("Widok", |ui| {
                ui.radio_value(&mut self.mode, ViewMode::Project, "Projekt 2D  (Tab)");
                ui.radio_value(&mut self.mode, ViewMode::Visual, "Wizualizacja 3D  (Tab)");
                ui.separator();
                if ui.button("Dopasuj  (F)").clicked() {
                    self.view2d.fit_requested = true;
                    self.view3d.fit_requested = true;
                }
                ui.separator();
                ui.checkbox(&mut self.show_history, "Historia zmian");
            });
            ui.menu_button("Narzędzia", |ui| {
                if ui.button("Biblioteka narzędzi…").clicked() {
                    self.tools_window.open = true;
                }
            });
            ui.menu_button("Maszyny", |ui| {
                if ui.button("Maszyny…").clicked() {
                    self.machines_window.open = true;
                }
                ui.separator();
                let ids: Vec<(u32, String)> = self.machines.machines.iter().map(|m| (m.id, m.name.clone())).collect();
                for (id, name) in ids {
                    if ui.radio(self.machines.active == id, name).clicked() {
                        self.machines.active = id;
                        self.save_machines();
                        self.doc.ops_dirty = true;
                    }
                }
            });
            ui.menu_button("Pomoc", |ui| {
                if ui.button("Skróty klawiszowe").clicked() {
                    self.shortcuts_open = true;
                }
                if ui.button("O programie").clicked() {
                    self.about_open = true;
                }
            });
            ui.separator();
            ui.selectable_value(&mut self.mode, ViewMode::Project, RichText::new("  Projekt 2D  ").strong());
            ui.selectable_value(&mut self.mode, ViewMode::Visual, RichText::new("  Wizualizacja 3D  ").strong());
        });
    }

    /// Aktywna maszyna: wyraźna ramka na górze prawego panelu, z wyborem maszyny.
    fn machine_card(&mut self, ui: &mut egui::Ui) {
        let Some(m) = self.machines.active().cloned() else {
            ui_kit::card(ui, |ui| {
                ui_kit::title(ui, "Maszyna");
                if ui.button("Dodaj maszynę…").clicked() {
                    self.machines_window.open = true;
                }
            });
            return;
        };
        let accent = Color32::from_rgb(61, 127, 224);
        let mut pick = None;
        let mut open = false;
        ui_kit::accent_card(ui, accent, |ui| {
            ui.horizontal(|ui| {
                ui_kit::title(ui, "Maszyna");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    open = ui.small_button("Ustawienia…").on_hover_text("Pole robocze, posuwy, sterownik, zmiana narzędzia").clicked();
                });
            });
            let others: Vec<(u32, String)> = self.machines.machines.iter().map(|m| (m.id, m.name.clone())).collect();
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(60.0, 44.0), egui::Sense::click());
                crate::icons::draw_machine(ui.painter(), r, m.kind);
                if resp.on_hover_text(m.kind.label()).clicked() {
                    open = true;
                }
                ui.vertical(|ui| {
                    if others.len() > 1 {
                        egui::ComboBox::from_id_salt("active_machine")
                            .width(ui.available_width() - 8.0)
                            .selected_text(RichText::new(&m.name).size(16.0).strong().color(Color32::WHITE))
                            .show_ui(ui, |ui| {
                                for (id, name) in &others {
                                    if ui.selectable_label(*id == m.id, name).clicked() {
                                        pick = Some(*id);
                                    }
                                }
                            });
                    } else {
                        ui.label(RichText::new(&m.name).size(16.0).strong().color(Color32::WHITE));
                    }
                    ui.label(RichText::new(format!("{} mm", m.summary())).color(ui_kit::LABEL));
                });
            });
        });
        if open {
            self.machines_window.open = true;
        }
        if let Some(id) = pick.filter(|&id| id != m.id) {
            self.machines.active = id;
            self.save_machines();
            self.doc.ops_dirty = true;
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        self.machine_card(ui);
        ui.add_space(6.0);
        self.elements_card(ui);
        ui.add_space(6.0);
        self.stock_card(ui);
        ui.add_space(6.0);
        self.layers_and_curves(ui);
    }

    /// Elementy projektu (wczytane i dodane pliki): zwarta lista; klik rozwija krótką informację.
    fn elements_card(&mut self, ui: &mut egui::Ui) {
        let doc = &mut self.doc;
        let counts = doc.source_counts();
        let shown: Vec<usize> = (0..doc.sources.len()).filter(|&i| counts[i] > 0).collect();
        let mut add = false;
        let mut toggle = None;
        let mut select_src = None;
        let mut delete_src = None;
        ui_kit::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui_kit::title(ui, "Elementy");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    add = ui
                        .small_button("+ Dodaj…")
                        .on_hover_text("Dodaj DXF lub projekt (Ctrl+I); pliki można też upuścić na okno")
                        .clicked();
                });
            });
            if shown.is_empty() {
                ui.label(RichText::new("Otwórz albo dodaj plik DXF lub projekt.").small().color(ui_kit::LABEL));
            }
            for &i in &shown {
                let open = self.open_source == Some(i);
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let bg = if open {
                    Color32::from_rgb(31, 51, 80)
                } else if resp.hovered() {
                    Color32::from_rgb(45, 49, 56)
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(rect, 4.0, bg);
                // trójkąt: rozwinięty / zwinięty
                let c = egui::pos2(rect.left() + 9.0, rect.center().y);
                let tri = if open {
                    vec![c + egui::vec2(-4.0, -2.5), c + egui::vec2(4.0, -2.5), c + egui::vec2(0.0, 3.0)]
                } else {
                    vec![c + egui::vec2(-2.5, -4.0), c + egui::vec2(3.0, 0.0), c + egui::vec2(-2.5, 4.0)]
                };
                painter.add(egui::Shape::convex_polygon(tri, Color32::from_gray(170), egui::Stroke::NONE));
                painter.text(
                    egui::pos2(rect.right() - 6.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format!("{} krz.", counts[i]),
                    egui::FontId::proportional(11.0),
                    ui_kit::LABEL,
                );
                painter.with_clip_rect(rect.shrink2(egui::vec2(0.0, 0.0)).with_max_x(rect.right() - 50.0)).text(
                    egui::pos2(rect.left() + 20.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &doc.sources[i],
                    egui::FontId::proportional(13.5),
                    Color32::WHITE,
                );
                if resp.on_hover_text(&doc.sources[i]).clicked() {
                    toggle = Some(i);
                }
                if open {
                    // krótka informacja o elemencie
                    let mine: Vec<usize> = (0..doc.contours.len()).filter(|&k| doc.contours[k].source == i).collect();
                    let bb = mine.iter().map(|&k| doc.contours[k].bbox).reduce(|a, b| a.union(&b));
                    let mut layers: Vec<&str> = Vec::new();
                    for &k in &mine {
                        if let Some(l) = doc.layers.get(doc.contours[k].layer) {
                            if !layers.contains(&l.name.as_str()) {
                                layers.push(&l.name);
                            }
                        }
                    }
                    let ops: Vec<&str> =
                        doc.ops.iter().filter(|o| o.contours.iter().any(|k| mine.contains(k))).map(|o| o.name.as_str()).collect();
                    egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 4, top: 2, bottom: 4 }).show(ui, |ui| {
                        let line = |ui: &mut egui::Ui, label: &str, text: String| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(RichText::new(label).small().color(ui_kit::LABEL));
                                ui.label(RichText::new(text).small());
                            });
                        };
                        if let Some(b) = bb {
                            let sz = b.size();
                            line(ui, "Wymiary", format!("{:.1} × {:.1} mm", sz.x, sz.y));
                        }
                        if !layers.is_empty() {
                            line(ui, "Warstwy", layers.join(", "));
                        }
                        line(ui, "Operacje", if ops.is_empty() { "brak".into() } else { ops.join(", ") });
                        ui.horizontal(|ui| {
                            if ui.small_button("Zaznacz").on_hover_text("Zaznacz krzywe elementu, np. żeby go przesunąć").clicked() {
                                select_src = Some(i);
                            }
                            if shown.len() > 1 && ui.small_button("Usuń").on_hover_text("Usuń krzywe tego elementu z projektu").clicked() {
                                delete_src = Some(i);
                            }
                        });
                    });
                }
            }
            if doc.path.is_some() || !doc.warnings.is_empty() {
                ui.add_space(2.0);
                if doc.path.is_some() {
                    ui.label(RichText::new(format!("Jednostki DXF: {}", doc.unit_name)).small().color(ui_kit::LABEL));
                }
                for w in &doc.warnings {
                    ui.label(RichText::new(format!("Uwaga: {w}")).small().color(Color32::from_rgb(230, 180, 80)));
                }
            }
        });
        if let Some(i) = toggle {
            self.open_source = if self.open_source == Some(i) { None } else { Some(i) };
        }
        if let Some(i) = select_src {
            self.doc.select_source(i);
        }
        if let Some(i) = delete_src {
            self.doc.select_source(i);
            self.doc.delete_selected();
            let name = self.doc.sources[i].clone();
            self.doc.label(crate::history::Mark::new(crate::icons::Icon::Delete), format!("Usunięto element: {name}"));
            self.open_source = None;
            self.stop_mode();
        }
        if add {
            self.add_dialog();
        }
    }

    fn stock_card(&mut self, ui: &mut egui::Ui) {
        let doc = &mut self.doc;
        let mut materials: Vec<String> = vec!["Drewno".into()];
        for t in &self.tools.tools {
            for p in &t.presets {
                if !materials.contains(&p.material) {
                    materials.push(p.material.clone());
                }
            }
        }
        if !materials.contains(&doc.stock.material) {
            materials.push(doc.stock.material.clone());
        }
        ui_kit::card(ui, |ui| {
            ui_kit::title(ui, "Półfabrykat");
            egui::Grid::new("stock").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                let a = ui_kit::big_num(ui, "Grubość", &mut doc.stock.thickness, 0.1..=500.0, 0.1, " mm");
                let b = ui_kit::num(ui, "Naddatek XY", &mut doc.stock.margin, 0.0..=500.0, 0.1, " mm");
                ui_kit::label(ui, "Materiał");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("material").selected_text(doc.stock.material.clone()).show_ui(ui, |ui| {
                        for m in &materials {
                            ui.selectable_value(&mut doc.stock.material, m.clone(), m);
                        }
                    });
                    if !doc.ops.is_empty()
                        && ui
                            .small_button("Do operacji")
                            .on_hover_text("Zastosuj obroty, posuwy i krok w dół z biblioteki narzędzi dla tego materiału we wszystkich operacjach")
                            .clicked()
                    {
                        let m = doc.stock.material.clone();
                        for op in &mut doc.ops {
                            if let Some(t) = self.tools.get(op.tool_id) {
                                op.cut.apply_tool(t, &m);
                            }
                        }
                        doc.ops_dirty = true;
                    }
                });
                ui.end_row();
                if a || b {
                    doc.revision += 1;
                }
            });
            if let Some(sb) = doc.stock_box() {
                let s = sb.size();
                let m = self.machines.active();
                let fits = m.is_none_or(|m| s.x <= m.travel_x && s.y <= m.travel_y);
                let text = format!(
                    "{:.1} × {:.1} × {:.1} mm, Z0 = góra{}",
                    s.x,
                    s.y,
                    doc.stock.thickness,
                    if fits { "" } else { " — WIĘKSZY NIŻ POLE ROBOCZE" }
                );
                ui.label(RichText::new(text).small().color(if fits { ui_kit::LABEL } else { Color32::LIGHT_RED }));
            }
        });
    }

    /// Warstwy i lista krzywych — zwijane, żeby nie zajmowały miejsca.
    fn layers_and_curves(&mut self, ui: &mut egui::Ui) {
        let doc = &mut self.doc;
        let hovered = self.view2d.hovered;
        ui_kit::card(ui, |ui| {
            if !doc.layers.is_empty() {
                let visible = doc.layers.iter().filter(|l| l.visible).count();
                let head = RichText::new(format!("WARSTWY  {visible} / {}", doc.layers.len())).small().strong().color(ui_kit::LABEL);
                egui::CollapsingHeader::new(head).id_salt("layers").default_open(true).show(ui, |ui| {
                    let mut changed = false;
                    ui.horizontal_wrapped(|ui| {
                        for l in &mut doc.layers {
                            changed |= ui.checkbox(&mut l.visible, &l.name).changed();
                        }
                    });
                    if changed {
                        doc.revision += 1;
                    }
                });
            }
            let head = RichText::new(format!("KRZYWE  {}  ·  zaznaczone {}", doc.contours.len(), doc.selected_count()))
                .small()
                .strong()
                .color(ui_kit::LABEL);
            egui::CollapsingHeader::new(head).id_salt("curves").default_open(false).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Zamknięte").on_hover_text("Zaznacz wszystkie zamknięte krzywe").clicked() {
                        for i in 0..doc.contours.len() {
                            let v = doc.is_visible(i) && doc.contours[i].contour.closed;
                            doc.set_selected(i, v);
                        }
                    }
                    if ui.small_button("Okręgi").on_hover_text("Zaznacz wszystkie okręgi (do wiercenia)").clicked() {
                        for i in 0..doc.contours.len() {
                            let v = doc.is_visible(i) && doc.contours[i].contour.as_circle().is_some();
                            doc.set_selected(i, v);
                        }
                    }
                    if ui.small_button("Wszystkie").clicked() {
                        for i in 0..doc.contours.len() {
                            let v = doc.is_visible(i);
                            doc.set_selected(i, v);
                        }
                    }
                    if ui.small_button("Odznacz").clicked() {
                        doc.clear_selection();
                    }
                });
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                egui::ScrollArea::vertical().auto_shrink([false, true]).show_rows(
                    ui,
                    ui.text_style_height(&egui::TextStyle::Body) + 4.0,
                    doc.contours.len(),
                    |ui, range| {
                        for i in range {
                            let c = &doc.contours[i];
                            let kind_color = if c.contour.closed { colors::CLOSED } else { colors::OPEN };
                            let shape = match c.contour.as_circle() {
                                Some((_, r)) => format!("Ø{:.2}", r * 2.0),
                                None if c.contour.closed => "●".into(),
                                None => "○".into(),
                            };
                            let layer = doc.layers.get(c.layer).map_or("", |l| l.name.as_str());
                            let text = RichText::new(format!("#{:<3}{:<7}{:>7.1}  {}", i + 1, shape, c.length, layer))
                                .monospace()
                                .color(if Some(i) == hovered { Color32::WHITE } else { kind_color });
                            let visible = doc.is_visible(i);
                            let r = ui.add_enabled(visible, egui::Button::selectable(doc.selected[i], text));
                            if r.clicked() {
                                let additive = ui.input(|inp| inp.modifiers.command || inp.modifiers.shift);
                                if additive {
                                    let v = !doc.selected[i];
                                    doc.set_selected(i, v);
                                } else {
                                    doc.clear_selection();
                                    doc.set_selected(i, true);
                                }
                            }
                        }
                    },
                );
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.view2d.cursor_world {
                Some(p) => ui.monospace(format!("X {:>9.3}  Y {:>9.3}", p.x, p.y)),
                None => ui.monospace("X        —  Y        —"),
            };
            ui.separator();
            let sub = self.toolbar.current();
            ui.label(format!("Aktywne: {} › {}", sub.group().label(), sub.label()));
            ui.separator();
            ui.label(format!("Zaznaczone: {}", self.doc.selected_count()));
            ui.separator();
            ui.label(
                RichText::new(
                    "Tab = Projekt 2D / Wizualizacja 3D · Ctrl+Z / Ctrl+Y = cofnij / ponów · Esc/Enter = koniec trybu · PPM = przesuń widok · kółko = zoom · F = dopasuj",
                )
                .weak()
                .small(),
            );
        });
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        const KEYS: &[(&str, &str)] = &[
            ("Ctrl+O", "otwórz DXF lub projekt"),
            ("Ctrl+I", "dodaj DXF lub projekt do bieżącego"),
            ("Ctrl+S", "zapisz projekt"),
            ("Ctrl+Shift+S", "zapisz projekt jako…"),
            ("Ctrl+E", "eksportuj G-code"),
            ("", ""),
            ("Ctrl+Z", "cofnij"),
            ("Ctrl+Y, Ctrl+Shift+Z", "ponów"),
            ("Ctrl+X", "wytnij zaznaczone krzywe"),
            ("Ctrl+C", "kopiuj zaznaczone krzywe"),
            ("Ctrl+V", "wklej (w miejsce kursora)"),
            ("Delete", "usuń zaznaczone krzywe, a bez nich — zaznaczoną operację"),
            ("Ctrl+A", "zaznacz wszystkie krzywe"),
            ("Esc", "zakończ tryb / odznacz"),
            ("Enter", "zakończ tryb"),
            ("", ""),
            ("Tab", "przełącz Projekt 2D / Wizualizacja 3D"),
            ("F", "dopasuj widok"),
            ("Spacja", "odtwarzanie symulacji (w wizualizacji)"),
            ("kółko myszy", "powiększenie"),
            ("prawy / środkowy przycisk", "przesuwanie widoku"),
            ("Shift / Ctrl + klik", "dodaj do zaznaczenia"),
        ];
        egui::Window::new("Skróty klawiszowe")
            .open(&mut self.shortcuts_open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("keys").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
                    for (k, what) in KEYS {
                        if k.is_empty() {
                            ui.end_row();
                            continue;
                        }
                        ui.label(RichText::new(*k).monospace().strong());
                        ui.label(*what);
                        ui.end_row();
                    }
                });
            });
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let logo = self
            .logo
            .get_or_insert_with(|| {
                let img = egui::ColorImage::from_rgba_unmultiplied(
                    [700, 200],
                    include_bytes!("../../assets/logo/millwise-logo-dark-700x200.rgba"),
                );
                ctx.load_texture("millwise-logo", img, egui::TextureOptions::LINEAR)
            })
            .clone();
        let mut open = true;
        egui::Window::new("O programie").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.image((logo.id(), egui::vec2(490.0, 140.0)));
            ui.add_space(6.0);
            ui.label(format!("Wersja {}", env!("CARGO_PKG_VERSION")));
            ui.label("Ścieżki narzędzia i G-code dla frezarek CNC 3-osiowych (GRBL, FluidNC, LinuxCNC).");
            ui.add_space(6.0);
            ui.label(format!("Copyright © 2026 {}", COPYRIGHT_HOLDER));
            ui.label(
                RichText::new(
                    "Ten program jest wolnym oprogramowaniem: możesz go rozpowszechniać i/lub modyfikować \
                     na warunkach Powszechnej Licencji Publicznej GNU (GPL) w wersji 3 lub (według \
                     Twojego wyboru) dowolnej późniejszej.\n\
                     Program jest rozpowszechniany BEZ JAKIEJKOLWIEK GWARANCJI. Sprawdzaj G-code \
                     przed uruchomieniem na maszynie.",
                )
                .small(),
            );
            ui.horizontal(|ui| {
                ui.hyperlink_to("Tekst licencji GPL-3.0", "https://www.gnu.org/licenses/gpl-3.0.html");
                ui.label(RichText::new("· biblioteki zewnętrzne: THIRD-PARTY.md").small().weak());
            });
        });
        if !open {
            self.about_open = false;
        }
    }

    fn gcode_window(&mut self, ctx: &egui::Context) {
        let Some(lines) = &self.gcode_preview else { return };
        let mut open = true;
        let mut export = false;
        egui::Window::new(format!("G-code · {}", self.machine().name))
            .open(&mut open)
            .default_size([520.0, 600.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{} linii", lines.len()));
                    if ui.button("Kopiuj").clicked() {
                        ui.ctx().copy_text(lines.join("\n"));
                    }
                    if ui.button("Eksportuj…").clicked() {
                        export = true;
                    }
                });
                ui.separator();
                let h = ui.text_style_height(&egui::TextStyle::Monospace);
                egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, h, lines.len(), |ui, range| {
                    for i in range {
                        let l = &lines[i];
                        let color = if l.starts_with('(') {
                            Color32::from_rgb(120, 160, 120)
                        } else if l.starts_with("G0") {
                            Color32::from_rgb(220, 120, 120)
                        } else if l.starts_with('M') || l.starts_with('T') {
                            Color32::from_rgb(220, 190, 100)
                        } else {
                            Color32::from_gray(210)
                        };
                        ui.label(RichText::new(format!("{:>6}  {l}", i + 1)).monospace().color(color));
                    }
                });
            });
        if !open {
            self.gcode_preview = None;
        }
        if export {
            self.export_gcode();
        }
    }
}

/// Dla programisty: `MILLWISE_SCREENSHOT=plik.ppm` zapisuje zrzut własnego okna
/// (po kilkudziesięciu klatkach) i zamyka program.
fn dev_screenshot(ctx: &egui::Context) {
    let Ok(path) = std::env::var("MILLWISE_SCREENSHOT") else { return };
    let n = ctx.cumulative_frame_nr();
    if n < 300 {
        ctx.request_repaint();
    }
    if n == 40 {
        let c = ctx.clone();
        let cb = egui::ScreenshotCallback::new(move |img| {
            let [w, h] = img.size;
            let mut data = format!("P6\n{w} {h}\n255\n").into_bytes();
            for p in &img.pixels {
                data.extend_from_slice(&[p.r(), p.g(), p.b()]);
            }
            if let Err(e) = std::fs::write(&path, data) {
                eprintln!("zrzut ekranu: {e}");
            }
            c.send_viewport_cmd(egui::ViewportCommand::Close);
            c.request_repaint();
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(cb));
    }
}

impl eframe::App for CamApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        dev_screenshot(&ctx);
        // upuszczone pliki: dochodzą do projektu jako kolejne elementy
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.add_files(dropped);
        }
        self.shortcuts(&ctx);
        self.ensure_generated();

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));

        if self.mode == ViewMode::Visual {
            // wizualizacja: tylko podgląd + symulacja
            let machine = self.machine();
            self.sim.sync(&ctx, &self.doc, &self.tools, &machine);
            self.sim.tick(&ctx);
            egui::Panel::left("sim").resizable(false).exact_size(300.0).show(ui, |ui| self.sim.panel(ui));
            let mut close = false;
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                let display = self.sim.display(&self.doc, &self.tools);
                close = self.view3d.ui(ui, rect, frame, &self.doc, display.as_ref());
            });
            if close {
                self.mode = ViewMode::Project;
                self.sim.playing = false;
            }
        } else {
            let mut tb_action = None;
            egui::Panel::left("tb_groups").resizable(false).exact_size(88.0).show(ui, |ui| {
                tb_action = self.toolbar.groups_ui(ui);
            });
            egui::Panel::left("tb_subs").resizable(false).exact_size(96.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(a) = self.toolbar.subs_ui(ui) {
                        tb_action = Some(a);
                    }
                });
            });
            if let Some(a) = tb_action {
                self.handle_toolbar(a, &ctx);
            }

            let machine_label = self.machine().controller.label().to_string();
            let picking = self.pick_target.filter(|_| self.toolbar.current().op_template(1.0).is_some());
            let mut action = OpsAction::None;
            egui::Panel::left("ops").resizable(true).default_size(310.0).show(ui, |ui| {
                action = ui_ops::ops_panel(ui, &mut self.doc, &self.tools, &mut self.ops_ui, picking, &machine_label);
            });
            egui::Panel::right("geometry").resizable(true).default_size(260.0).show(ui, |ui| self.side_panel(ui));
            if self.show_history {
                egui::Panel::bottom("history")
                    .resizable(false)
                    .exact_size(48.0)
                    .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(8, 6)))
                    .show(ui, |ui| self.history_panel(ui));
            }
            match action {
                OpsAction::None => {}
                OpsAction::ExportGcode => self.export_gcode(),
                OpsAction::PreviewGcode => {
                    self.ensure_generated();
                    match self.preview_lines() {
                        Ok(l) => self.gcode_preview = Some(l),
                        Err(e) => self.error = Some(e),
                    }
                }
                OpsAction::EditCurves(i) => {
                    if let Some(op) = self.doc.ops.get(i) {
                        self.toolbar.set(Sub::for_op(&op.kind));
                        self.pick_target = Some(i);
                        self.doc.selected_op = Some(i);
                    }
                }
                OpsAction::StopPicking => self.stop_mode(),
            }
            self.ensure_generated();

            let tool_d: Vec<f64> =
                self.doc.ops.iter().map(|op| self.tools.get(op.tool_id).map_or(0.0, |t| t.diameter)).collect();
            let machine = self.machine();
            let sub = self.toolbar.current();
            let template = sub.op_template(self.doc.stock.thickness);
            let target = self.pick_target.filter(|&t| t < self.doc.ops.len());
            let target_kind = target.map(|t| self.doc.ops[t].kind.clone());
            let members: Vec<usize> = target.map(|t| self.doc.ops[t].contours.clone()).unwrap_or_default();
            let mut view_rect = egui::Rect::NOTHING;
            let mut events = Vec::new();
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                view_rect = rect;
                let inter = match (&template, sub) {
                    (Some((kind, _, _)), _) => Interaction::Op {
                        kind: target_kind.as_ref().unwrap_or(kind),
                        members: &members,
                        color: target.map_or(sub.group().color(), op_color),
                    },
                    (None, Sub::Move) => Interaction::Move,
                    (None, Sub::ZeroPick) => Interaction::PickPoint,
                    _ => Interaction::Select,
                };
                events = self.view2d.ui(ui, rect, &mut self.doc, &tool_d, inter, Some((machine.travel_x, machine.travel_y)));
            });
            self.handle_view_events(events, &ctx);
            self.tool_options(&ctx, view_rect);
            // kliknięcia w widoku 2D mogły zmienić krzywe operacji
            self.ensure_generated();
        }

        let used: Vec<u32> = self.doc.ops.iter().map(|o| o.tool_id).collect();
        if self.tools_window.show(&ctx, &mut self.tools, &used) {
            self.save_tools();
            self.doc.ops_dirty = true;
        }
        let part = self.doc.stock_box().map(|b| (b.size().x, b.size().y));
        if self.machines_window.show(&ctx, &mut self.machines, part) {
            self.save_machines();
            self.doc.ops_dirty = true;
        }
        self.gcode_window(&ctx);
        self.about_window(&ctx);
        self.shortcuts_window(&ctx);

        // zakończona edycja (mysz puszczona, żadne pole nie jest edytowane) = krok historii
        let settled = !ctx.input(|i| i.pointer.any_down()) && !ctx.egui_wants_keyboard_input();
        self.history.track(&mut self.doc, settled);

        for (title, msg, color) in [("Błąd", &mut self.error, Color32::LIGHT_RED), ("Informacja", &mut self.info, Color32::LIGHT_GREEN)] {
            if let Some(text) = msg.clone() {
                egui::Window::new(title)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
                    .show(&ctx, |ui| {
                        ui.label(RichText::new(text).color(color));
                        if ui.button("OK").clicked() {
                            *msg = None;
                        }
                    });
            }
        }
    }
}

/// Znaczek w prawym górnym rogu ikony kroku historii.
fn draw_badge(painter: &egui::Painter, rect: egui::Rect, badge: crate::history::Badge) {
    use crate::history::Badge;
    let c = egui::pos2(rect.right() - 7.0, rect.top() + 7.0);
    let color = match badge {
        Badge::None => return,
        Badge::Add | Badge::On => Color32::from_rgb(60, 170, 90),
        Badge::Remove => Color32::from_rgb(210, 70, 60),
        Badge::Off => Color32::from_gray(110),
        Badge::Edit => Color32::from_rgb(220, 160, 50),
        Badge::Curves | Badge::Order => Color32::from_rgb(61, 127, 224),
    };
    painter.circle(c, 5.5, color, egui::Stroke::new(1.0, Color32::from_rgb(17, 19, 22)));
    let w = egui::Stroke::new(1.5, Color32::WHITE);
    let d = 2.8;
    match badge {
        Badge::Add => {
            painter.line_segment([c - egui::vec2(d, 0.0), c + egui::vec2(d, 0.0)], w);
            painter.line_segment([c - egui::vec2(0.0, d), c + egui::vec2(0.0, d)], w);
        }
        Badge::Remove => {
            painter.line_segment([c - egui::vec2(d, 0.0), c + egui::vec2(d, 0.0)], w);
        }
        Badge::Off => {
            painter.line_segment([c + egui::vec2(-d * 0.8, d * 0.8), c + egui::vec2(d * 0.8, -d * 0.8)], w);
        }
        Badge::Edit => {
            painter.circle_filled(c, 1.6, Color32::WHITE);
        }
        Badge::Curves => {
            painter.line_segment([c + egui::vec2(-d, d * 0.6), c + egui::vec2(0.0, -d * 0.6)], w);
            painter.line_segment([c + egui::vec2(0.0, -d * 0.6), c + egui::vec2(d, d * 0.6)], w);
        }
        Badge::Order => {
            painter.line_segment([c - egui::vec2(0.0, d), c + egui::vec2(0.0, d)], w);
            painter.line_segment([c - egui::vec2(0.0, d), c + egui::vec2(-1.8, -1.0)], w);
            painter.line_segment([c - egui::vec2(0.0, d), c + egui::vec2(1.8, -1.0)], w);
        }
        Badge::On => {
            painter.line_segment([c + egui::vec2(-d, 0.0), c + egui::vec2(-0.6, d * 0.8)], w);
            painter.line_segment([c + egui::vec2(-0.6, d * 0.8), c + egui::vec2(d, -d * 0.8)], w);
        }
        Badge::None => {}
    }
}
