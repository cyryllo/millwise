//! Widok 3D: półfabrykat, kontury, siatka i osie. Później ścieżki narzędzia
//! i symulacja usuwania materiału.

use cam_i18n::tr;
mod camera;
mod renderer;

use crate::doc::Document;
use crate::view2d::colors;
use camera::OrbitCamera;
use eframe::egui::{self, Color32, Rect, Sense};
use glam::Vec3;
use renderer::{LineVertex, MeshVertex, Renderer, SceneData};

pub struct View3D {
    camera: OrbitCamera,
    renderer: Option<Renderer>,
    built_revision: u64,
    scene_center: Vec3,
    scene_radius: f32,
    pub fit_requested: bool,
    /// Ustaw widok od spodu przy następnej klatce (dla programisty).
    pub bottom_view: bool,
    built_key: Option<(u64, u8, u8)>,
    sim_key: Option<(u64, u64)>,
    progress_key: Option<(u64, u64)>,
    auto_fit: bool,
    last_size: egui::Vec2,
    /// Zakładka modelu: pokazywać ścieżki i półfabrykat.
    show_paths: bool,
    show_stock: bool,
    /// Klik lewym przyciskiem w widok (bez przeciągania) — odbiera wywołujący.
    pub clicked: bool,
}

impl Default for View3D {
    fn default() -> Self {
        Self {
            camera: OrbitCamera::default(),
            renderer: None,
            built_revision: u64::MAX,
            scene_center: Vec3::ZERO,
            scene_radius: 100.0,
            fit_requested: true,
            bottom_view: false,
            built_key: None,
            sim_key: None,
            progress_key: None,
            auto_fit: true,
            last_size: egui::Vec2::ZERO,
            show_paths: true,
            show_stock: true,
            clicked: false,
        }
    }
}

/// Do czego służy widok 3D.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    /// Symulacja obróbki (z przyciskiem zamknięcia).
    Simulation,
    /// Zakładka „Model 3D”: model, półfabrykat i ścieżki — praca nad operacjami 3D.
    Model,
}

