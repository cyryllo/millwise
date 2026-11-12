//! Import DXF → kontury 2D (w milimetrach).
//!
//! Obsługiwane encje: LINE, ARC, CIRCLE, LWPOLYLINE, POLYLINE (2D), SPLINE,
//! ELLIPSE oraz INSERT (bloki, z transformacją). Wszystko jest rzutowane na
//! płaszczyznę XY i łączone w kontury osobno dla każdej warstwy.

use anyhow::{Context, Result};
use cam_geom::contour::arc_steps;
use cam_geom::{chain_segments, Contour, Vec2, Vertex};
use dxf::entities::{Entity, EntityType};
use dxf::enums::Units;
use dxf::Drawing;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::TAU;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Maksymalna szczelina przy łączeniu elementów [mm].
    pub chain_tolerance: f64,
    /// Błąd aproksymacji splajnów i elips [mm].
    pub curve_tolerance: f64,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self { chain_tolerance: 0.01, curve_tolerance: 0.01 }
    }
}

#[derive(Debug, Clone)]
pub struct ImportedContour {
    pub contour: Contour,
    pub layer: String,
}

#[derive(Debug, Clone, Default)]
pub struct DxfImport {
    pub contours: Vec<ImportedContour>,
    pub layers: Vec<String>,
    /// Mnożnik zastosowany do jednostek pliku (1.0 = mm).
    pub unit_scale: f64,
    pub unit_name: String,
    pub warnings: Vec<String>,
}

pub fn load_dxf(path: impl AsRef<Path>, opts: &ImportOptions) -> Result<DxfImport> {
    let path = path.as_ref();
    let drawing =
        Drawing::load_file(path).with_context(|| format!("Nie można wczytać pliku {}", path.display()))?;
    Ok(import_drawing(&drawing, opts))
}

pub fn import_drawing(drawing: &Drawing, opts: &ImportOptions) -> DxfImport {
    let (unit_scale, unit_name) = units_scale(drawing.header.default_drawing_units);
    let mut ctx = Ctx {
        blocks: drawing.blocks().map(|b| (b.name.clone(), b)).collect(),
        tol: opts.curve_tolerance / unit_scale,
        pieces: BTreeMap::new(),
        skipped: BTreeMap::new(),
    };
    let ident = Affine::scale(unit_scale);
    for e in drawing.entities() {
        if e.common.is_in_paper_space {
            continue;
        }
        ctx.entity(e, &ident, None, 0);
    }

    let mut out = DxfImport { unit_scale, unit_name: unit_name.to_string(), ..Default::default() };
    for (layer, pieces) in std::mem::take(&mut ctx.pieces) {
        for c in chain_segments(pieces, opts.chain_tolerance) {
            out.contours.push(ImportedContour { contour: c, layer: layer.clone() });
        }
        out.layers.push(layer);
    }
    for (kind, n) in ctx.skipped {
        out.warnings.push(format!("Pominięto {n} × {kind} (nieobsługiwany typ)"));
    }
    out
}

fn units_scale(u: Units) -> (f64, &'static str) {
    match u {
        Units::Inches => (25.4, "cale"),
        Units::Feet => (304.8, "stopy"),
        Units::Centimeters => (10.0, "cm"),
        Units::Meters => (1000.0, "m"),
        Units::Decimeters => (100.0, "dm"),
        Units::Microns => (0.001, "µm"),
        Units::Mils => (0.0254, "mils"),
        Units::Millimeters => (1.0, "mm"),
        _ => (1.0, "brak (przyjęto mm)"),
    }
}

/// Przekształcenie afiniczne 2D: p' = M·p + t.
#[derive(Debug, Clone, Copy)]
struct Affine {
    m: [[f64; 2]; 2],
    t: Vec2,
}

