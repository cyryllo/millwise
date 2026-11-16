//! Widok 2D: rysowanie konturów, zoom/pan, zaznaczanie kliknięciem i ramką.

/// Co robi lewy przycisk myszy w widoku.
use cam_i18n::tr;
pub enum Interaction<'a> {
    Select,
    /// Przeciąganie przesuwa zaznaczone krzywe.
    Move,
    /// Klik dodaje/usuwa krzywą w operacji (`members` — krzywe aktywnej operacji).
    Op { kind: &'a cam_ops::OpKind, members: &'a [usize], color: eframe::egui::Color32 },
    /// Wskazanie punktu (z przyciąganiem).
    PickPoint,
    /// Klik krzywej zgłasza ją aplikacji (np. narożniki operacji); bez zaznaczania.
    PickContour,
    /// Wstawianie napisu: podgląd (łamane względem punktu wstawienia) za kursorem, klik = punkt.
    PlaceText { preview: &'a [Vec<cam_geom::Vec2>] },
    /// Pomiar: punkty z przyciąganiem; `a` — pierwszy punkt, `b` — drugi (gdy pomiar zamknięty).
    Measure { a: Option<cam_geom::Vec2>, b: Option<cam_geom::Vec2> },
}

/// Zdarzenia z widoku obsługiwane przez aplikację.
pub enum ViewEvent {
    Contour(usize),
    Box(Vec<usize>),
    Point(cam_geom::Vec2),
    /// Przesunięcie zaznaczenia [mm].
    Moved(cam_geom::Vec2),
    /// Kliknięcie w puste miejsce rysunku (bez krzywej pod kursorem).
    Empty,
}

use crate::doc::Document;
use cam_geom::{Aabb2, Vec2};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind};

pub mod colors {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(28, 30, 34);
    pub const GRID_MINOR: Color32 = Color32::from_rgb(38, 41, 46);
    pub const GRID_MAJOR: Color32 = Color32::from_rgb(50, 54, 61);
    pub const AXIS_X: Color32 = Color32::from_rgb(200, 70, 70);
    pub const AXIS_Y: Color32 = Color32::from_rgb(70, 180, 80);
    pub const CLOSED: Color32 = Color32::from_rgb(110, 170, 230);
    pub const OPEN: Color32 = Color32::from_rgb(230, 150, 70);
    pub const SELECTED: Color32 = Color32::from_rgb(255, 220, 60);
    pub const HOVER: Color32 = Color32::WHITE;
    pub const STOCK: Color32 = Color32::from_rgb(120, 120, 120);
}

pub struct View2D {
    /// Punkt świata w środku widoku [mm].
    center: Vec2,
    /// Piksele na milimetr.
    scale: f64,
    pub fit_requested: bool,
    box_start: Option<Pos2>,
    pub cursor_world: Option<Vec2>,
    pub hovered: Option<usize>,
    /// Widok dopasowany automatycznie i nieruszany przez użytkownika — dopasuj ponownie po zmianie rozmiaru.
    auto_fit: bool,
    last_size: egui::Vec2,
    /// Krótki komunikat na dole widoku (tekst, czas pojawienia się).
    flash: Option<(String, f64)>,
    /// Ścieżki rysowane przez kartę graficzną i stan, dla którego wysłano odcinki.
    gpu: Option<crate::view2d_gpu::PathsRenderer>,
    gpu_key: Option<PathsKey>,
}

/// Co wpływa na odcinki ścieżek na karcie: przeliczone ścieżki (adres i długość
/// danych każdej operacji), włączenie, średnica freza i zaznaczona operacja.
#[derive(PartialEq)]
struct PathsKey {
    selected: Option<usize>,
    ops: Vec<(usize, usize, bool, u64, u64)>,
}

impl PathsKey {
    fn new(doc: &Document, tool_diameters: &[f64]) -> Self {
        PathsKey {
            selected: doc.selected_op,
            ops: doc
                .ops
                .iter()
                .zip(&doc.op_cache)
                .enumerate()
                .map(|(i, (op, c))| {
                    let (ptr, len) = c.as_ref().map_or((0, 0), |c| (c.display.as_ptr() as usize, c.display.len()));
                    (ptr, len, op.enabled, tool_diameters.get(i).copied().unwrap_or(0.0).to_bits(), op.cut.top_z.to_bits())
                })
                .collect(),
        }
    }
}

