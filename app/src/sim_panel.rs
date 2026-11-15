//! Symulacja usuwania materiału w trybie Wizualizacja.
//!
//! Całe liczenie odbywa się w osobnym wątku: najpierw przeliczenie całego
//! programu (klatki kluczowe do szybkiego przewijania + lista kolizji), potem
//! na żądanie — stan na dany czas i siatka 3D. Interfejs pokazuje ostatnią
//! gotową klatkę, więc nigdy nie przycina.

use cam_i18n::tr;
use crate::doc::Document;
use crate::ui_ops::fmt_time;
use crate::view2d::op_color;
use crate::view3d::SimDisplay;
use cam_geom::Aabb2;
use cam_ops::{ToolLibrary, P3};
use cam_post::Machine;
use cam_sim::solid::SolidSim;
use cam_sim::{Collision, Heightmap, SimMesh, SimOp, Simulator, ToolShape};
use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Instant;

/// Budżet pamięci na klatki kluczowe.
const KEYFRAME_BUDGET: usize = 160 * 1024 * 1024;

enum Cmd {
    /// Dokładna klatka (bryła, gdy silnik bryłowy).
    Seek(f64),
    /// Szybki podgląd z mapy wysokości — podczas przeciągania suwaka.
    Preview(f64),
}

/// Gotowa klatka symulacji.
pub struct Frame {
    time: f64,
    /// Klatka dokładna (false = szybki podgląd w trakcie przewijania).
    exact: bool,
    mesh: SimMesh,
    pos: Option<(P3, usize)>,
    serial: u64,
    millis: u128,
}

enum Msg {
    Progress(f32),
    Ready { duration: f64, op_start: Vec<f64>, collisions: Vec<Collision>, cell: f64, millis: u128 },
    Frame(Box<Frame>),
}

struct Worker {
    tx: Sender<Cmd>,
    rx: Receiver<Msg>,
}

type OpData = (ToolShape, Vec<(P3, bool, f64)>);

/// Silnik wizualizacji materiału.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// Bryła (operacje boolowskie) — dokładna geometria, wolniejsze przeliczenie.
    Solid,
    /// Mapa wysokości — bardzo szybka, dla dużych programów.
    Fast,
}