impl View3D {
    /// Zwraca true, gdy użytkownik zamknął wizualizację.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        frame: &eframe::Frame,
        doc: &Document,
        sim: Option<&SimDisplay>,
        kind: ViewKind,
    ) -> bool {
        let mut close = false;
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let Some(rs) = frame.wgpu_render_state() else {
            ui.painter_at(rect).text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr!("Brak wgpu — widok 3D niedostępny"),
                egui::FontId::proportional(14.0),
                Color32::LIGHT_RED,
            );
            return false;
        };
        let renderer = self.renderer.get_or_insert_with(|| Renderer::new(&rs.device));

        let model_view = kind == ViewKind::Model;
        let opts = SceneOpts {
            stock_box: sim.is_none_or(|s| !s.show_material) && (!model_view || self.show_stock),
            paths: sim.is_none_or(|s| s.show_paths) && (!model_view || self.show_paths),
            rapids: sim.is_none_or(|s| s.show_rapids) && !model_view,
            contours: sim.is_none(),
            side: sim.and_then(|s| s.side),
            model: sim.is_none_or(|s| s.show_model),
            // zakładka modelu i symulacja „Ścieżki”: model docelowy jako pełna bryła
            model_solid: kind == ViewKind::Model || sim.is_some_and(|s| s.progress.is_some()),
            faint_paths: sim.is_some_and(|s| s.progress.is_some()),
        };
        let scene_key = (doc.revision, opts.key(), opts.key2());
        if self.built_key != Some(scene_key) {
            let (scene, center, radius) = build_scene(doc, &opts);
            renderer.upload(&rs.device, &scene);
            let first = self.built_revision == u64::MAX || doc.revision == 1;
            self.built_revision = doc.revision;
            self.built_key = Some(scene_key);
            self.scene_center = center;
            self.scene_radius = radius;
            if first {
                self.fit_requested = true;
            }
        }

        // --- symulacja: materiał i narzędzie ---
        match sim {
            Some(sd) if sd.show_material => {
                if self.sim_key != Some(sd.key) {
                    let n = sim_mesh_chunks(sd.mesh, sd.top, sd.bottom, renderer::CHUNK_VERTS, |k, v, i| {
                        renderer.upload_sim_chunk(&rs.device, &rs.queue, k, v, i)
                    });
                    renderer.set_sim_chunks(n);
                    self.sim_key = Some(sd.key);
                }
            }
            _ => {
                if self.sim_key.is_some() {
                    renderer.set_sim_chunks(0);
                    self.sim_key = None;
                }
            }
        }
        // symulacja „Ścieżki”: linie postępu (przebudowa tylko po zmianie czasu)
        let pkey = sim.and_then(|s| s.progress.map(|p| p.1));
        if self.progress_key != pkey {
            let lines: Vec<LineVertex> = sim
                .and_then(|s| s.progress)
                .map(|(segs, _)| {
                    let mut v = Vec::with_capacity(segs.len() * 2);
                    for &(a, b, c) in segs {
                        let c = lin(c);
                        v.push(LineVertex { pos: [a.x as f32, a.y as f32, a.z as f32], color: c });
                        v.push(LineVertex { pos: [b.x as f32, b.y as f32, b.z as f32], color: c });
                    }
                    v
                })
                .unwrap_or_default();
            renderer.upload_progress_lines(&rs.device, &rs.queue, &lines);
            self.progress_key = pkey;
        }
        let tool_lines = sim.and_then(|s| s.tool).map(|(p, r, len, col)| tool_outline(p, r, len, col)).unwrap_or_default();
        renderer.upload_dynamic_lines(&rs.device, &rs.queue, &tool_lines);

        if std::mem::take(&mut self.bottom_view) {
            self.camera.set_bottom();
        }
        // --- wejście ---
        if resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            self.camera.orbit(d.x, d.y);
        }
        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
            self.auto_fit = false;
        }
        if rect.size() != self.last_size {
            self.last_size = rect.size();
            if self.auto_fit {
                self.fit_requested = true;
            }
        }
        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            self.camera.pan(d.x, d.y, rect.height());
        }
        if resp.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let f = (-scroll * 0.0025).exp() / zoom;
            if f != 1.0 {
                self.camera.zoom(f);
                self.auto_fit = false;
            }
        }
        if resp.double_clicked() {
            self.fit_requested = true;
        } else if resp.clicked() {
            self.clicked = true;
        }

        // --- render ---
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        let aspect = rect.width() / rect.height().max(1.0);
        if self.fit_requested {
            self.camera.fit(self.scene_center, self.scene_radius, aspect);
            self.fit_requested = false;
            self.auto_fit = true;
        }
        let vp = self.camera.view_proj(aspect, self.scene_radius);
        let tex = renderer.render(rs, size, vp);
        ui.painter_at(rect).image(
            tex,
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        // --- zero programu: symbol i podpisane osie (nakładka 2D) ---
        let project = |p: Vec3| -> Option<egui::Pos2> {
            let c = vp * p.extend(1.0);
            if c.w <= 1e-6 {
                return None;
            }
            let ndc = c.truncate() / c.w;
            Some(egui::pos2(rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - ndc.y) * 0.5 * rect.height()))
        };
        if let Some(o) = project(Vec3::ZERO).filter(|o| rect.contains(*o)) {
            let painter = ui.painter_at(rect);
            for (axis, col, label) in [
                (Vec3::X, crate::view2d::colors::AXIS_X, "X"),
                (Vec3::Y, crate::view2d::colors::AXIS_Y, "Y"),
                (Vec3::Z, Color32::from_rgb(90, 130, 235), "Z"),
            ] {
                // długość osi ~ 60 px na ekranie niezależnie od powiększenia
                if let Some(e) = project(axis * (self.scene_radius * 0.1).max(5.0)) {
                    let d = e - o;
                    if d.length() > 0.5 {
                        crate::view2d::draw_axis_arrow(&painter, o, d, 60.0, col, label);
                    }
                }
            }
            crate::view2d::draw_zero_symbol(&painter, o, 9.0);
            painter.text(
                o + egui::vec2(-10.0, 10.0),
                egui::Align2::RIGHT_TOP,
                "X0 Y0 Z0",
                egui::FontId::proportional(11.0),
                Color32::from_gray(225),
            );
        }

        // --- pasek widoków (w zakładce modelu na dole — u góry jest pasek narzędzia) ---
        let bar = match kind {
            ViewKind::Simulation => Rect::from_min_size(rect.min + egui::vec2(10.0, 10.0), egui::vec2(rect.width() - 20.0, 40.0)),
            ViewKind::Model => Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.bottom() - 34.0), egui::vec2(rect.width() - 20.0, 26.0)),
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
            ui.horizontal(|ui| {
                if kind == ViewKind::Simulation {
                    let b = egui::Button::new(
                        egui::RichText::new(tr!("Zamknij symulację  (Esc)")).strong().color(Color32::WHITE).size(14.0),
                    )
                    .fill(Color32::from_rgb(61, 127, 224))
                    .min_size(egui::vec2(0.0, 38.0))
                    .corner_radius(8.0);
                    if ui.add(b).clicked() {
                        close = true;
                    }
                    ui.add_space(10.0);
                }
                if ui.small_button(tr!("Izo")).clicked() {
                    self.camera.set_iso();
                }
                if ui.small_button(tr!("Góra")).clicked() {
                    self.camera.set_top();
                }
                if ui.small_button(tr!("Przód")).clicked() {
                    self.camera.set_front();
                }
                if ui.small_button(tr!("Spód")).clicked() {
                    self.camera.set_bottom();
                }
                if ui.small_button(tr!("Dopasuj")).clicked() {
                    self.fit_requested = true;
                }
                ui.checkbox(&mut self.camera.orthographic, tr!("Ortho"));
            });
        });
        // zakładka modelu: przełączniki w osobnym wierszu nad przyciskami widoków
        if kind == ViewKind::Model {
            let row = Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.bottom() - 62.0), egui::vec2(rect.width() - 20.0, 26.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_paths, tr!("Ścieżki"));
                    ui.checkbox(&mut self.show_stock, tr!("Półfabrykat"));
                });
            });
        }
        close
    }
}