impl Affine {
    fn scale(s: f64) -> Self {
        Affine { m: [[s, 0.0], [0.0, s]], t: Vec2::ZERO }
    }
    fn apply(&self, p: Vec2) -> Vec2 {
        Vec2::new(self.m[0][0] * p.x + self.m[0][1] * p.y, self.m[1][0] * p.x + self.m[1][1] * p.y) + self.t
    }
    /// self ∘ o  (najpierw o, potem self)
    fn then_after(&self, o: &Affine) -> Affine {
        let a = &self.m;
        let b = &o.m;
        Affine {
            m: [
                [a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
                [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]],
            ],
            t: self.apply(o.t),
        }
    }
    fn det(&self) -> f64 {
        self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]
    }
    /// Czy przekształcenie zachowuje okręgi (skala jednorodna + obrót/odbicie).
    fn is_conformal(&self) -> bool {
        let c0 = Vec2::new(self.m[0][0], self.m[1][0]);
        let c1 = Vec2::new(self.m[0][1], self.m[1][1]);
        (c0.length() - c1.length()).abs() < 1e-9 * c0.length().max(1.0) && c0.dot(c1).abs() < 1e-9
    }
}

struct Ctx<'a> {
    blocks: HashMap<String, &'a dxf::Block>,
    /// tolerancja krzywych w jednostkach pliku
    tol: f64,
    pieces: BTreeMap<String, Vec<Contour>>,
    skipped: BTreeMap<&'static str, usize>,
}

impl<'a> Ctx<'a> {
    fn push(&mut self, layer: &str, mut c: Contour, xf: &Affine) {
        if c.verts.len() < 2 {
            return;
        }
        if !xf.is_conformal() && c.verts.iter().any(|v| v.bulge != 0.0) {
            let pts = c.flatten(self.tol);
            c = polyline(&pts, false);
        }
        let flip = xf.det() < 0.0;
        for v in &mut c.verts {
            v.p = xf.apply(v.p);
            if flip {
                v.bulge = -v.bulge;
            }
        }
        self.pieces.entry(layer.to_string()).or_default().push(c);
    }