fn spawn_worker(
    ctx: egui::Context,
    data: Vec<OpData>,
    area: Aabb2,
    thickness: f64,
    resolution: usize,
    rapid: f64,
    engine: Engine,
) -> Worker {
    let (tx_cmd, rx_cmd) = channel::<Cmd>();
    let (tx_msg, rx_msg) = channel::<Msg>();
    std::thread::Builder::new()
        .name("millwise-sim".into())
        .spawn(move || {
            let started = Instant::now();
            let ops: Vec<SimOp> = data.iter().map(|(t, p)| SimOp { tool: t.clone(), points: p }).collect();
            let mut sim = Simulator::new(Heightmap::new(area, 0.0, -thickness, resolution), &ops, rapid);
            let dur = sim.duration();

            // --- przeliczenie całego programu z klatkami kluczowymi ---
            let per = sim.map.z.len().max(1) * 4;
            let n = (KEYFRAME_BUDGET / per).clamp(10, 300);
            let mut keys = vec![sim.snapshot()];
            for k in 1..=n {
                sim.advance_to(dur * k as f64 / n as f64);
                keys.push(sim.snapshot());
                if k % 4 == 0 || k == n {
                    let scale = if engine == Engine::Solid { 0.0 } else { 1.0 };
                    if tx_msg.send(Msg::Progress(scale * k as f32 / n as f32)).is_err() {
                        return; // symulacja porzucona
                    }
                    ctx.request_repaint();
                }
            }
            // --- silnik bryłowy: przeliczenie z klatkami kluczowymi ---
            let mut solid = (engine == Engine::Solid).then(|| {
                SolidSim::new(&sim, P3::new(area.min.x, area.min.y, -thickness), P3::new(area.max.x, area.max.y, 0.0))
            });
            if let Some(s) = &mut solid {
                // klatki kluczowe są tanie (kafelki współdzielone) — gęsto, żeby przewijanie było szybkie
                let every = (s.segment_count() / 400).max(16);
                let tx = tx_msg.clone();
                let c = ctx.clone();
                let ok = s.precompute(every, |p| {
                    let alive = tx.send(Msg::Progress(p)).is_ok();
                    c.request_repaint();
                    alive
                });
                if !ok {
                    return;
                }
            }
            let ready = Msg::Ready {
                duration: dur,
                op_start: sim.op_start.clone(),
                collisions: sim.collisions.clone(),
                cell: sim.map.cell,
                millis: started.elapsed().as_millis(),
            };
            if tx_msg.send(ready).is_err() {
                return;
            }

            // --- klatki na żądanie ---
            let mut serial = 0u64;
            let mut target = Some((0.0, true));
            loop {
                if let Some((t, exact)) = target.take() {
                    let t0 = Instant::now();
                    sim.seek_with(t, &keys);
                    serial += 1;
                    let mesh = match &mut solid {
                        Some(s) if exact => s.mesh_at(t),
                        _ => sim.mesh(),
                    };
                    let exact = exact || solid.is_none();
                    let frame = Frame { time: t, exact, mesh, pos: sim.position(), serial, millis: t0.elapsed().as_millis() };
                    if tx_msg.send(Msg::Frame(Box::new(frame))).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
                // czekaj na polecenie; z kolejki bierz tylko najnowsze
                let Ok(mut cmd) = rx_cmd.recv() else { return };
                loop {
                    match rx_cmd.try_recv() {
                        Ok(c) => cmd = c,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                match cmd {
                    Cmd::Seek(t) => target = Some((t, true)),
                    Cmd::Preview(t) => target = Some((t, false)),
                }
            }
        })
        .expect("nie można uruchomić wątku symulacji");
    Worker { tx: tx_cmd, rx: rx_msg }
}

pub struct SimState {
    worker: Option<Worker>,
    /// Klucz danych, z których zbudowano symulację.
    built_for: Option<(u64, u64, u64, usize, (bool, bool))>,
    /// Numer symulacji (zmienia się przy przebudowie).
    sim_serial: u64,
    ops: Vec<(usize, String)>,
    ready: bool,
    progress: f32,
    duration: f64,
    op_start: Vec<f64>,
    collisions: Vec<Collision>,
    cell: f64,
    millis: u128,
    /// Czas wyświetlany/odtwarzany w interfejsie.
    time: f64,
    pending: bool,
    frame: Option<Frame>,
    pub playing: bool,
    speed: f64,
    resolution: usize,
    engine: Engine,
    show_material: bool,
    pub show_paths: bool,
    show_rapids: bool,
    show_tool: bool,
    pub show_model: bool,
    /// Obróbka dwustronna: która strona jest symulowana, i czy w ogóle są dwie.
    pub side: cam_ops::Side,
    sides: bool,
    /// Po przeliczeniu od razu przewiń do końca (dla programisty / zrzutów ekranu).
    pub jump_to_end: bool,
    /// Po przeliczeniu przewiń do ułamka programu (dla programisty).
    pub jump_to_fraction: Option<f64>,
    empty_mesh: SimMesh,
    /// Suwak czasu jest przeciągany — wtedy szybki podgląd zamiast bryły.
    scrubbing: bool,
    /// Silnik wybierany automatycznie (dopóki użytkownik nie wybierze sam).
    engine_auto: bool,
    /// Automatycznie wybrano silnik „Szybki”, bo program jest bardzo gęsty (liczba ruchów).
    dense: Option<usize>,
}

/// Od tylu ruchów silnik bryłowy liczyłby za długo — automatycznie „Szybki”.
const DENSE_MOVES: usize = 40_000;

impl Default for SimState {
    fn default() -> Self {
        Self {
            worker: None,
            built_for: None,
            sim_serial: 0,
            ops: Vec::new(),
            ready: false,
            progress: 0.0,
            duration: 0.0,
            op_start: Vec::new(),
            collisions: Vec::new(),
            cell: 0.0,
            millis: 0,
            time: 0.0,
            pending: false,
            frame: None,
            playing: false,
            speed: 20.0,
            resolution: 400,
            engine: Engine::Solid,
            show_material: true,
            show_paths: true,
            show_rapids: false,
            show_tool: true,
            show_model: true,
            side: cam_ops::Side::A,
            sides: false,
            jump_to_end: false,
            jump_to_fraction: None,
            empty_mesh: SimMesh::default(),
            scrubbing: false,
            engine_auto: true,
            dense: None,
        }
    }
}

impl SimState {
    /// Uruchamia przeliczenie od nowa, gdy zmieniły się ścieżki, półfabrykat lub dokładność.
    pub fn sync(&mut self, ctx: &egui::Context, doc: &Document, lib: &ToolLibrary, machine: &Machine) {
        self.sides = doc.stock.two_sided.is_some() && doc.ops.iter().any(|o| o.enabled && o.side == cam_ops::Side::B);
        let side_b = self.sides && self.side == cam_ops::Side::B;
        if self.engine_auto {
            let moves: usize = doc
                .ops
                .iter()
                .zip(&doc.op_cache)
                .filter(|(op, _)| op.enabled && (!self.sides || op.side == self.side))
                .filter_map(|(_, c)| c.as_ref())
                .map(|c| c.result.toolpath.moves.len())
                .sum();
            if moves > DENSE_MOVES {
                if self.dense.is_none() {
                    self.engine = Engine::Fast;
                    self.resolution = self.resolution.max(800);
                }
                self.dense = Some(moves);
            } else {
                if self.dense.is_some() {
                    self.engine = Engine::Solid;
                }
                self.dense = None;
            }
        }
        let key = (
            doc.paths_revision,
            doc.stock.thickness.to_bits(),
            doc.stock.margin.to_bits(),
            self.resolution,
            (self.engine == Engine::Solid, side_b),
        );
        if self.built_for == Some(key) {
            return;
        }
        self.built_for = Some(key);
        self.worker = None; // stary wątek zakończy się sam
        self.playing = false;
        self.ready = false;
        self.progress = 0.0;
        self.pending = false;
        self.frame = None;
        self.time = 0.0;
        self.collisions.clear();
        self.sim_serial += 1;
        self.ops.clear();
        let Some(area) = doc.stock_box() else { return };
        let mut data: Vec<OpData> = Vec::new();
        for (i, (op, cache)) in doc.ops.iter().zip(&doc.op_cache).enumerate() {
            let (Some(cache), Some(tool)) = (cache, lib.get(op.tool_id)) else { continue };
            // dwie strony: tylko operacje wybranej strony (strona B w układzie odwróconej płyty)
            if self.sides && op.side != self.side {
                continue;
            }
            if !op.enabled || cache.result.toolpath.moves.is_empty() {
                continue;
            }
            data.push((ToolShape::new(&tool.kind, tool.diameter), cache.result.toolpath.polyline_with_feed(0.01)));
            self.ops.push((i, op.name.clone()));
        }
        if data.is_empty() {
            return;
        }
        self.worker =
            Some(spawn_worker(ctx.clone(), data, area, doc.stock.thickness, self.resolution, machine.rapid_xy, self.engine));
    }

    /// Odbiór wyników z wątku i postęp odtwarzania; wywoływane co klatkę.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let Some(w) = &self.worker else { return };
        while let Ok(msg) = w.rx.try_recv() {
            match msg {
                Msg::Progress(p) => self.progress = p,
                Msg::Ready { duration, op_start, collisions, cell, millis } => {
                    self.ready = true;
                    self.duration = duration;
                    self.op_start = op_start;
                    self.collisions = collisions;
                    self.cell = cell;
                    self.millis = millis;
                    self.pending = true; // wątek liczy klatkę t = 0
                    if std::mem::take(&mut self.jump_to_end) {
                        self.time = duration;
                    }
                    if let Some(f) = self.jump_to_fraction.take() {
                        self.time = duration * f.clamp(0.0, 1.0);
                    }
                }
                Msg::Frame(f) => {
                    self.frame = Some(*f);
                    self.pending = false;
                }
            }
        }
        if !self.ready {
            ctx.request_repaint();
            return;
        }
        if self.playing {
            let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
            self.time = (self.time + dt * self.speed).min(self.duration);
            if self.time >= self.duration {
                self.playing = false;
            }
            ctx.request_repaint();
        }
        let shown = self.frame.as_ref().map(|f| (f.time, f.exact));
        // nowa klatka: inny czas, albo po przewijaniu (był szybki podgląd) — dokładna
        let stale = shown.is_none_or(|(t, exact)| (t - self.time).abs() > 1e-9 || (!exact && !self.scrubbing));
        if !self.pending && stale {
            if let Some(w) = &self.worker {
                let cmd = if self.scrubbing { Cmd::Preview(self.time) } else { Cmd::Seek(self.time) };
                if w.tx.send(cmd).is_ok() {
                    self.pending = true;
                }
            }
        }
        if self.scrubbing || shown.is_some_and(|(_, exact)| !exact) {
            ctx.request_repaint();
        }
    }

    pub fn display<'a>(&'a self, doc: &Document, lib: &ToolLibrary) -> Option<SimDisplay<'a>> {
        let frame = self.frame.as_ref()?;
        let tool = if self.show_tool {
            frame.pos.and_then(|(p, oi)| {
                let (doc_i, _) = self.ops.get(oi)?;
                let t = lib.get(doc.ops.get(*doc_i)?.tool_id)?;
                Some((p, t.diameter * 0.5, t.flute_length, op_color(*doc_i)))
            })
        } else {
            None
        };
        Some(SimDisplay {
            mesh: if self.show_material { &frame.mesh } else { &self.empty_mesh },
            key: (self.sim_serial, if self.show_material { frame.serial } else { u64::MAX }),
            top: 0.0,
            bottom: -doc.stock.thickness as f32,
            tool,
            show_material: self.show_material,
            show_paths: self.show_paths,
            show_rapids: self.show_rapids,
            show_model: self.show_model,
            side: self.sides.then_some(self.side),
        })
    }

    /// `bottom` rysuje dodatkowe przyciski na samym dole panelu (G-code).
    pub fn panel(&mut self, ui: &mut egui::Ui, bottom: impl FnOnce(&mut egui::Ui)) {
        ui.add_space(6.0);
        ui.heading(tr!("Symulacja"));
        if self.sides {
            ui.horizontal(|ui| {
                ui.label(tr!("Strona"));
                ui.selectable_value(&mut self.side, cam_ops::Side::A, RichText::new(" A ").strong());
                ui.selectable_value(&mut self.side, cam_ops::Side::B, RichText::new(" B ").strong())
                    .on_hover_text(tr!("Płyta odwrócona; strona B liczona na pełnym materiale, bez obróbki strony A"));
            });
        }
        if self.worker.is_none() {
            ui.label(RichText::new(tr!("Brak ścieżek do symulacji — dodaj operacje w Projekcie 2D.")).weak());
            return;
        }
        if !self.ready {
            ui.add_space(8.0);
            ui.label(if self.engine == Engine::Solid { tr!("Przeliczanie bryły materiału…") } else { tr!("Przeliczanie symulacji…") });
            ui.add(egui::ProgressBar::new(self.progress).show_percentage().animate(true));
            ui.label(RichText::new(tr!("Cały program jest liczony raz — potem przewijanie i odtwarzanie są płynne.")).small().weak());
            return;
        }
        let dur = self.duration;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(tr!("Początek")).min_size(egui::vec2(0.0, 32.0))).clicked() {
                self.time = 0.0;
                self.playing = false;
            }
            let finished = self.time >= dur;
            let label = if self.playing { tr!("Pauza") } else if finished { tr!("Od nowa") } else { tr!("Odtwórz") };
            let b = egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
                .fill(Color32::from_rgb(61, 127, 224))
                .min_size(egui::vec2(90.0, 32.0));
            if ui.add(b).clicked() {
                if finished && !self.playing {
                    self.time = 0.0;
                }
                self.playing = !self.playing;
            }
            if ui.add(egui::Button::new(tr!("Koniec")).min_size(egui::vec2(0.0, 32.0))).clicked() {
                self.time = dur;
                self.playing = false;
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} / {}", fmt_time(self.time / 60.0), fmt_time(dur / 60.0))).monospace().size(15.0));
            if self.pending {
                ui.spinner();
            }
        });
        let slider = egui::Slider::new(&mut self.time, 0.0..=dur.max(0.001)).show_value(false);
        let r = ui.add_sized([ui.available_width(), 18.0], slider);
        // przeciąganie suwaka: szybki podgląd; dokładna bryła po puszczeniu
        self.scrubbing = r.dragged();
        ui.horizontal(|ui| {
            ui.label(tr!("Prędkość"));
            ui.add(egui::Slider::new(&mut self.speed, 1.0..=500.0).logarithmic(true).suffix("×").max_decimals(0));
        });

        ui.separator();
        ui.strong(tr!("Pokaż"));
        ui.checkbox(&mut self.show_material, tr!("Materiał (usuwanie)"));
        ui.checkbox(&mut self.show_paths, tr!("Ścieżki narzędzia"));
        ui.checkbox(&mut self.show_rapids, tr!("Ruchy szybkie (G0)"));
        ui.checkbox(&mut self.show_tool, tr!("Narzędzie"));
        ui.checkbox(&mut self.show_model, tr!("Model 3D")).on_hover_text(tr!("Model STL — widoczny tam, gdzie materiał jest zdjęty, albo po wyłączeniu materiału"));
        ui.horizontal(|ui| {
            ui.label(tr!("Silnik"));
            let a = ui
                .selectable_value(&mut self.engine, Engine::Solid, tr!("Bryła"))
                .on_hover_text(tr!("Dokładna bryła (operacje boolowskie) — czyste ściany, fazy i zaokrąglenia; dłuższe przeliczenie"));
            let b = ui
                .selectable_value(&mut self.engine, Engine::Fast, tr!("Szybki"))
                .on_hover_text(tr!("Mapa wysokości — natychmiastowe przeliczenie, dla bardzo dużych programów"));
            if a.clicked() || b.clicked() {
                self.engine_auto = false;
            }
        });
        if let (Some(n), true, Engine::Fast) = (self.dense, self.engine_auto, self.engine) {
            ui.label(
                RichText::new(tr!("Bardzo gęsta ścieżka ({} ruchów) — wybrano silnik Szybki; Bryła liczyłaby się bardzo długo.", n))
                    .small()
                    .color(Color32::from_rgb(230, 180, 80)),
            );
        }
        ui.add_enabled_ui(self.engine == Engine::Fast, |ui| ui.horizontal(|ui| {
            ui.label(tr!("Dokładność"));
            let label = |n: usize| match n {
                200 => tr!("zgrubna"),
                400 => tr!("normalna"),
                800 => tr!("wysoka"),
                _ => tr!("bardzo wysoka"),
            };
            egui::ComboBox::from_id_salt("sim_res").selected_text(label(self.resolution)).show_ui(ui, |ui| {
                for n in [200, 400, 800, 1600] {
                    ui.selectable_value(&mut self.resolution, n, tr!("{} ({} kom.)", label(n), n));
                }
            });
        }));
        let frame_ms = self.frame.as_ref().map_or(0, |f| f.millis);
        ui.label(
            RichText::new(tr!("komórka {:.2} mm · przeliczenie {} ms · klatka {} ms", self.cell, self.millis, frame_ms))
                .small()
                .weak(),
        );

        ui.separator();
        ui.strong(tr!("Operacje"));
        let cur_op = self.op_start.partition_point(|&s| s <= self.time).saturating_sub(1);
        let mut jump = None;
        for (k, (doc_i, name)) in self.ops.iter().enumerate() {
            let start = self.op_start[k];
            let end = self.op_start.get(k + 1).copied().unwrap_or(dur);
            let state = if self.time >= end && end > start {
                tr!("gotowe").to_string()
            } else if k == cur_op && self.time > start {
                format!("{:.0}%", (self.time - start) / (end - start).max(1e-9) * 100.0)
            } else {
                "—".to_string()
            };
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, op_color(*doc_i));
                if ui.selectable_label(k == cur_op, name).on_hover_text(tr!("Przewiń do początku operacji")).clicked() {
                    jump = Some(start);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(state).monospace().weak());
                });
            });
        }
        if let Some(t) = jump {
            self.time = t;
            self.playing = false;
        }

        ui.separator();
        if self.collisions.is_empty() {
            ui.label(RichText::new(tr!("Kolizje ruchów szybkich: brak")).color(Color32::from_rgb(120, 220, 140)));
        } else {
            ui.label(
                RichText::new(tr!("Kolizje: {} — ruch szybki (G0) w materiale!", self.collisions.len()))
                    .strong()
                    .color(Color32::from_rgb(255, 110, 100)),
            );
            for c in self.collisions.iter().take(8) {
                let name = self.ops.get(c.op).map_or("?", |o| o.1.as_str());
                ui.label(
                    RichText::new(tr!("{}: X {:.2} Y {:.2} Z {:.2} (głęb. {:.2})", name, c.at.x, c.at.y, c.at.z, c.depth))
                        .small()
                        .color(Color32::from_rgb(255, 150, 140)),
                );
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            bottom(ui);
            ui.add_space(6.0);
            if let Some((p, _)) = self.frame.as_ref().and_then(|f| f.pos) {
                ui.label(RichText::new(format!("X {:.3}  Y {:.3}  Z {:.3}", p.x, p.y, p.z)).monospace());
                ui.label(RichText::new(tr!("Pozycja narzędzia")).small().weak());
            }
        });
    }
}