fn lin(c: Color32) -> [f32; 4] {
    egui::Rgba::from(c).to_array()
}

fn line(out: &mut Vec<LineVertex>, a: Vec3, b: Vec3, color: [f32; 4]) {
    out.push(LineVertex { pos: a.to_array(), color });
    out.push(LineVertex { pos: b.to_array(), color });
}

/// Krok siatki 1/2/5·10^n bliski `target`.
fn nice_step(target: f32) -> f32 {
    let base = 10f32.powf(target.max(1e-3).log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if base * m >= target {
            return base * m;
        }
    }
    base * 10.0
}

/// Dane symulacji do wyświetlenia.
pub struct SimDisplay<'a> {
    pub mesh: &'a cam_sim::SimMesh,
    pub top: f32,
    pub bottom: f32,
    /// (identyfikator symulacji, wersja mapy) — zmiana = przebudowa siatki.
    pub key: (u64, u64),
    /// Narzędzie: czubek, promień, długość części roboczej, kolor.
    pub tool: Option<(cam_ops::P3, f64, f64, Color32)>,
    pub show_material: bool,
    pub show_paths: bool,
    pub show_rapids: bool,
    pub show_model: bool,
    /// Obróbka dwustronna: pokazywana strona (B w układzie odwróconej płyty).
    pub side: Option<cam_ops::Side>,
    /// Tryb „Ścieżki” (bez materiału): linie postępu (od, do, kolor) i ich wersja.
    pub progress: Option<(&'a [(cam_ops::P3, cam_ops::P3, Color32)], (u64, u64))>,
}

struct SceneOpts {
    stock_box: bool,
    paths: bool,
    rapids: bool,
    contours: bool,
    side: Option<cam_ops::Side>,
    model: bool,
    /// Model jako nieprzezroczysta, cieniowana bryła (zakładka „Model 3D”).
    model_solid: bool,
    /// Ścieżki przyciemnione (pod liniami postępu w symulacji „Ścieżki”).
    faint_paths: bool,
}

impl SceneOpts {
    fn key(&self) -> u8 {
        let side = match self.side {
            None => 0,
            Some(cam_ops::Side::A) => 1,
            Some(cam_ops::Side::B) => 2,
        };
        self.stock_box as u8
            | (self.paths as u8) << 1
            | (self.rapids as u8) << 2
            | (self.contours as u8) << 3
            | side << 4
            | (self.model as u8) << 6
            | (self.model_solid as u8) << 7
    }

    fn key2(&self) -> u8 {
        self.faint_paths as u8
    }
}