    fn entity(&mut self, e: &Entity, xf: &Affine, parent_layer: Option<&str>, depth: u32) {
        // encje na warstwie "0" w bloku dziedziczą warstwę INSERT-a
        let layer = match parent_layer {
            Some(pl) if e.common.layer == "0" => pl,
            _ => e.common.layer.as_str(),
        };
        let layer = if layer.is_empty() { "0" } else { layer };
        match &e.specific {
            EntityType::Line(l) => {
                let c = Contour {
                    verts: vec![Vertex::new(l.p1.x, l.p1.y, 0.0), Vertex::new(l.p2.x, l.p2.y, 0.0)],
                    closed: false,
                };
                self.push(layer, c, xf);
            }
            EntityType::Circle(c) => {
                let ocs = ocs(c.normal.z);
                self.push(layer, Contour::circle(Vec2::new(c.center.x, c.center.y), c.radius), &xf.then_after(&ocs));
            }
            EntityType::Arc(a) => {
                let ocs = ocs(a.normal.z);
                let c = arc_contour(Vec2::new(a.center.x, a.center.y), a.radius, a.start_angle, a.end_angle);
                self.push(layer, c, &xf.then_after(&ocs));
            }
            EntityType::LwPolyline(p) => {
                let c = Contour {
                    verts: p.vertices.iter().map(|v| Vertex::new(v.x, v.y, v.bulge)).collect(),
                    closed: p.is_closed(),
                };
                let ocs = ocs(p.extrusion_direction.z);
                self.push(layer, fix_open_bulge(c), &xf.then_after(&ocs));
            }
            EntityType::Polyline(p) => {
                // 3D polyline (8), siatki (16, 64) — pomijamy
                if p.flags & (8 | 16 | 64) != 0 {
                    *self.skipped.entry("POLYLINE 3D/siatka").or_default() += 1;
                    return;
                }
                let c = Contour {
                    verts: p.vertices().map(|v| Vertex::new(v.location.x, v.location.y, v.bulge)).collect(),
                    closed: p.flags & 1 != 0,
                };
                let ocs = ocs(p.normal.z);
                self.push(layer, fix_open_bulge(c), &xf.then_after(&ocs));
            }
            EntityType::Spline(s) => {
                let pts = flatten_spline(s, self.tol);
                let closed = s.flags & 1 != 0
                    || (pts.len() > 2 && pts[0].distance(*pts.last().unwrap()) < self.tol * 0.1);
                let mut pts = pts;
                if closed && pts.len() > 2 && pts[0].distance(*pts.last().unwrap()) < self.tol {
                    pts.pop();
                }
                self.push(layer, polyline(&pts, closed), xf);
            }
            EntityType::Ellipse(el) => {
                let (pts, closed) = flatten_ellipse(el, self.tol);
                let ocs = ocs(el.normal.z);
                self.push(layer, polyline(&pts, closed), &xf.then_after(&ocs));
            }
            EntityType::Insert(ins) => {
                if depth > 16 {
                    return;
                }
                let Some(block) = self.blocks.get(ins.name.as_str()).copied() else {
                    *self.skipped.entry("INSERT bez bloku").or_default() += 1;
                    return;
                };
                let rot = ins.rotation.to_radians();
                let (s, c) = rot.sin_cos();
                let base = Vec2::new(block.base_point.x, block.base_point.y);
                let cols = ins.column_count.max(1);
                let rows = ins.row_count.max(1);
                for r in 0..rows {
                    for col in 0..cols {
                        let off = Vec2::new(col as f64 * ins.column_spacing, r as f64 * ins.row_spacing);
                        let off = Vec2::new(c * off.x - s * off.y, s * off.x + c * off.y);
                        let local = Affine {
                            m: [
                                [c * ins.x_scale_factor, -s * ins.y_scale_factor],
                                [s * ins.x_scale_factor, c * ins.y_scale_factor],
                            ],
                            t: Vec2::ZERO,
                        };
                        let mut local = local;
                        local.t = Vec2::new(ins.location.x, ins.location.y) + off - local.apply(base);
                        let ocs = ocs(ins.extrusion_direction.z);
                        let total = xf.then_after(&ocs.then_after(&local));
                        for be in &block.entities {
                            self.entity(be, &total, Some(layer), depth + 1);
                        }
                    }
                }
            }
            // elementy opisowe — ignorowane bez ostrzeżenia
            EntityType::ModelPoint(_)
            | EntityType::Text(_)
            | EntityType::MText(_)
            | EntityType::RText(_)
            | EntityType::ArcAlignedText(_)
            | EntityType::Attribute(_)
            | EntityType::AttributeDefinition(_)
            | EntityType::RotatedDimension(_)
            | EntityType::RadialDimension(_)
            | EntityType::DiameterDimension(_)
            | EntityType::AngularThreePointDimension(_)
            | EntityType::OrdinateDimension(_)
            | EntityType::Leader(_)
            | EntityType::Tolerance(_)
            | EntityType::Seqend(_)
            | EntityType::Vertex(_)
            | EntityType::XLine(_)
            | EntityType::Ray(_) => {}
            EntityType::Solid(_) => *self.skipped.entry("SOLID").or_default() += 1,
            EntityType::Face3D(_) => *self.skipped.entry("3DFACE").or_default() += 1,
            EntityType::Image(_) => *self.skipped.entry("IMAGE").or_default() += 1,
            _ => *self.skipped.entry("inne").or_default() += 1,
        }
    }
}

/// Układ OCS: dla wektora normalnego (0,0,-1) oś X jest odbita.
fn ocs(normal_z: f64) -> Affine {
    if normal_z < 0.0 { Affine { m: [[-1.0, 0.0], [0.0, 1.0]], t: Vec2::ZERO } } else { Affine::scale(1.0) }
}

/// W otwartej polilinii ostatni wierzchołek nie rozpoczyna segmentu.
fn fix_open_bulge(mut c: Contour) -> Contour {
    if !c.closed {
        if let Some(v) = c.verts.last_mut() {
            v.bulge = 0.0;
        }
    }
    c
}