impl Default for View2D {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            scale: 2.0,
            fit_requested: true,
            box_start: None,
            cursor_world: None,
            hovered: None,
            auto_fit: true,
            last_size: egui::Vec2::ZERO,
            flash: None,
            gpu: None,
            gpu_key: None,
        }
    }
}

impl View2D {
    fn to_screen(&self, rect: Rect, p: Vec2) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.scale) as f32,
            c.y - ((p.y - self.center.y) * self.scale) as f32,
        )
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> Vec2 {
        let c = rect.center();
        Vec2::new(
            self.center.x + (p.x - c.x) as f64 / self.scale,
            self.center.y - (p.y - c.y) as f64 / self.scale,
        )
    }

    fn fit(&mut self, rect: Rect, bb: Aabb2) {
        let size = bb.size();
        let sx = rect.width() as f64 / size.x.max(1e-3);
        let sy = rect.height() as f64 / size.y.max(1e-3);
        self.scale = (sx.min(sy) * 0.9).clamp(1e-3, 1e5);
        self.center = bb.center();
    }

    /// `tool_diameters[i]` — średnica narzędzia operacji `i` (do pasa szerokości freza).
    /// `work_area` — pole robocze maszyny (X, Y) rysowane od lewego dolnego rogu półfabrykatu.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        doc: &mut Document,
        tool_diameters: &[f64],
        inter: Interaction,
        work_area: Option<(f64, f64)>,
        rs: Option<&egui_wgpu::RenderState>,
    ) -> Vec<ViewEvent> {
        let mut events = Vec::new();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        if rect.size() != self.last_size {
            self.last_size = rect.size();
            if self.auto_fit {
                self.fit_requested = true;
            }
        }
        if self.fit_requested {
            self.auto_fit = true;
            let bb = doc.stock_box().unwrap_or(Aabb2 { min: Vec2::new(-10.0, -10.0), max: Vec2::new(100.0, 100.0) });
            self.fit(rect, bb);
            self.fit_requested = false;
        }

        // --- wejście ---
        let hover_pos = resp.hover_pos();
        self.cursor_world = hover_pos.map(|p| self.to_world(rect, p));

        if let Some(hp) = hover_pos {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.0025).exp() * zoom as f64;
            if factor != 1.0 {
                let before = self.to_world(rect, hp);
                self.scale = (self.scale * factor).clamp(1e-3, 1e5);
                self.auto_fit = false;
                let after = self.to_world(rect, hp);
                self.center = self.center + (before - after);
            }
        }
        if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
            let d = resp.drag_delta();
            self.center = self.center + Vec2::new(-d.x as f64 / self.scale, d.y as f64 / self.scale);
            self.auto_fit = false;
        }
        if resp.double_clicked() {
            self.fit_requested = true;
        }

        let pick_tol = 6.0 / self.scale;
        let picking_point = matches!(inter, Interaction::PickPoint | Interaction::Measure { .. } | Interaction::PlaceText { .. });
        self.hovered = if self.box_start.is_none() && !picking_point {
            self.cursor_world.and_then(|p| doc.pick(p, pick_tol))
        } else {
            None
        };
        let snapped = if picking_point {
            self.cursor_world.map(|p| doc.snap_point(p, 10.0 / self.scale))
        } else {
            None
        };

        let modifiers = ui.input(|i| i.modifiers);
        let additive = modifiers.shift || modifiers.command;
        let now = ui.input(|i| i.time);

        let clicked = resp.clicked_by(egui::PointerButton::Primary);
        match &inter {
            Interaction::Op { .. } if clicked && self.hovered.is_none() && !additive => events.push(ViewEvent::Empty),
            Interaction::Op { kind, .. } if clicked => {
                if let Some(i) = self.hovered {
                    if doc.contour_fits(kind, i) {
                        events.push(ViewEvent::Contour(i));
                    } else {
                        let msg = match kind {
                            cam_ops::OpKind::Pocket(_) => tr!("Kieszeń wymaga krzywej zamkniętej"),
                            cam_ops::OpKind::VCarve(_) => tr!("V-carving wymaga krzywej zamkniętej"),
                            cam_ops::OpKind::Drill(_) => tr!("Wiercenie wymaga okręgu"),
                            _ => tr!("Krzywa nie pasuje do operacji"),
                        };
                        self.flash = Some((msg.into(), now));
                    }
                }
            }
            Interaction::PickContour if clicked => match self.hovered {
                Some(i) => events.push(ViewEvent::Contour(i)),
                None if !additive => events.push(ViewEvent::Empty),
                None => {}
            },
            Interaction::PickPoint | Interaction::Measure { .. } | Interaction::PlaceText { .. } if clicked => {
                if let Some(p) = snapped {
                    events.push(ViewEvent::Point(p));
                }
            }
            Interaction::Select | Interaction::Move if clicked => match self.hovered {
                Some(i) if additive => {
                    let v = !doc.selected[i];
                    doc.set_selected(i, v);
                }
                Some(i) => {
                    doc.clear_selection();
                    doc.set_selected(i, true);
                }
                None if !additive => {
                    doc.clear_selection();
                    events.push(ViewEvent::Empty);
                }
                None => {}
            },
            _ => {}
        }

        let moving = matches!(inter, Interaction::Move);
        if moving && resp.dragged_by(egui::PointerButton::Primary) {
            if resp.drag_started_by(egui::PointerButton::Primary) {
                // przeciągnięcie niezaznaczonej krzywej najpierw ją zaznacza
                if let Some(i) = self.hovered.filter(|&i| !doc.selected[i]) {
                    if !additive {
                        doc.clear_selection();
                    }
                    doc.set_selected(i, true);
                }
            }
            let d = resp.drag_delta();
            if d != egui::Vec2::ZERO {
                events.push(ViewEvent::Moved(Vec2::new(d.x as f64 / self.scale, -d.y as f64 / self.scale)));
            }
        }

        if !moving && !picking_point && resp.drag_started_by(egui::PointerButton::Primary) {
            self.box_start = resp.interact_pointer_pos();
        }
        let mut box_rect = None;
        if let (Some(start), Some(cur)) = (self.box_start, resp.interact_pointer_pos().or(hover_pos)) {
            box_rect = Some((start, cur));
            if resp.drag_stopped() {
                let a = self.to_world(rect, start);
                let b = self.to_world(rect, cur);
                let mut bb = Aabb2::EMPTY;
                bb.add_point(a);
                bb.add_point(b);
                // jak w CAD: w prawo = całe w ramce, w lewo = przecinające
                let crossing = cur.x < start.x;
                let found = doc.in_box(bb, crossing);
                if matches!(inter, Interaction::Op { .. }) {
                    events.push(ViewEvent::Box(found));
                } else {
                    if !additive {
                        doc.clear_selection();
                    }
                    for i in found {
                        doc.set_selected(i, true);
                    }
                }
                self.box_start = None;
                box_rect = None;
            }
        }
        if !resp.dragged() && !resp.drag_stopped() {
            self.box_start = None;
        }

        // --- rysowanie ---
        painter.rect_filled(rect, 0.0, colors::BG);
        self.draw_grid(&painter, rect);

        if let Some(sb) = doc.stock_box() {
            let r = Rect::from_two_pos(self.to_screen(rect, sb.min), self.to_screen(rect, sb.max));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, colors::STOCK), StrokeKind::Middle);
            if let Some((wx, wy)) = work_area {
                let a = self.to_screen(rect, sb.min);
                let b = self.to_screen(rect, sb.min + Vec2::new(wx, wy));
                let col = Color32::from_rgb(61, 127, 224).gamma_multiply(0.6);
                let pts = [a, Pos2::new(b.x, a.y), b, Pos2::new(a.x, b.y), a];
                painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0, col), 8.0, 5.0));
                painter.text(
                    Pos2::new(a.x + 4.0, b.y + 4.0),
                    egui::Align2::LEFT_TOP,
                    tr!("pole robocze {:.0} × {:.0}", wx, wy),
                    egui::FontId::proportional(11.0),
                    col,
                );
            }
        }

        let view_bb = {
            let mut b = Aabb2::EMPTY;
            b.add_point(self.to_world(rect, rect.min));
            b.add_point(self.to_world(rect, rect.max));
            b
        };
        // model 3D z góry: trójkąty skierowane w górę, jaśniejsze wyżej
        if let Some(m) = &doc.stock.model {
            let fp = m.footprint();
            if intersects(&fp, &view_bb) {
                let h = m.size()[2].max(1e-3) as f32;
                let z_top = -(m.top_allowance as f32);
                let (lo, hi) = (Color32::from_rgb(52, 66, 84), Color32::from_rgb(130, 160, 196));
                let mut mesh = egui::Mesh::default();
                for t in m.placed() {
                    let (a, b, c) = (t[0], t[1], t[2]);
                    // tylko trójkąty widoczne z góry (normalna w górę)
                    if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) <= 0.0 {
                        continue;
                    }
                    let f = (((a[2] + b[2] + c[2]) / 3.0 - (z_top - h)) / h).clamp(0.0, 1.0);
                    let col = Color32::from_rgb(
                        (lo.r() as f32 + (hi.r() as f32 - lo.r() as f32) * f) as u8,
                        (lo.g() as f32 + (hi.g() as f32 - lo.g() as f32) * f) as u8,
                        (lo.b() as f32 + (hi.b() as f32 - lo.b() as f32) * f) as u8,
                    );
                    let base = mesh.vertices.len() as u32;
                    for p in [a, b, c] {
                        mesh.colored_vertex(self.to_screen(rect, Vec2::new(p[0] as f64, p[1] as f64)), col);
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                painter.add(egui::Shape::mesh(mesh));
                let r = Rect::from_two_pos(self.to_screen(rect, fp.min), self.to_screen(rect, fp.max));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(130, 160, 196)), StrokeKind::Middle);
            }
        }

        // kolejność: zwykłe, zaznaczone, podświetlony (na wierzchu)
        let (op_kind, members, op_col) = match &inter {
            Interaction::Op { kind, members, color } => (Some(*kind), *members, *color),
            _ => (None, &[][..], Color32::WHITE),
        };
        let in_op = |i: usize| members.contains(&i);
        let mut order: Vec<usize> = (0..doc.contours.len()).filter(|&i| doc.is_visible(i)).collect();
        order.sort_by_key(|&i| (Some(i) == self.hovered, doc.selected[i] || in_op(i)));
        for i in order {
            let c = &doc.contours[i];
            if !intersects(&c.bbox, &view_bb) {
                continue;
            }
            let member = in_op(i);
            let selected = doc.selected[i] || member;
            let hovered = Some(i) == self.hovered;
            let fits = op_kind.is_none_or(|k| doc.contour_fits(k, i));
            let color = if hovered && !fits {
                Color32::from_rgb(200, 80, 80)
            } else if hovered {
                colors::HOVER
            } else if member {
                op_col
            } else if op_kind.is_some() && !fits {
                Color32::from_gray(70)
            } else if selected {
                colors::SELECTED
            } else if c.contour.closed {
                colors::CLOSED
            } else {
                colors::OPEN
            };
            let width = if selected || hovered { 2.0 } else { 1.2 };
            let mut pts: Vec<Pos2> = c.flat.iter().map(|p| self.to_screen(rect, *p)).collect();
            if c.contour.closed {
                if let Some(f) = pts.first().copied() {
                    pts.push(f);
                }
            }
            painter.line(pts, Stroke::new(width, color));
            if selected {
                self.draw_direction(&painter, rect, c, color);
            }
        }

        match rs {
            Some(rs) => self.draw_toolpaths_gpu(ui, &painter, rect, doc, tool_diameters, rs),
            None => self.draw_toolpaths(&painter, rect, doc, tool_diameters),
        }
        self.draw_zero(&painter, rect);
        // obróbka dwustronna: oś odwracania (przerywana)
        if let Some(t) = &doc.stock.two_sided {
            let col = Color32::from_rgb(200, 120, 230);
            let (a, b) = match t.axis {
                crate::doc::FlipAxis::Y => (Pos2::new(self.to_screen(rect, Vec2::new(t.pos, 0.0)).x, rect.top()), Pos2::new(self.to_screen(rect, Vec2::new(t.pos, 0.0)).x, rect.bottom())),
                crate::doc::FlipAxis::X => (Pos2::new(rect.left(), self.to_screen(rect, Vec2::new(0.0, t.pos)).y), Pos2::new(rect.right(), self.to_screen(rect, Vec2::new(0.0, t.pos)).y)),
            };
            painter.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.2, col), 10.0, 6.0));
            painter.text(a + egui::vec2(6.0, 18.0), egui::Align2::LEFT_TOP, tr!("oś odwracania"), egui::FontId::proportional(11.0), col);
        }

        if let Some((a, b)) = box_rect {
            let r = Rect::from_two_pos(a, b);
            let crossing = b.x < a.x;
            let col = if crossing { Color32::from_rgb(90, 200, 120) } else { Color32::from_rgb(90, 150, 230) };
            painter.rect_filled(r, 0.0, col.gamma_multiply(0.12));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, col), StrokeKind::Middle);
        }

        // napis: podgląd za kursorem (lewy koniec linii bazowej w punkcie wstawienia)
        if let (Interaction::PlaceText { preview }, Some(at)) = (&inter, snapped) {
            let col = Color32::from_rgb(230, 150, 70).gamma_multiply(0.8);
            for poly in preview.iter() {
                let mut pts: Vec<Pos2> = poly.iter().map(|&q| self.to_screen(rect, at + q)).collect();
                if let Some(&f) = pts.first() {
                    pts.push(f);
                }
                painter.add(egui::Shape::line(pts, Stroke::new(1.2, col)));
            }
        }
        // pomiar: linia od A do B (albo do kursora) z długością
        if let Interaction::Measure { a: Some(a), b } = inter {
            if let Some(b) = b.or(snapped) {
                let (sa, sb) = (self.to_screen(rect, a), self.to_screen(rect, b));
                let col = Color32::from_rgb(250, 200, 70);
                painter.line_segment([sa, sb], Stroke::new(1.5, col));
                for s in [sa, sb] {
                    painter.circle_filled(s, 3.5, col);
                }
                let d = b - a;
                painter.text(
                    sa.lerp(sb, 0.5) + egui::vec2(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.3} mm", d.length()),
                    egui::FontId::proportional(14.0),
                    col,
                );
            }
        }
        if let Some(p) = snapped {
            let sp = self.to_screen(rect, p);
            let c = Color32::WHITE;
            painter.circle_stroke(sp, 6.0, Stroke::new(1.5, c));
            painter.line_segment([sp - egui::vec2(12.0, 0.0), sp + egui::vec2(12.0, 0.0)], Stroke::new(1.0, c));
            painter.line_segment([sp - egui::vec2(0.0, 12.0), sp + egui::vec2(0.0, 12.0)], Stroke::new(1.0, c));
            painter.text(
                sp + egui::vec2(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("X {:.3}  Y {:.3}", p.x, p.y),
                egui::FontId::monospace(11.0),
                c,
            );
        }
        if let Some((msg, t)) = &self.flash {
            if now - t < 2.5 {
                painter.text(
                    rect.center_bottom() + egui::vec2(0.0, -40.0),
                    egui::Align2::CENTER_CENTER,
                    msg,
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(255, 120, 100),
                );
                ui.ctx().request_repaint();
            } else {
                self.flash = None;
            }
        }

        if let Some(i) = self.hovered {
            let c = &doc.contours[i];
            let text = format!(
                "{} · {} · L = {:.2} mm{}",
                doc.layers[c.layer].name,
                if c.contour.closed { tr!("zamknięty") } else { tr!("otwarty") },
                c.length,
                if c.contour.closed { format!(" · A = {:.1} mm²", c.area) } else { String::new() }
            );
            if let Some(hp) = hover_pos {
                painter.text(
                    hp + egui::vec2(14.0, 14.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional(12.0),
                    Color32::from_gray(220),
                );
            }
        }
        events
    }

    /// Znacznik zera programu (X0 Y0); poza widokiem — wskaźnik przy krawędzi.
    fn draw_zero(&self, painter: &egui::Painter, rect: Rect) {
        let o = self.to_screen(rect, Vec2::ZERO);
        let inner = rect.shrink(28.0);
        if inner.contains(o) {
            draw_axis_arrow(painter, o, egui::vec2(1.0, 0.0), 64.0, colors::AXIS_X, "X");
            draw_axis_arrow(painter, o, egui::vec2(0.0, -1.0), 64.0, colors::AXIS_Y, "Y");
            draw_zero_symbol(painter, o, 10.0);
            painter.text(
                o + egui::vec2(-10.0, 10.0),
                egui::Align2::RIGHT_TOP,
                "X0 Y0",
                egui::FontId::proportional(11.0),
                Color32::from_gray(225),
            );
        } else {
            // zero poza widokiem: symbol przy krawędzi w jego kierunku
            let c = rect.center();
            let d = o - c;
            let k = ((inner.width() * 0.5) / d.x.abs().max(1e-3)).min((inner.height() * 0.5) / d.y.abs().max(1e-3));
            let p = c + d * k;
            draw_zero_symbol(painter, p, 7.0);
            let dir = d.normalized();
            let tip = p + dir * 18.0;
            let n = egui::vec2(-dir.y, dir.x);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, tip - dir * 8.0 + n * 5.0, tip - dir * 8.0 - n * 5.0],
                Color32::WHITE,
                Stroke::NONE,
            ));
            painter.text(p - dir * 14.0, egui::Align2::CENTER_CENTER, "zero", egui::FontId::proportional(10.0), Color32::from_gray(210));
        }
    }

    pub fn flash(&mut self, msg: impl Into<String>, now: f64) {
        self.flash = Some((msg.into(), now));
    }

    /// Ścieżki przez kartę graficzną: odcinki wysyłane tylko po zmianie, klatka to
    /// jedno narysowanie tekstury (przy gęstych ścieżkach 3D płynnie).
    fn draw_toolpaths_gpu(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        doc: &Document,
        tool_diameters: &[f64],
        rs: &egui_wgpu::RenderState,
    ) {
        let r = self.gpu.get_or_insert_with(|| crate::view2d_gpu::PathsRenderer::new(&rs.device));
        let key = PathsKey::new(doc, tool_diameters);
        if self.gpu_key.as_ref() != Some(&key) {
            r.upload(&rs.device, &toolpath_segments(doc, tool_diameters));
            self.gpu_key = Some(key);
        }
        if r.is_empty() {
            return;
        }
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        let tex = r.render(rs, size, self.center, self.scale, ppp);
        painter.image(tex, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }

    fn draw_toolpaths(&self, painter: &egui::Painter, rect: Rect, doc: &Document, tool_diameters: &[f64]) {
        for (i, (op, cache)) in doc.ops.iter().zip(&doc.op_cache).enumerate() {
            let Some(cache) = cache else { continue };
            if !op.enabled || cache.display.is_empty() {
                continue;
            }
            let selected = doc.selected_op == Some(i);
            let base = op_color(i);
            let color = if selected || doc.selected_op.is_none() { base } else { base.gamma_multiply(0.35) };
            let rapid_color = Color32::from_rgb(220, 60, 60).gamma_multiply(if selected { 0.9 } else { 0.4 });

            // pas o szerokości narzędzia — tylko dla zaznaczonej operacji
            if selected {
                let d = tool_diameters.get(i).copied().unwrap_or(0.0);
                let w = (d * self.scale) as f32;
                if w > 2.0 {
                    let band = base.gamma_multiply(0.18);
                    let mut run: Vec<Pos2> = Vec::new();
                    let mut prev: Option<cam_ops::P3> = None;
                    for &(p, rapid) in &cache.display {
                        if rapid || p.z > op.cut.top_z - 1e-6 {
                            if run.len() > 1 {
                                painter.add(egui::Shape::line(std::mem::take(&mut run), Stroke::new(w, band)));
                            }
                            run.clear();
                        } else {
                            if run.is_empty() {
                                if let Some(pp) = prev {
                                    run.push(self.to_screen(rect, pp.xy()));
                                }
                            }
                            run.push(self.to_screen(rect, p.xy()));
                        }
                        prev = Some(p);
                    }
                    if run.len() > 1 {
                        painter.add(egui::Shape::line(run, Stroke::new(w, band)));
                    }
                }
            }

            // linie ścieżki: posuwy ciągłe, ruchy szybkie przerywane
            let mut prev: Option<Pos2> = None;
            let mut run: Vec<Pos2> = Vec::new();
            for &(p, rapid) in &cache.display {
                let sp = self.to_screen(rect, p.xy());
                if rapid {
                    if run.len() > 1 {
                        painter.add(egui::Shape::line(std::mem::take(&mut run), Stroke::new(1.3, color)));
                    }
                    run.clear();
                    if let Some(pp) = prev {
                        if pp.distance(sp) > 1.0 {
                            painter.extend(egui::Shape::dashed_line(&[pp, sp], Stroke::new(1.0, rapid_color), 4.0, 4.0));
                        }
                    }
                } else {
                    if run.is_empty() {
                        if let Some(pp) = prev {
                            run.push(pp);
                        }
                    }
                    run.push(sp);
                }
                prev = Some(sp);
            }
            if run.len() > 1 {
                painter.add(egui::Shape::line(run, Stroke::new(1.3, color)));
            }
        }
    }

    /// Strzałka kierunku i punkt startowy konturu.
    fn draw_direction(&self, painter: &egui::Painter, rect: Rect, c: &crate::doc::DocContour, color: Color32) {
        if c.flat.len() < 2 {
            return;
        }
        let p0 = self.to_screen(rect, c.flat[0]);
        painter.circle_filled(p0, 3.5, color);
        // strzałka w połowie długości łamanej
        let total: f64 = c.flat.windows(2).map(|w| w[0].distance(w[1])).sum();
        let mut acc = 0.0;
        for w in c.flat.windows(2) {
            let l = w[0].distance(w[1]);
            if acc + l >= total * 0.5 && l > 0.0 {
                let t = (total * 0.5 - acc) / l;
                let p = self.to_screen(rect, w[0].lerp(w[1], t));
                let d = self.to_screen(rect, w[1]) - self.to_screen(rect, w[0]);
                if d.length() < 1e-3 {
                    break;
                }
                let d = d.normalized();
                let n = egui::vec2(-d.y, d.x);
                let s = 8.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![p + d * s, p - d * s * 0.6 + n * s * 0.6, p - d * s * 0.6 - n * s * 0.6],
                    color,
                    Stroke::NONE,
                ));
                break;
            }
            acc += l;
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        // krok siatki 1/2/5·10^n tak, by linie były co ≥ 12 px
        let target = 12.0 / self.scale;
        let mut step = 10f64.powf(target.log10().floor());
        for m in [1.0, 2.0, 5.0, 10.0] {
            if step * m >= target {
                step *= m;
                break;
            }
        }
        let major = step * 10.0;
        let a = self.to_world(rect, rect.left_bottom());
        let b = self.to_world(rect, rect.right_top());
        let draw = |s: f64, color: Color32| {
            let mut x = (a.x / s).floor() * s;
            while x <= b.x {
                let p = self.to_screen(rect, Vec2::new(x, 0.0));
                painter.line_segment([Pos2::new(p.x, rect.top()), Pos2::new(p.x, rect.bottom())], Stroke::new(1.0, color));
                x += s;
            }
            let mut y = (a.y / s).floor() * s;
            while y <= b.y {
                let p = self.to_screen(rect, Vec2::new(0.0, y));
                painter.line_segment([Pos2::new(rect.left(), p.y), Pos2::new(rect.right(), p.y)], Stroke::new(1.0, color));
                y += s;
            }
        };
        draw(step, colors::GRID_MINOR);
        draw(major, colors::GRID_MAJOR);
        let o = self.to_screen(rect, Vec2::ZERO);
        painter.line_segment([Pos2::new(rect.left(), o.y), Pos2::new(rect.right(), o.y)], Stroke::new(1.0, colors::AXIS_X.gamma_multiply(0.6)));
        painter.line_segment([Pos2::new(o.x, rect.top()), Pos2::new(o.x, rect.bottom())], Stroke::new(1.0, colors::AXIS_Y.gamma_multiply(0.6)));
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            tr!("siatka {} mm", fmt_step(step)),
            egui::FontId::proportional(11.0),
            Color32::from_gray(140),
        );
    }
}