fn build_scene(doc: &Document, opts: &SceneOpts) -> (SceneData, Vec3, f32) {
    let mut s = SceneData::default();
    let t = doc.stock.thickness as f32;

    let (min, max) = match doc.stock_box() {
        Some(b) => (Vec3::new(b.min.x as f32, b.min.y as f32, -t), Vec3::new(b.max.x as f32, b.max.y as f32, 0.0)),
        None => (Vec3::new(0.0, 0.0, -t), Vec3::new(100.0, 100.0, 0.0)),
    };
    let size = max - min;
    let center = (min + max) * 0.5;
    let radius = size.length() * 0.5;

    // model 3D (STL): półprzezroczysty — w materiale widać go tam, gdzie materiał jest już zdjęty
    if let (true, Some(m)) = (opts.model, &doc.stock.model) {
        let (col, out) = if opts.model_solid {
            ([0.55, 0.68, 0.85, 1.0], &mut s.opaque)
        } else {
            ([0.42, 0.62, 0.92, 0.45], &mut s.transparent)
        };
        for t in m.placed() {
            let p = t.map(|v| Vec3::new(v[0], v[1], v[2]));
            let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
            for v in p {
                out.push(MeshVertex { pos: v.to_array(), normal: n.to_array(), color: col });
            }
        }
    }

    // siatka na poziomie stołu (spód materiału)
    let step = nice_step(size.x.max(size.y) / 10.0);
    let gz = -t - 0.01;
    let g0 = ((min.truncate() - glam::Vec2::splat(step * 2.0)) / step).floor() * step;
    let g1 = ((max.truncate() + glam::Vec2::splat(step * 2.0)) / step).ceil() * step;
    let grid_col = lin(colors::GRID_MAJOR);
    let mut x = g0.x;
    while x <= g1.x + 1e-3 {
        line(&mut s.lines, Vec3::new(x, g0.y, gz), Vec3::new(x, g1.y, gz), grid_col);
        x += step;
    }
    let mut y = g0.y;
    while y <= g1.y + 1e-3 {
        line(&mut s.lines, Vec3::new(g0.x, y, gz), Vec3::new(g1.x, y, gz), grid_col);
        y += step;
    }

    // osie w zerze programu
    let al = (size.x.max(size.y) * 0.12).max(10.0);
    line(&mut s.lines, Vec3::ZERO, Vec3::X * al, lin(colors::AXIS_X));
    line(&mut s.lines, Vec3::ZERO, Vec3::Y * al, lin(colors::AXIS_Y));
    line(&mut s.lines, Vec3::ZERO, Vec3::Z * al, lin(Color32::from_rgb(80, 120, 230)));

    // półfabrykat: przezroczysta bryła + krawędzie
    if doc.bbox.is_some() {
        if opts.stock_box {
            let a = if opts.model_solid { 0.12 } else { 0.22 };
            let base = egui::Rgba::from(Color32::from_rgb(196, 160, 110));
            let col = [base.r() * a, base.g() * a, base.b() * a, a];
            push_box(&mut s.transparent, min, max, col);
        }
        let edge = lin(Color32::from_rgb(150, 130, 100));
        let c = |i: u32| {
            Vec3::new(
                if i & 1 != 0 { max.x } else { min.x },
                if i & 2 != 0 { max.y } else { min.y },
                if i & 4 != 0 { max.z } else { min.z },
            )
        };
        for (i, j) in [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
            line(&mut s.lines, c(i), c(j), edge);
        }
    }

    // kontury na górnej powierzchni materiału
    let z = 0.02;
    for (i, c) in doc.contours.iter().enumerate() {
        if !opts.contours || !doc.is_visible(i) || c.flat.len() < 2 {
            continue;
        }
        let color = if doc.selected[i] {
            colors::SELECTED
        } else if c.contour.closed {
            colors::CLOSED
        } else {
            colors::OPEN
        };
        let col = lin(color);
        let p = |v: cam_geom::Vec2| Vec3::new(v.x as f32, v.y as f32, z);
        for w in c.flat.windows(2) {
            line(&mut s.lines, p(w[0]), p(w[1]), col);
        }
        if c.contour.closed {
            line(&mut s.lines, p(*c.flat.last().unwrap()), p(c.flat[0]), col);
        }
    }

    // ścieżki narzędzia
    for (i, (op, cache)) in doc.ops.iter().zip(&doc.op_cache).enumerate() {
        let Some(cache) = cache else { continue };
        if !op.enabled || (!opts.paths && !opts.rapids) || opts.side.is_some_and(|s| s != op.side) {
            continue;
        }
        // strona B w symulacji: ścieżka w układzie odwróconej płyty (tak jak w G-code)
        let raw;
        let pts: &[(cam_ops::P3, bool)] = if opts.side == Some(cam_ops::Side::B) {
            raw = cache.result.toolpath.polyline(0.01);
            &raw
        } else {
            &cache.display
        };
        let selected = doc.selected_op == Some(i);
        let base = crate::view2d::op_color(i);
        let col = if opts.faint_paths {
            lin(Color32::from_gray(150).gamma_multiply(0.22))
        } else {
            lin(if selected || doc.selected_op.is_none() { base } else { base.gamma_multiply(0.3) })
        };
        let rapid = lin(Color32::from_rgb(220, 60, 60).gamma_multiply(if selected { 0.8 } else { 0.3 }));
        let mut prev: Option<Vec3> = None;
        for &(p, is_rapid) in pts {
            let v = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
            if let Some(a) = prev {
                if (is_rapid && opts.rapids) || (!is_rapid && opts.paths) {
                    line(&mut s.lines, a, v, if is_rapid { rapid } else { col });
                }
            }
            prev = Some(v);
        }
    }

    (s, center, radius)
}