fn polyline(pts: &[Vec2], closed: bool) -> Contour {
    Contour { verts: pts.iter().map(|p| Vertex { p: *p, bulge: 0.0 }).collect(), closed }
}

/// Łuk DXF (kąty w stopniach, zawsze CCW) jako kontur z łukami ≤ 180°.
fn arc_contour(center: Vec2, r: f64, start_deg: f64, end_deg: f64) -> Contour {
    let a0 = start_deg.to_radians();
    let mut sweep = end_deg.to_radians() - a0;
    while sweep <= 0.0 {
        sweep += TAU;
    }
    let parts = (sweep / std::f64::consts::PI).ceil().max(1.0) as usize;
    let step = sweep / parts as f64;
    let bulge = (step / 4.0).tan();
    let mut c = Contour::new(false);
    for i in 0..=parts {
        let p = center + Vec2::from_angle(a0 + step * i as f64) * r;
        c.verts.push(Vertex { p, bulge: if i < parts { bulge } else { 0.0 } });
    }
    c
}

fn flatten_ellipse(el: &dxf::entities::Ellipse, tol: f64) -> (Vec<Vec2>, bool) {
    let c = Vec2::new(el.center.x, el.center.y);
    let maj = Vec2::new(el.major_axis.x, el.major_axis.y);
    let min = maj.perp() * el.minor_axis_ratio;
    let t0 = el.start_parameter;
    let mut sweep = el.end_parameter - t0;
    while sweep <= 0.0 {
        sweep += TAU;
    }
    let closed = (sweep - TAU).abs() < 1e-6;
    let n = arc_steps(maj.length(), sweep, tol).max(8);
    let count = if closed { n } else { n + 1 };
    let pts = (0..count)
        .map(|i| {
            let t = t0 + sweep * i as f64 / n as f64;
            c + maj * t.cos() + min * t.sin()
        })
        .collect();
    (pts, closed)
}

fn flatten_spline(s: &dxf::entities::Spline, tol: f64) -> Vec<Vec2> {
    let cps: Vec<Vec2> = s.control_points.iter().map(|p| Vec2::new(p.x, p.y)).collect();
    let p = s.degree_of_curve.max(1) as usize;
    let knots = &s.knot_values;
    if cps.len() <= p || knots.len() != cps.len() + p + 1 {
        // brak poprawnej definicji NURBS — użyj punktów dopasowania
        return s.fit_points.iter().map(|p| Vec2::new(p.x, p.y)).collect();
    }
    let w: Vec<f64> =
        if s.weight_values.len() == cps.len() { s.weight_values.clone() } else { vec![1.0; cps.len()] };
    let u0 = knots[p];
    let u1 = knots[cps.len()];
    // liczba próbek z długości łamanej kontrolnej
    let poly_len: f64 = cps.windows(2).map(|w| w[0].distance(w[1])).sum();
    let n = ((poly_len / tol.max(1e-6)).sqrt() as usize * 2).clamp(cps.len() * 4, 4000);
    (0..=n).map(|i| de_boor(u0 + (u1 - u0) * i as f64 / n as f64, p, knots, &cps, &w)).collect()
}