/// Odcinki ścieżek wszystkich operacji dla karty graficznej — te same kolory i
/// szerokości co w rysowaniu przez procesor (`draw_toolpaths`).
fn toolpath_segments(doc: &Document, tool_diameters: &[f64]) -> Vec<crate::view2d_gpu::Segment> {
    use crate::view2d_gpu::{SegKind, Segment};
    let mut out = Vec::new();
    for (i, (op, cache)) in doc.ops.iter().zip(&doc.op_cache).enumerate() {
        let Some(cache) = cache else { continue };
        if !op.enabled || cache.display.is_empty() {
            continue;
        }
        let selected = doc.selected_op == Some(i);
        let base = op_color(i);
        // bez sumowania nakładających się przejść (mieszanie „max”) kolory są mocniejsze
        // niż przy rysowaniu przez procesor, żeby wyglądały podobnie
        let color = if selected || doc.selected_op.is_none() { base } else { base.gamma_multiply(0.55) };
        let rapid_color = Color32::from_rgb(220, 60, 60).gamma_multiply(if selected { 0.9 } else { 0.5 });
        let band = base.gamma_multiply(0.6);
        let d = tool_diameters.get(i).copied().unwrap_or(0.0) as f32;
        let top = op.cut.top_z - 1e-6;
        for w in cache.display.windows(2) {
            let ((a, _), (b, rapid)) = (w[0], w[1]);
            let (pa, pb) = (a.xy(), b.xy());
            if rapid {
                out.push(Segment::new(pa, pb, rapid_color, 1.0, SegKind::Dashed));
                continue;
            }
            if pa == pb {
                continue;
            }
            // pas o szerokości narzędzia — tylko dla zaznaczonej operacji, tylko w materiale
            if selected && d > 0.0 && b.z <= top {
                out.push(Segment::new(pa, pb, band, d, SegKind::Band));
            }
            out.push(Segment::new(pa, pb, color, 1.3, SegKind::Line));
        }
    }
    out
}