fn push_box(out: &mut Vec<MeshVertex>, min: Vec3, max: Vec3, color: [f32; 4]) {
    let faces: [(Vec3, [Vec3; 4]); 6] = [
        (Vec3::Z, [Vec3::new(min.x, min.y, max.z), Vec3::new(max.x, min.y, max.z), Vec3::new(max.x, max.y, max.z), Vec3::new(min.x, max.y, max.z)]),
        (-Vec3::Z, [Vec3::new(min.x, min.y, min.z), Vec3::new(min.x, max.y, min.z), Vec3::new(max.x, max.y, min.z), Vec3::new(max.x, min.y, min.z)]),
        (Vec3::X, [Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, max.y, min.z), Vec3::new(max.x, max.y, max.z), Vec3::new(max.x, min.y, max.z)]),
        (-Vec3::X, [Vec3::new(min.x, min.y, min.z), Vec3::new(min.x, min.y, max.z), Vec3::new(min.x, max.y, max.z), Vec3::new(min.x, max.y, min.z)]),
        (Vec3::Y, [Vec3::new(min.x, max.y, min.z), Vec3::new(min.x, max.y, max.z), Vec3::new(max.x, max.y, max.z), Vec3::new(max.x, max.y, min.z)]),
        (-Vec3::Y, [Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z), Vec3::new(max.x, min.y, max.z), Vec3::new(min.x, min.y, max.z)]),
    ];
    for (n, q) in faces {
        for idx in [0, 1, 2, 0, 2, 3] {
            out.push(MeshVertex { pos: q[idx].to_array(), normal: n.to_array(), color });
        }
    }
}

/// Kolory siatki materiału: góra — drewno, dno wycięć ciemniejsze z głębokością, ściany, boki.
/// Siatka materiału w kawałkach dla karty graficznej: każdy kawałek ma najwyżej
/// `max_verts` wierzchołków (z kolorami) i własne indeksy i od razu idzie do `emit(nr, …)`.
/// Wielka siatka (np. po gęstym wykańczaniu) nie mieści się w jednym buforze — i nie jest
/// kopiowana w całości naraz. Zwraca liczbę kawałków.
fn sim_mesh_chunks(
    mesh: &cam_sim::SimMesh,
    top: f32,
    bottom: f32,
    max_verts: usize,
    mut emit: impl FnMut(usize, &[MeshVertex], &[u32]),
) -> usize {
    let range = (top - bottom).max(1e-3);
    let wood_top = egui::Rgba::from(Color32::from_rgb(200, 164, 112));
    let cut_hi = egui::Rgba::from(Color32::from_rgb(178, 136, 88));
    let cut_lo = egui::Rgba::from(Color32::from_rgb(120, 86, 52));
    let wall = egui::Rgba::from(Color32::from_rgb(168, 126, 80));
    let side = egui::Rgba::from(Color32::from_rgb(150, 112, 70));
    let color = |v: &cam_sim::SimVertex| {
        let z = v.pos[2];
        let c = match v.kind {
            1 => wall,
            2 => side,
            _ if z >= top - 1e-3 => wood_top,
            _ => {
                let f = ((top - z) / range).clamp(0.0, 1.0);
                cut_hi * (1.0 - f) + cut_lo * f
            }
        };
        MeshVertex { pos: v.pos, normal: v.normal, color: [c.r(), c.g(), c.b(), 1.0] }
    };
    // numer wierzchołka w bieżącym kawałku: (kawałek + 1, numer); 0 = jeszcze nie użyty
    let mut local: Vec<(u32, u32)> = vec![(0, 0); mesh.verts.len()];
    let mut n = 0;
    let mut verts: Vec<MeshVertex> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut stamp = 1u32;
    for tri in mesh.indices.chunks_exact(3) {
        if verts.len() + 3 > max_verts {
            emit(n, &verts, &idx);
            n += 1;
            verts.clear();
            idx.clear();
            stamp += 1;
        }
        for &g in tri {
            let slot = &mut local[g as usize];
            if slot.0 != stamp {
                *slot = (stamp, verts.len() as u32);
                verts.push(color(&mesh.verts[g as usize]));
            }
            idx.push(slot.1);
        }
    }
    if !idx.is_empty() {
        emit(n, &verts, &idx);
        n += 1;
    }
    n
}