#[cfg(test)]
mod perf {
    use super::*;

    /// Pomiar: cargo test --release -p millwise sim_perf -- --ignored --nocapture
    #[test]
    #[ignore]
    fn sim_perf() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut lib = ToolLibrary::default();
        let (mut doc, _) = crate::storage::load_project(&root.join("samples/demo.millwise"), &mut lib).unwrap();
        doc.regenerate(&lib, None);
        let data: Vec<OpData> = doc
            .ops
            .iter()
            .zip(&doc.op_cache)
            .filter_map(|(op, c)| {
                let t = lib.get(op.tool_id)?;
                Some((ToolShape::new(&t.kind, t.diameter), c.as_ref()?.result.toolpath.polyline_with_feed(0.01)))
            })
            .collect();
        let ops: Vec<SimOp> = data.iter().map(|(t, p)| SimOp { tool: t.clone(), points: p }).collect();
        let area = doc.stock_box().unwrap();
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -doc.stock.thickness, 400), &ops, 6000.0);
        let t = Instant::now();
        sim.advance_to(sim.duration());
        eprintln!("symulacja: {:?}, odcinków ścieżki: {}", t.elapsed(), sim.path.len());
        let t = Instant::now();
        let m = sim.mesh();
        eprintln!("siatka: {:?}, wierzchołków {}, trójkątów {}", t.elapsed(), m.verts.len(), m.indices.len() / 3);
        for frac in [0.137, 1.0] {
            sim.reset();
            sim.advance_to(sim.duration() * frac);
            let m = sim.mesh();
            let mut bad = 0;
            let mut example = None;
            for tri in m.indices.chunks(3) {
                let v = [tri[0], tri[1], tri[2]].map(|i| m.verts[i as usize]);
                if v.iter().all(|v| v.kind == 0) {
                    let zs = v.map(|v| v.pos[2]);
                    let span = zs.iter().cloned().fold(f32::MIN, f32::max) - zs.iter().cloned().fold(f32::MAX, f32::min);
                    if span > 0.5 {
                        bad += 1;
                        example.get_or_insert(v.map(|v| v.pos));
                    }
                }
            }
            eprintln!("t={frac}: trójkąty przez uskok: {bad}, przykład {example:?}");
        }

        // --- silnik bryłowy: profil czasu ---
        use cam_sim::solid;
        let min = P3::new(area.min.x, area.min.y, -doc.stock.thickness);
        let max = P3::new(area.max.x, area.max.y, 0.0);
        let mut sim2 = Simulator::new(Heightmap::new(area, 0.0, -doc.stock.thickness, 400), &ops, 6000.0);
        sim2.advance_to(0.0);
        let mut ss = solid::SolidSim::new(&sim2, min, max);
        let t = Instant::now();
        let every = (ss.segment_count() / 40).max(16);
        ss.precompute(every, |_| true);
        eprintln!("bryła: przeliczenie {:?} ({} odcinków), {}", t.elapsed(), ss.segment_count(), ss.stats());
        let d = sim2.duration();
        eprintln!("bryła końcowa: trójkątów {}, objętość {:.1}", ss.triangles_at(d), ss.volume_at(d));
    }

    /// Pomiar dla modelu 3D (zgrubna + wykańczająca):
    /// cargo test --release -p millwise sim_perf_3d -- --ignored --nocapture
    #[test]
    #[ignore]
    fn sim_perf_3d() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let lib = ToolLibrary::with_defaults();
        let mesh = cam_io::load_stl(&root.join("samples/model.stl")).unwrap();
        let mut doc = crate::doc::Document::default();
        doc.finish_load();
        doc.set_model(crate::model3d::Model3D::new("model.stl".into(), mesh, 1.0));
        doc.place_at_origin();
        let low = -doc.stock.model.as_ref().unwrap().heights().unwrap().lowest_top().unwrap();
        let flat = lib.tools.iter().find(|t| t.number == 1).unwrap().clone();
        let ball = lib.tools.iter().find(|t| matches!(t.kind, cam_ops::ToolKind::BallEnd)).unwrap().clone();
        let mk = |kind: cam_ops::OpKind, tool: &cam_ops::Tool| {
            let mut cut = cam_ops::CutParams { depth: low, ..Default::default() };
            cut.apply_tool(tool, cam_ops::DEFAULT_MATERIAL);
            cam_ops::Operation { side: cam_ops::Side::A, name: "x".into(), enabled: true, kind, tool_id: tool.id, cut, contours: vec![] }
        };
        doc.add_op(mk(cam_ops::OpKind::Rough3D(Default::default()), &flat));
        let so = std::env::var("PERF_STEPOVER").ok().and_then(|v| v.parse().ok()).unwrap_or(0.1);
        // PERF_ENGRAVER=1: frez grawerski 0,2 mm, odstęp `so` w mm
        let engr = lib.tools.iter().find(|t| matches!(t.kind, cam_ops::ToolKind::Engraver { .. })).unwrap().clone();
        let (fin, so) = if std::env::var_os("PERF_ENGRAVER").is_some() { (&engr, so / engr.diameter) } else { (&ball, so) };
        doc.add_op(mk(cam_ops::OpKind::Finish3D(cam_ops::Finish3DParams { stepover: so, ..Default::default() }), fin));
        doc.regenerate(&lib, None);
        let data: Vec<OpData> = doc
            .ops
            .iter()
            .zip(&doc.op_cache)
            .filter_map(|(op, c)| {
                let t = lib.get(op.tool_id)?;
                Some((ToolShape::new(&t.kind, t.diameter), c.as_ref()?.result.toolpath.polyline_with_feed(0.01)))
            })
            .collect();
        let ops: Vec<SimOp> = data.iter().map(|(t, p)| SimOp { tool: t.clone(), points: p }).collect();
        let area = doc.stock_box().unwrap();
        let th = doc.stock.thickness;
        // silnik szybki (mapa wysokości, „wysoka” dokładność): całość i przewijanie
        let mut hm = Simulator::new(Heightmap::new(area, 0.0, -th, 800), &ops, 6000.0);
        let t = Instant::now();
        hm.advance_to(hm.duration());
        let t_adv = t.elapsed();
        let t = Instant::now();
        let m = hm.mesh();
        eprintln!(
            "mapa wysokości: {} odcinków, przeliczenie {:?}, siatka {:?} ({} trójkątów, komórka {:.3} mm)",
            hm.path.len(),
            t_adv,
            t.elapsed(),
            m.indices.len() / 3,
            hm.map.cell
        );
        if std::env::var_os("PERF_ONLY_FAST").is_some() {
            return;
        }
        let mut sim = Simulator::new(Heightmap::new(area, 0.0, -th, 400), &ops, 6000.0);
        sim.advance_to(0.0);
        use cam_sim::solid;
        let mut ss = solid::SolidSim::new(&sim, P3::new(area.min.x, area.min.y, -th), P3::new(area.max.x, area.max.y, 0.0));
        let every = (ss.segment_count() / 400).max(16);
        let t = Instant::now();
        let mut last = (0.0f32, Instant::now());
        ss.precompute(every, |p| {
            if p - last.0 >= 0.1 {
                eprintln!("  postęp {:.0}%: {:?} od poprzedniego", p * 100.0, last.1.elapsed());
                last = (p, Instant::now());
            }
            true
        });
        eprintln!("bryła: {} odcinków, co {every}, przeliczenie {:?}, {}", ss.segment_count(), t.elapsed(), ss.stats());
        eprintln!("klatki kluczowe: {} trójkątów razem", ss.keyframe_triangles());
        // przewijanie ręczne: skoki w różne miejsca
        let dur = sim.duration();
        let mut seeks = Vec::new();
        for frac in [0.95, 0.6, 0.85, 0.7, 0.99, 0.75] {
            let t = Instant::now();
            let m = ss.mesh_at(dur * frac);
            seeks.push((frac, t.elapsed().as_millis(), m.indices.len() / 3));
        }
        eprintln!("przewijanie (ułamek, ms, trójkąty): {seeks:?}");
        // odtwarzanie: klatki co 1/30 s przy 20× (czas programu)
        for frac in [0.1, 0.5, 0.9] {
            let mut tt = dur * frac;
            let t = Instant::now();
            let m = ss.mesh_at(tt);
            let first = t.elapsed();
            let mut times = Vec::new();
            for _ in 0..10 {
                tt += 20.0 / 30.0;
                let t = Instant::now();
                let m = ss.mesh_at(tt);
                times.push((t.elapsed().as_millis(), m.indices.len() / 3));
            }
            eprintln!("t={frac}: skok {:?} ({} trójkątów), kolejne klatki (ms, trójkąty): {times:?}", first, m.indices.len() / 3);
        }
    }
}