/// Wartość NURBS w punkcie `u` (algorytm de Boora we współrzędnych jednorodnych).
fn de_boor(u: f64, p: usize, knots: &[f64], cps: &[Vec2], w: &[f64]) -> Vec2 {
    let n = cps.len();
    let mut k = p;
    while k < n - 1 && u >= knots[k + 1] {
        k += 1;
    }
    let mut d: Vec<(f64, f64, f64)> =
        (0..=p).map(|j| { let i = k - p + j; (cps[i].x * w[i], cps[i].y * w[i], w[i]) }).collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k - p + j;
            let denom = knots[i + p + 1 - r] - knots[i];
            let a = if denom.abs() < 1e-12 { 0.0 } else { (u - knots[i]) / denom };
            d[j] = (
                (1.0 - a) * d[j - 1].0 + a * d[j].0,
                (1.0 - a) * d[j - 1].1 + a * d[j].1,
                (1.0 - a) * d[j - 1].2 + a * d[j].2,
            );
        }
    }
    let (x, y, ww) = d[p];
    Vec2::new(x / ww, y / ww)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dxf::entities::*;
    use dxf::Point;

    fn ent(t: EntityType, layer: &str) -> Entity {
        let mut e = Entity::new(t);
        e.common.layer = layer.to_string();
        e
    }

    #[test]
    fn rounded_rect_from_lines_and_arcs() {
        let mut d = Drawing::new();
        d.header.default_drawing_units = Units::Millimeters;
        // prostokąt 40x20 z zaokrągleniami R5, rozbity na LINE + ARC
        let r = 5.0;
        let (w, h) = (40.0, 20.0);
        d.add_entity(ent(EntityType::Line(Line::new(Point::new(r, 0.0, 0.0), Point::new(w - r, 0.0, 0.0))), "kontur"));
        d.add_entity(ent(EntityType::Arc(Arc::new(Point::new(w - r, r, 0.0), r, 270.0, 0.0)), "kontur"));
        d.add_entity(ent(EntityType::Line(Line::new(Point::new(w, r, 0.0), Point::new(w, h - r, 0.0))), "kontur"));
        d.add_entity(ent(EntityType::Arc(Arc::new(Point::new(w - r, h - r, 0.0), r, 0.0, 90.0)), "kontur"));
        d.add_entity(ent(EntityType::Line(Line::new(Point::new(w - r, h, 0.0), Point::new(r, h, 0.0))), "kontur"));
        d.add_entity(ent(EntityType::Arc(Arc::new(Point::new(r, h - r, 0.0), r, 90.0, 180.0)), "kontur"));
        d.add_entity(ent(EntityType::Line(Line::new(Point::new(0.0, h - r, 0.0), Point::new(0.0, r, 0.0))), "kontur"));
        d.add_entity(ent(EntityType::Arc(Arc::new(Point::new(r, r, 0.0), r, 180.0, 270.0)), "kontur"));
        d.add_entity(ent(EntityType::Circle(Circle::new(Point::new(20.0, 10.0, 0.0), 3.0)), "otwory"));

        let imp = import_drawing(&d, &ImportOptions::default());
        assert_eq!(imp.layers, vec!["kontur".to_string(), "otwory".to_string()]);
        assert_eq!(imp.contours.len(), 2);
        let rect = &imp.contours[0].contour;
        assert!(rect.closed);
        let expected = w * h - (4.0 - std::f64::consts::PI) * r * r;
        assert!((rect.signed_area().abs() - expected).abs() < 1e-6, "{}", rect.signed_area());
    }

    #[test]
    fn inch_units_are_scaled() {
        let mut d = Drawing::new();
        d.header.default_drawing_units = Units::Inches;
        d.add_entity(ent(EntityType::Circle(Circle::new(Point::new(0.0, 0.0, 0.0), 1.0)), "0"));
        let imp = import_drawing(&d, &ImportOptions::default());
        let bb = imp.contours[0].contour.bbox();
        assert!((bb.size().x - 50.8).abs() < 1e-9);
    }

    #[test]
    fn spline_endpoints() {
        // kwadratowa B-spline przez (0,0) i (10,0) z punktem kontrolnym (5,10)
        let cps = [Vec2::new(0.0, 0.0), Vec2::new(5.0, 10.0), Vec2::new(10.0, 0.0)];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let w = [1.0; 3];
        assert!(de_boor(0.0, 2, &knots, &cps, &w).distance(cps[0]) < 1e-12);
        assert!(de_boor(1.0, 2, &knots, &cps, &w).distance(cps[2]) < 1e-12);
        assert!((de_boor(0.5, 2, &knots, &cps, &w).y - 5.0).abs() < 1e-12);
    }
}