/// Sylwetka narzędzia z linii: okręgi na czubku i końcu ostrzy, tworzące, oś.
fn tool_outline(tip: cam_ops::P3, r: f64, len: f64, color: Color32) -> Vec<LineVertex> {
    let mut out = Vec::new();
    let col = lin(color);
    let shank = lin(Color32::from_gray(150));
    let t = Vec3::new(tip.x as f32, tip.y as f32, tip.z as f32);
    let r = r as f32;
    let len = (len as f32).max(r * 2.0);
    let n = 24;
    let ring = |z: f32, rr: f32, c: [f32; 4], out: &mut Vec<LineVertex>| {
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
            line(out, t + Vec3::new(rr * a0.cos(), rr * a0.sin(), z), t + Vec3::new(rr * a1.cos(), rr * a1.sin(), z), c);
        }
    };
    ring(0.0, r, col, &mut out);
    ring(len, r, col, &mut out);
    ring(len + len * 0.8, r * 0.9, shank, &mut out);
    for k in 0..4 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2;
        let o = Vec3::new(r * a.cos(), r * a.sin(), 0.0);
        line(&mut out, t + o, t + o + Vec3::Z * len, col);
        let o2 = o * 0.9;
        line(&mut out, t + o2 + Vec3::Z * len, t + o2 + Vec3::Z * (len * 1.8), shank);
    }
    line(&mut out, t, t + Vec3::Z * len * 2.2, lin(Color32::from_rgb(235, 90, 70)));
    out
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn big_mesh_is_split_into_chunks_with_the_same_triangles() {
        // siatka 300 × 300 węzłów, indeksowana jak mapa wysokości
        let n = 300;
        let mut mesh = cam_sim::SimMesh::default();
        for j in 0..n {
            for i in 0..n {
                mesh.verts.push(cam_sim::SimVertex { pos: [i as f32, j as f32, -(i % 7) as f32], normal: [0.0, 0.0, 1.0], kind: 0 });
            }
        }
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                let a = (j * n + i) as u32;
                mesh.indices.extend_from_slice(&[a, a + 1, a + n as u32 + 1, a, a + n as u32 + 1, a + n as u32]);
            }
        }
        let mut chunks: Vec<(Vec<MeshVertex>, Vec<u32>)> = Vec::new();
        let n = sim_mesh_chunks(&mesh, 0.0, -10.0, 10_000, |k, v, i| {
            assert_eq!(k, chunks.len());
            chunks.push((v.to_vec(), i.to_vec()));
        });
        assert_eq!(n, chunks.len());
        assert!(chunks.len() >= 9, "{}", chunks.len());
        let mut k = 0;
        for (v, idx) in &chunks {
            assert!(v.len() <= 10_000);
            for &l in idx {
                // ten sam wierzchołek co w całej siatce, w tej samej kolejności trójkątów
                assert_eq!(v[l as usize].pos, mesh.verts[mesh.indices[k] as usize].pos);
                k += 1;
            }
        }
        assert_eq!(k, mesh.indices.len());
        assert_eq!(sim_mesh_chunks(&cam_sim::SimMesh::default(), 0.0, -1.0, 100, |_, _, _| {}), 0);
    }
}