fn intersects(a: &Aabb2, b: &Aabb2) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.y <= b.max.y && a.max.y >= b.min.y
}

fn fmt_step(s: f64) -> String {
    let t = format!("{s:.4}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Kolor operacji `i` (paleta stała, dobrze widoczna na ciemnym tle).
pub fn op_color(i: usize) -> Color32 {
    const P: [Color32; 6] = [
        Color32::from_rgb(90, 220, 120),
        Color32::from_rgb(220, 110, 230),
        Color32::from_rgb(80, 210, 230),
        Color32::from_rgb(240, 160, 60),
        Color32::from_rgb(160, 150, 255),
        Color32::from_rgb(230, 230, 110),
    ];
    P[i % P.len()]
}

/// Symbol zera programu: koło z dwiema zamalowanymi ćwiartkami.
pub fn draw_zero_symbol(painter: &egui::Painter, c: Pos2, r: f32) {
    use std::f32::consts::FRAC_PI_2;
    painter.circle_filled(c, r + 1.5, Color32::from_rgb(17, 19, 22));
    painter.circle_filled(c, r, Color32::from_rgb(245, 245, 245));
    // ćwiartki I i III (w ekranie: prawa-górna i lewa-dolna)
    for q in [0, 2] {
        let a0 = -FRAC_PI_2 * (q as f32 + 1.0);
        let mut pts = vec![c];
        for i in 0..=8 {
            let a = a0 + FRAC_PI_2 * i as f32 / 8.0;
            pts.push(c + egui::vec2(a.cos(), a.sin()) * r);
        }
        painter.add(egui::Shape::convex_polygon(pts, Color32::from_rgb(17, 19, 22), Stroke::NONE));
    }
    painter.circle_stroke(c, r, Stroke::new(1.5, Color32::from_rgb(245, 245, 245)));
}

/// Strzałka osi z podpisem (kierunek w pikselach ekranu).
pub fn draw_axis_arrow(painter: &egui::Painter, o: Pos2, dir: egui::Vec2, len: f32, color: Color32, label: &str) {
    let dir = dir.normalized();
    let tip = o + dir * len;
    let n = egui::vec2(-dir.y, dir.x);
    painter.line_segment([o, tip - dir * 8.0], Stroke::new(2.5, color));
    painter.add(egui::Shape::convex_polygon(
        vec![tip, tip - dir * 11.0 + n * 5.5, tip - dir * 11.0 - n * 5.5],
        color,
        Stroke::NONE,
    ));
    painter.text(tip + dir * 9.0, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(13.0), color);
}
