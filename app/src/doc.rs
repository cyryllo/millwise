//! Model dokumentu: geometria, warstwy, zaznaczenie, półfabrykat, operacje.

use cam_i18n::tr;
use cam_geom::{Aabb2, Contour, Vec2};
use cam_io::{DxfImport, ImportOptions};
use cam_ops::{OpResult, Operation, ToolLibrary, P3};
use crate::history::{Badge, Mark};
use crate::icons::Icon;
use cam_post::{Machine, PostSettings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct DocContour {
    pub contour: Contour,
    pub layer: usize,
    /// Element (plik), z którego pochodzi krzywa — indeks w `Document::sources`.
    pub source: usize,
    /// Kontur spłaszczony do łamanej — do rysowania i zaznaczania.
    pub flat: Vec<Vec2>,
    pub bbox: Aabb2,
    pub length: f64,
    pub area: f64,
}

impl DocContour {
    pub fn new(contour: Contour, layer: usize) -> Self {
        DocContour {
            flat: contour.flatten(0.005),
            bbox: contour.bbox(),
            length: contour.length(),
            area: contour.signed_area().abs(),
            contour,
            layer,
            source: 0,
        }
    }

    fn with_source(mut self, source: usize) -> Self {
        self.source = source;
        self
    }
}

/// Liczba zmian narzędzia między kolejnymi włączonymi operacjami.
pub fn count_tool_changes<'a>(ops: impl Iterator<Item = &'a Operation>) -> usize {
    let tools: Vec<u32> = ops.filter(|o| o.enabled).map(|o| o.tool_id).collect();
    tools.windows(2).filter(|w| w[0] != w[1]).count()
}

/// Kolejność „według narzędzi”: operacje tym samym frezem po sobie, narzędzia
/// w kolejności pierwszego użycia; profile zewnętrzne (wycinanie detalu) na
/// końcu, żeby detal nie odpadł przed resztą obróbki. Kolejność wewnątrz grup bez zmian.
pub fn grouped_order(ops: &[Operation]) -> Vec<usize> {
    let cutout = |o: &Operation| matches!(&o.kind, cam_ops::OpKind::Profile(p) if p.side == cam_ops::ProfileSide::Outside);
    let mut first_use: Vec<u32> = Vec::new();
    for o in ops {
        if !first_use.contains(&o.tool_id) {
            first_use.push(o.tool_id);
        }
    }
    let mut order: Vec<usize> = (0..ops.len()).collect();
    order.sort_by_key(|&i| {
        let o = &ops[i];
        (cutout(o), first_use.iter().position(|&t| t == o.tool_id), i)
    });
    order
}

/// Zmiana z parametrami zapamiętana w historii, żeby można ją było później
/// przebudować z innymi wartościami (klik w historii).
#[derive(Clone, Debug, PartialEq)]
pub enum ParamEdit {
    /// Szyk; odstępy między krawędziami kopii [mm].
    Array { selection: Vec<usize>, cols: usize, rows: usize, gap_x: f64, gap_y: f64 },
    /// Skalowanie wokół środka zaznaczenia [%].
    Scale { selection: Vec<usize>, pct: f64 },
    /// Obrót wokół środka zaznaczenia [°].
    Rotate { selection: Vec<usize>, deg: f64 },
}

impl ParamEdit {
    /// Krzywe, na których zrobiono zmianę (numery w stanie sprzed niej).
    pub fn selection(&self) -> &[usize] {
        match self {
            ParamEdit::Array { selection, .. } | ParamEdit::Scale { selection, .. } | ParamEdit::Rotate { selection, .. } => selection,
        }
    }
}

/// Krzywa w schowku: kontur, nazwa warstwy i nazwa elementu (pliku).
#[derive(Clone)]
pub struct ClipItem {
    pub contour: Contour,
    pub layer: String,
    pub source: String,
}

pub struct Layer {
    pub name: String,
    pub visible: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stock {
    /// Grubość materiału [mm]. Góra półfabrykatu to Z=0.
    pub thickness: f64,
    /// Naddatek wokół geometrii [mm].
    pub margin: f64,
    /// Materiał — wybiera zestaw parametrów skrawania narzędzi.
    pub material: String,
    /// Obróbka dwustronna (None = jednostronna).
    pub two_sided: Option<TwoSided>,
    /// Model 3D (STL) w półfabrykacie.
    pub model: Option<crate::model3d::Model3D>,
}

impl Default for Stock {
    fn default() -> Self {
        Self { thickness: 18.0, margin: 5.0, material: "Drewno".into(), two_sided: None, model: None }
    }
}

/// Oś, wokół której odwraca się płytę.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlipAxis {
    /// Wokół osi X: przód przechodzi na tył (odbicie Y).
    X,
    /// Wokół osi Y: lewa krawędź przechodzi na prawą (odbicie X).
    Y,
}

/// Obróbka dwustronna: oś odwracania i otwory na kołki ustalające.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TwoSided {
    pub axis: FlipAxis,
    /// Położenie osi: X linii pionowej (oś Y) albo Y linii poziomej (oś X) [mm].
    pub pos: f64,
    /// Średnica kołków [mm].
    pub pin_diameter: f64,
    /// Ile otwory wchodzą w stół ofiarny [mm].
    pub pin_extra: f64,
    /// Odstęp otworu od detalu [mm].
    pub pin_gap: f64,
}

impl Default for TwoSided {
    fn default() -> Self {
        TwoSided { axis: FlipAxis::Y, pos: 0.0, pin_diameter: 6.0, pin_extra: 5.0, pin_gap: 8.0 }
    }
}

impl TwoSided {
    /// Odbicie względem osi odwracania (geometria strony B).
    pub fn mirror(&self, c: &mut Contour) {
        match self.axis {
            FlipAxis::Y => c.mirror_x(self.pos),
            FlipAxis::X => c.mirror_y(self.pos),
        }
    }

    pub fn mirror_point(&self, p: P3) -> P3 {
        match self.axis {
            FlipAxis::Y => P3 { x: 2.0 * self.pos - p.x, ..p },
            FlipAxis::X => P3 { y: 2.0 * self.pos - p.y, ..p },
        }
    }
}

/// Sposób wyrównania zaznaczonych krzywych.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

/// Wynik generowania operacji + dane do wyświetlania.
pub struct OpCache {
    pub result: OpResult,
    /// Spłaszczona ścieżka: (punkt, czy ruch szybki).
    pub display: Vec<(P3, bool)>,
    pub minutes: f64,
}


#[derive(Default)]
pub struct Document {
    pub path: Option<PathBuf>,
    /// Plik projektu (.millwise), jeśli zapisany/wczytany.
    pub project_path: Option<PathBuf>,
    pub contours: Vec<DocContour>,
    pub layers: Vec<Layer>,
    /// Elementy projektu: nazwy wczytanych i dodanych plików.
    pub sources: Vec<String>,
    pub selected: Vec<bool>,
    pub bbox: Option<Aabb2>,
    pub stock: Stock,
    pub unit_name: String,
    pub warnings: Vec<String>,
    /// Zwiększany przy każdej zmianie wpływającej na widoki.
    pub revision: u64,

    pub ops: Vec<Operation>,
    pub op_cache: Vec<Option<OpCache>>,
    pub selected_op: Option<usize>,
    pub ops_dirty: bool,
    pub post: PostSettings,
    /// Zwiększany przy każdym przeliczeniu ścieżek (symulacja startuje od nowa).
    pub paths_revision: u64,
    /// Zwiększany przy każdej zmianie krzywych (historia zmian porównuje go zamiast krzywych).
    pub geom_revision: u64,
    /// Opis ostatniej zmiany dla historii (np. „Obrót 90°”) z ikoną; bez opisu historia nazwie zmianę sama.
    pub edit_label: Option<(Mark, String)>,
    /// Parametry zmiany zrobionej teraz (szyk, skala, obrót) — trafiają do historii.
    pub step_param: Option<ParamEdit>,
}

impl Document {
    pub fn load_dxf(path: &Path) -> anyhow::Result<Self> {
        let imp = cam_io::load_dxf(path, &ImportOptions::default())?;
        Ok(Self::from_import(imp, Some(path.to_path_buf())))
    }

    pub fn from_import(imp: DxfImport, path: Option<PathBuf>) -> Self {
        let layers: Vec<Layer> = imp.layers.iter().map(|n| Layer { name: n.clone(), visible: true }).collect();
        let contours: Vec<DocContour> = imp
            .contours
            .into_iter()
            .map(|ic| {
                let layer = layers.iter().position(|l| l.name == ic.layer).unwrap_or(0);
                DocContour::new(ic.contour, layer)
            })
            .collect();
        let mut d = Document {
            path,
            contours,
            layers,
            unit_name: imp.unit_name,
            warnings: imp.warnings,
            ..Default::default()
        };
        d.finish_load();
        d
    }

    /// Uzupełnia pola pochodne po wczytaniu geometrii.
    pub fn finish_load(&mut self) {
        if self.sources.is_empty() {
            let name = self.project_path.as_ref().or(self.path.as_ref()).and_then(|p| p.file_name());
            self.sources.push(name.map_or_else(|| "Rysunek".into(), |n| n.to_string_lossy().into_owned()));
        }
        let last = self.sources.len() - 1;
        for c in &mut self.contours {
            c.source = c.source.min(last);
        }
        self.recompute_bbox();
        self.selected = vec![false; self.contours.len()];
        self.op_cache = self.ops.iter().map(|_| None).collect();
        self.ops_dirty = true;
        self.revision = 1;
    }

    /// Obrys rysunku i modelu 3D (z góry).
    pub fn recompute_bbox(&mut self) {
        let model = self.stock.model.as_ref().map(|m| m.footprint());
        self.bbox = self.contours.iter().map(|c| c.bbox).chain(model).reduce(|a, b| a.union(&b));
    }

    /// Wstawia model 3D (zastępuje poprzedni): obok rysunku albo w zerze,
    /// grubość półfabrykatu według wysokości modelu.
    pub fn set_model(&mut self, mut m: crate::model3d::Model3D) {
        let name = m.name.clone();
        self.stock.model = None;
        self.recompute_bbox();
        if let Some(b) = self.bbox {
            m.pos = [b.max.x + 10.0, b.min.y];
        }
        m.refresh();
        self.stock.thickness = m.stock_thickness();
        self.stock.model = Some(m);
        if !self.sources.contains(&name) {
            self.sources.push(name.clone());
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::Stock).badge(Badge::Add), tr!("Model 3D: {}", name));
    }

    pub fn file_name(&self) -> String {
        self.project_path
            .as_ref()
            .or(self.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr!("bez nazwy").into())
    }

    pub fn is_visible(&self, i: usize) -> bool {
        self.layers.get(self.contours[i].layer).is_none_or(|l| l.visible)
    }

    pub fn stock_box(&self) -> Option<Aabb2> {
        self.bbox.map(|b| b.expanded(self.stock.margin))
    }

    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|s| **s).count()
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        (0..self.selected.len()).filter(|&i| self.selected[i]).collect()
    }

    pub fn clear_selection(&mut self) {
        if self.selected.iter().any(|s| *s) {
            self.selected.iter_mut().for_each(|s| *s = false);
            self.revision += 1;
        }
    }

    pub fn set_selected(&mut self, i: usize, v: bool) {
        if self.selected[i] != v {
            self.selected[i] = v;
            self.revision += 1;
        }
    }

    /// Najbliższy widoczny kontur w promieniu `max_dist` (w mm).
    pub fn pick(&self, p: Vec2, max_dist: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, c) in self.contours.iter().enumerate() {
            if !self.is_visible(i) || !c.bbox.expanded(max_dist).contains_point(p) {
                continue;
            }
            let d = c.contour.distance_to(p);
            if d <= max_dist && best.is_none_or(|b| d < b.1) {
                best = Some((i, d));
            }
        }
        best.map(|b| b.0)
    }

    /// Kontury w prostokącie. `crossing` = wystarczy, że kontur przecina prostokąt.
    pub fn in_box(&self, r: Aabb2, crossing: bool) -> Vec<usize> {
        (0..self.contours.len())
            .filter(|&i| self.is_visible(i))
            .filter(|&i| {
                let c = &self.contours[i];
                if r.contains_box(&c.bbox) {
                    return true;
                }
                crossing && c.flat.iter().any(|p| r.contains_point(*p))
            })
            .collect()
    }

    // ---------- operacje ----------

    /// Czy krzywa `i` pasuje do typu operacji (kieszeń — zamknięte, wiercenie — okręgi).
    pub fn contour_fits(&self, kind: &cam_ops::OpKind, i: usize) -> bool {
        let c = &self.contours[i].contour;
        match kind {
            cam_ops::OpKind::Pocket(_) | cam_ops::OpKind::VCarve(_) => c.closed,
            cam_ops::OpKind::Drill(_) => c.as_circle().is_some(),
            _ => true,
        }
    }

    /// Dodaje lub usuwa krzywą z operacji. Zwraca komunikat, gdy krzywa nie pasuje.
    pub fn toggle_op_contour(&mut self, op: usize, i: usize) -> Result<(), &'static str> {
        if !self.contour_fits(&self.ops[op].kind, i) {
            return Err(match self.ops[op].kind {
                cam_ops::OpKind::Pocket(_) => tr!("Kieszeń wymaga krzywej zamkniętej"),
                cam_ops::OpKind::VCarve(_) => tr!("V-carving wymaga krzywej zamkniętej"),
                cam_ops::OpKind::Drill(_) => tr!("Wiercenie wymaga okręgu"),
                _ => tr!("Krzywa nie pasuje do operacji"),
            });
        }
        let list = &mut self.ops[op].contours;
        match list.iter().position(|&k| k == i) {
            Some(pos) => {
                list.remove(pos);
            }
            None => list.push(i),
        }
        self.ops_dirty = true;
        Ok(())
    }

    /// Dodaje pasujące krzywe do operacji (bez duplikatów).
    pub fn add_op_contours(&mut self, op: usize, idx: &[usize]) {
        for &i in idx {
            if self.contour_fits(&self.ops[op].kind, i) && !self.ops[op].contours.contains(&i) {
                self.ops[op].contours.push(i);
                self.ops_dirty = true;
            }
        }
    }

    pub fn add_op(&mut self, op: Operation) -> usize {
        self.ops.push(op);
        self.op_cache.push(None);
        self.ops_dirty = true;
        let i = self.ops.len() - 1;
        self.selected_op = Some(i);
        i
    }

    pub fn remove_op(&mut self, i: usize) {
        self.ops.remove(i);
        self.op_cache.remove(i);
        self.selected_op = match self.selected_op {
            Some(s) if s == i => None,
            Some(s) if s > i => Some(s - 1),
            s => s,
        };
        self.ops_dirty = true;
    }

    pub fn move_op(&mut self, i: usize, up: bool) {
        let j = if up { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < self.ops.len()) };
        if let Some(j) = j {
            self.move_op_to(i, j);
        }
    }

    /// Środki otworów na kołki: na osi odwracania, po obu stronach detalu
    /// (poza warstwą KOŁKI), w odstępie `pin_gap` od niego.
    pub fn pin_centers(&self) -> Option<[Vec2; 2]> {
        let t = self.stock.two_sided.as_ref()?;
        let part = (0..self.contours.len())
            .filter(|&i| self.layers.get(self.contours[i].layer).is_none_or(|l| l.name != "KOŁKI"))
            .map(|i| self.contours[i].bbox)
            .reduce(|a, b| a.union(&b))?;
        let r = t.pin_diameter * 0.5;
        Some(match t.axis {
            FlipAxis::Y => [Vec2::new(t.pos, part.min.y - t.pin_gap - r), Vec2::new(t.pos, part.max.y + t.pin_gap + r)],
            FlipAxis::X => [Vec2::new(part.min.x - t.pin_gap - r, t.pos), Vec2::new(part.max.x + t.pin_gap + r, t.pos)],
        })
    }

    /// Liczba zmian narzędzia między kolejnymi włączonymi operacjami.
    pub fn tool_changes(&self) -> usize {
        Self::count_changes(self.ops.iter())
    }

    fn count_changes<'a>(ops: impl Iterator<Item = &'a Operation>) -> usize {
        count_tool_changes(ops)
    }

    fn grouped_order(&self) -> Vec<usize> {
        grouped_order(&self.ops)
    }

    pub fn tool_changes_if_grouped(&self) -> usize {
        Self::count_changes(self.grouped_order().into_iter().map(|i| &self.ops[i]))
    }

    /// Ustawia operacje według narzędzi (jeden krok w historii).
    pub fn group_by_tool(&mut self) {
        let order = self.grouped_order();
        let before = self.tool_changes();
        let ops: Vec<Operation> = order.iter().map(|&i| self.ops[i].clone()).collect();
        let cache: Vec<Option<OpCache>> = order.iter().map(|&i| self.op_cache[i].take()).collect();
        self.selected_op = self.selected_op.and_then(|s| order.iter().position(|&x| x == s));
        self.ops = ops;
        self.op_cache = cache;
        self.ops_dirty = true;
        self.revision += 1;
        let after = self.tool_changes();
        self.label(Mark::new(Icon::Document).badge(Badge::Order), tr!("Kolejność według narzędzi (zmian narzędzia: {}, teraz {})", before, after));
    }

    /// Zmiana kolejności obróbki: operacja `from` trafia na miejsce `to` (numer na liście po zmianie).
    pub fn move_op_to(&mut self, from: usize, to: usize) {
        let n = self.ops.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let mut order: Vec<usize> = (0..n).collect();
        let k = order.remove(from);
        order.insert(to, k);
        let op = self.ops.remove(from);
        let cache = self.op_cache.remove(from);
        let name = op.name.clone();
        self.ops.insert(to, op);
        self.op_cache.insert(to, cache);
        self.selected_op = self.selected_op.and_then(|s| order.iter().position(|&x| x == s));
        self.ops_dirty = true;
        self.revision += 1;
        let mark = Mark::op(to, &self.ops[to], Badge::Order);
        self.label(mark, tr!("Kolejność obróbki: {} na {}. miejsce", name, to + 1));
    }

    /// Przelicza ścieżki wszystkich operacji.
    pub fn regenerate(&mut self, lib: &ToolLibrary, machine: Option<&Machine>) {
        self.op_cache = self
            .ops
            .iter()
            .map(|op| {
                let tool = lib.get(op.tool_id)?;
                let mut geom: Vec<Contour> =
                    op.contours.iter().filter_map(|&i| self.contours.get(i)).map(|c| c.contour.clone()).collect();
                // strona B: geometria odbita względem osi odwracania (G-code dla odwróconej płyty)
                let flip = self.stock.two_sided.as_ref().filter(|_| op.side == cam_ops::Side::B);
                if let Some(t) = flip {
                    geom.iter_mut().for_each(|c| t.mirror(c));
                }
                let mut result = cam_ops::generate(op, tool, &geom);
                if op.side == cam_ops::Side::B && self.stock.two_sided.is_none() {
                    result.warnings.push(tr!("Strona B bez włączonej obróbki dwustronnej — liczona jak strona A").into());
                }
                if geom.len() < op.contours.len() {
                    result.warnings.push(tr!("Część konturów operacji nie istnieje").into());
                }
                if let Some(m) = machine {
                    check_machine(op, &result.toolpath, m, &mut result.warnings);
                }
                // na rysunku (widok strony A) ścieżka strony B odbita z powrotem
                let display = match flip {
                    Some(t) => result.toolpath.polyline(0.01).into_iter().map(|(p, r)| (t.mirror_point(p), r)).collect(),
                    None => result.toolpath.polyline(0.01),
                };
                let rapid = machine.map_or(3000.0, |m| m.rapid_xy);
                let minutes = result.toolpath.estimate_minutes(rapid);
                Some(OpCache { result, display, minutes })
            })
            .collect();
        self.ops_dirty = false;
        self.revision += 1;
        self.paths_revision += 1;
    }

    // ---------- edycja geometrii ----------

    pub fn selection_bbox(&self) -> Option<Aabb2> {
        self.selected_indices().into_iter().map(|i| self.contours[i].bbox).reduce(|a, b| a.union(&b))
    }

    fn rebuild(&mut self, i: usize) {
        let c = &self.contours[i];
        self.contours[i] = DocContour::new(c.contour.clone(), c.layer).with_source(c.source);
    }

    fn geometry_changed(&mut self) {
        self.recompute_bbox();
        self.ops_dirty = true;
        self.revision += 1;
        self.geom_revision += 1;
    }

    pub fn label(&mut self, mark: Mark, text: impl Into<String>) {
        self.edit_label = Some((mark, text.into()));
    }

    /// Usuwa zaznaczone krzywe; operacje tracą odwołania do nich. Zwraca liczbę usuniętych.
    pub fn delete_selected(&mut self) -> usize {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return 0;
        }
        // nowy numer każdej pozostającej krzywej
        let mut map = vec![None; self.contours.len()];
        let mut n = 0;
        for (i, m) in map.iter_mut().enumerate() {
            if !self.selected[i] {
                *m = Some(n);
                n += 1;
            }
        }
        let mut k = 0;
        self.contours.retain(|_| {
            k += 1;
            map[k - 1].is_some()
        });
        self.selected = vec![false; self.contours.len()];
        for op in &mut self.ops {
            op.contours = op.contours.iter().filter_map(|&i| map.get(i).copied().flatten()).collect();
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::Delete), tr!("Usunięto krzywe ({})", sel.len()));
        sel.len()
    }

    /// Dodaje krzywe (np. ze schowka) przesunięte o `d`; zaznaczone zostają tylko nowe.
    pub fn add_contours(&mut self, items: &[ClipItem], d: Vec2) {
        if items.is_empty() {
            return;
        }
        self.selected.iter_mut().for_each(|s| *s = false);
        for it in items {
            let mut c = it.contour.clone();
            c.translate(d);
            let layer = match self.layers.iter().position(|l| l.name == it.layer) {
                Some(i) => i,
                None if !it.layer.is_empty() => {
                    self.layers.push(Layer { name: it.layer.clone(), visible: true });
                    self.layers.len() - 1
                }
                None => 0,
            };
            let source = self.source_index(&it.source);
            self.contours.push(DocContour::new(c, layer).with_source(source));
            self.selected.push(true);
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::Paste).badge(Badge::Add), tr!("Wklejono krzywe ({})", items.len()));
    }

    /// Kopia zaznaczonych krzywych do schowka: kontur i nazwa warstwy.
    pub fn copy_selected(&self) -> Vec<ClipItem> {
        self.selected_indices()
            .into_iter()
            .map(|i| {
                let c = &self.contours[i];
                ClipItem {
                    contour: c.contour.clone(),
                    layer: self.layers.get(c.layer).map(|l| l.name.clone()).unwrap_or_default(),
                    source: self.sources.get(c.source).cloned().unwrap_or_default(),
                }
            })
            .collect()
    }

    /// Numer elementu o tej nazwie; dodaje go, jeśli go nie ma.
    fn source_index(&mut self, name: &str) -> usize {
        if let Some(i) = self.sources.iter().position(|s| s == name) {
            return i;
        }
        if self.sources.is_empty() && name.is_empty() {
            self.sources.push("Rysunek".into());
            return 0;
        }
        self.sources.push(if name.is_empty() { "Rysunek".into() } else { name.to_string() });
        self.sources.len() - 1
    }

    /// Odstęp między elementami przy układaniu dodanych plików [mm].
    const ELEMENT_GAP: f64 = 10.0;

    /// Miejsce (lewy dolny róg) na nowy element o rozmiarze `size`: obok
    /// istniejących, rzędami od dołu — najpierw w prawo, a gdy przekroczyłby
    /// `max_width`, w kolejnym rzędzie wyżej; bez nachodzenia na inne elementy.
    fn free_spot(&self, size: Vec2, max_width: Option<f64>) -> Vec2 {
        let gap = Self::ELEMENT_GAP;
        let Some(all) = self.bbox else { return Vec2::new(0.0, 0.0) };
        // prostokąty zajęte przez elementy (pliki)
        let mut boxes: Vec<Aabb2> = Vec::new();
        for s in 0..self.sources.len().max(1) {
            if let Some(b) = self.contours.iter().filter(|c| c.source == s).map(|c| c.bbox).reduce(|a, b| a.union(&b)) {
                boxes.push(b);
            }
        }
        if boxes.is_empty() {
            boxes.push(all);
        }
        let limit = max_width.map(|w| all.min.x + w.max(size.x));
        // kandydaci: na prawo od każdego elementu (na wysokości jego dołu) i nad całością
        let mut cand: Vec<Vec2> = boxes.iter().map(|b| Vec2::new(b.max.x + gap, b.min.y)).collect();
        cand.extend(boxes.iter().map(|b| Vec2::new(b.min.x, b.max.y + gap)));
        cand.push(Vec2::new(all.min.x, all.max.y + gap));
        cand.sort_by(|a, b| (a.y, a.x).partial_cmp(&(b.y, b.x)).unwrap());
        let free = |p: Vec2| {
            let r = Aabb2 { min: p, max: p + size };
            limit.is_none_or(|l| r.max.x <= l + 1e-6)
                && boxes.iter().all(|b| {
                    let b = b.expanded(gap * 0.5 - 1e-6);
                    let r = r.expanded(gap * 0.5 - 1e-6);
                    r.max.x <= b.min.x || r.min.x >= b.max.x || r.max.y <= b.min.y || r.min.y >= b.max.y
                })
        };
        cand.into_iter().find(|&p| free(p)).unwrap_or(Vec2::new(all.min.x, all.max.y + gap))
    }

    /// Przesuwa cały rysunek tak, żeby lewy dolny róg półfabrykatu był w punkcie
    /// 0,0 (zero programu). Bez wpisu w historii — do użycia przy wczytaniu.
    pub fn place_at_origin(&mut self) {
        let Some(sb) = self.stock_box() else { return };
        let d = Vec2::new(0.0, 0.0) - sb.min;
        if d.length() < 1e-9 {
            return;
        }
        for i in 0..self.contours.len() {
            self.contours[i].contour.translate(d);
            self.rebuild(i);
        }
        if let Some(m) = &mut self.stock.model {
            m.translate(d);
        }
        self.geometry_changed();
    }

    /// Liczba krzywych każdego elementu.
    pub fn source_counts(&self) -> Vec<usize> {
        let mut n = vec![0; self.sources.len()];
        for c in &self.contours {
            if let Some(k) = n.get_mut(c.source) {
                *k += 1;
            }
        }
        n
    }

    /// Zaznacza krzywe elementu `s` (tylko je).
    pub fn select_source(&mut self, s: usize) {
        for i in 0..self.contours.len() {
            let v = self.contours[i].source == s;
            self.set_selected(i, v);
        }
    }

    /// Dokłada drugi dokument (dodany plik DXF lub projekt) jako nowy element:
    /// krzywe stają obok dotychczasowych i zostają zaznaczone, warstwy o tej
    /// samej nazwie są łączone, operacje projektu dochodzą razem z krzywymi.
    /// Zwraca liczbę dodanych krzywych i operacji.
    /// `max_width` — szerokość, w której mają się zmieścić elementy (pole robocze
    /// maszyny minus naddatki); gdy nowy element się nie mieści, idzie do rzędu wyżej.
    pub fn merge(&mut self, other: Document, max_width: Option<f64>) -> (usize, usize) {
        let name = other.file_name();
        let d = match other.bbox {
            Some(b) if !self.contours.is_empty() => self.free_spot(b.size(), max_width) - b.min,
            _ => Vec2::new(0.0, 0.0),
        };
        let layer_map: Vec<usize> = other
            .layers
            .iter()
            .map(|l| match self.layers.iter().position(|x| x.name == l.name) {
                Some(i) => i,
                None => {
                    self.layers.push(Layer { name: l.name.clone(), visible: true });
                    self.layers.len() - 1
                }
            })
            .collect();
        // elementy dodanego projektu; plik DXF to jeden element nazwany jak plik
        let names: Vec<String> = if other.sources.len() > 1 { other.sources.clone() } else { vec![name.clone()] };
        let source_map: Vec<usize> = names
            .iter()
            .map(|n| {
                let mut unique = n.clone();
                let mut k = 2;
                while self.sources.contains(&unique) {
                    unique = format!("{n} ({k})");
                    k += 1;
                }
                self.sources.push(unique);
                self.sources.len() - 1
            })
            .collect();
        let base = self.contours.len();
        self.selected.iter_mut().for_each(|s| *s = false);
        for c in other.contours {
            let mut k = c.contour;
            k.translate(d);
            let layer = layer_map.get(c.layer).copied().unwrap_or(0);
            let source = source_map.get(c.source).copied().unwrap_or(source_map[0]);
            self.contours.push(DocContour::new(k, layer).with_source(source));
            self.selected.push(true);
        }
        let n_ops = other.ops.len();
        for mut op in other.ops {
            op.contours.iter_mut().for_each(|i| *i += base);
            self.add_op(op);
        }
        self.selected_op = None;
        let n = self.contours.len() - base;
        self.geometry_changed();
        self.label(Mark::new(Icon::Open).badge(Badge::Add), tr!("Dodano element: {}", name));
        (n, n_ops)
    }

    /// Przywraca stan z historii zmian (krzywe, operacje, półfabrykat, ustawienia G-code).
    pub fn restore_state(
        &mut self,
        contours: Vec<DocContour>,
        layers: &[String],
        sources: Vec<String>,
        ops: Vec<Operation>,
        stock: Stock,
        post: PostSettings,
    ) {
        // widoczność warstw zostaje taka, jak ustawił użytkownik
        self.layers = layers
            .iter()
            .map(|n| Layer { name: n.clone(), visible: self.layers.iter().find(|l| &l.name == n).is_none_or(|l| l.visible) })
            .collect();
        self.sources = sources;
        self.contours = contours;
        self.selected = vec![false; self.contours.len()];
        self.op_cache = ops.iter().map(|_| None).collect();
        self.ops = ops;
        self.selected_op = self.selected_op.filter(|&i| i < self.ops.len());
        self.stock = stock;
        self.post = post;
        self.edit_label = None;
        self.geometry_changed();
    }

    /// Wykonuje przekształcenie na zaznaczonych krzywych. Zwraca false, gdy nic nie zaznaczono.
    pub fn transform_selected(&mut self, f: impl Fn(&mut Contour)) -> bool {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return false;
        }
        for i in sel {
            f(&mut self.contours[i].contour);
            self.rebuild(i);
        }
        self.geometry_changed();
        true
    }

    pub fn move_selected(&mut self, d: Vec2) -> bool {
        let ok = self.transform_selected(|c| c.translate(d));
        if ok {
            self.label(Mark::new(Icon::Move), tr!("Przesunięcie"));
        }
        ok
    }

    pub fn rotate_selected(&mut self, deg: f64) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        self.label(Mark::new(Icon::Rotate), tr!("Obrót {}°", deg));
        self.step_param = Some(ParamEdit::Rotate { selection: self.selected_indices(), deg });
        self.transform_selected(|k| k.rotate(c, deg.to_radians()))
    }

    pub fn scale_selected(&mut self, factor: f64) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        let pct = (factor * 1e4).round() / 1e2;
        self.label(Mark::new(Icon::Scale), tr!("Skalowanie {}%", pct));
        self.step_param = Some(ParamEdit::Scale { selection: self.selected_indices(), pct });
        self.transform_selected(|k| k.scale(c, factor))
    }

    pub fn flip_selected(&mut self, horizontal: bool) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        if horizontal {
            self.label(Mark::new(Icon::FlipH), tr!("Odbicie w poziomie"));
        } else {
            self.label(Mark::new(Icon::FlipV), tr!("Odbicie w pionie"));
        }
        if horizontal {
            self.transform_selected(|k| k.mirror_x(c.x))
        } else {
            self.transform_selected(|k| k.mirror_y(c.y))
        }
    }

    /// Wyrównuje każdą zaznaczoną krzywą do krawędzi/środka całego zaznaczenia.
    pub fn align_selected(&mut self, mode: AlignMode) -> bool {
        let Some(all) = self.selection_bbox() else { return false };
        self.label(Mark::new(Icon::Align), tr!("Wyrównanie"));
        for i in self.selected_indices() {
            let bb = self.contours[i].bbox;
            let d = match mode {
                AlignMode::Left => Vec2::new(all.min.x - bb.min.x, 0.0),
                AlignMode::Right => Vec2::new(all.max.x - bb.max.x, 0.0),
                AlignMode::CenterX => Vec2::new(all.center().x - bb.center().x, 0.0),
                AlignMode::Bottom => Vec2::new(0.0, all.min.y - bb.min.y),
                AlignMode::Top => Vec2::new(0.0, all.max.y - bb.max.y),
                AlignMode::CenterY => Vec2::new(0.0, all.center().y - bb.center().y),
            };
            self.contours[i].contour.translate(d);
            self.rebuild(i);
        }
        self.geometry_changed();
        true
    }

    /// Szyk z odstępami między krawędziami kopii; parametry trafiają do historii.
    pub fn array_with_gaps(&mut self, cols: usize, rows: usize, gap_x: f64, gap_y: f64) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let selection = self.selected_indices();
        let s = bb.size();
        let ok = self.array_selected(cols, rows, s.x + gap_x, s.y + gap_y);
        if ok {
            self.step_param = Some(ParamEdit::Array { selection, cols, rows, gap_x, gap_y });
        }
        ok
    }

    /// Szyk: kopiuje zaznaczenie w siatkę `cols` × `rows` z odstępem (dx, dy).
    /// Kopie są dodawane do operacji, które zawierały oryginał.
    pub fn array_selected(&mut self, cols: usize, rows: usize, dx: f64, dy: f64) -> bool {
        let sel = self.selected_indices();
        if sel.is_empty() || cols * rows < 2 {
            return false;
        }
        for r in 0..rows {
            for c in 0..cols {
                if r == 0 && c == 0 {
                    continue;
                }
                let d = Vec2::new(dx * c as f64, dy * r as f64);
                for &i in &sel {
                    let mut k = self.contours[i].contour.clone();
                    k.translate(d);
                    let new_idx = self.contours.len();
                    let (layer, source) = (self.contours[i].layer, self.contours[i].source);
                    self.contours.push(DocContour::new(k, layer).with_source(source));
                    self.selected.push(true);
                    for op in &mut self.ops {
                        if op.contours.contains(&i) {
                            op.contours.push(new_idx);
                        }
                    }
                }
            }
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::Array), tr!("Szyk {} × {}", cols, rows));
        true
    }

    /// Przesuwa całą geometrię tak, by punkt `p` stał się zerem programu (X0 Y0).
    pub fn set_zero(&mut self, p: Vec2) {
        for i in 0..self.contours.len() {
            self.contours[i].contour.translate(-p);
            self.rebuild(i);
        }
        if let Some(m) = &mut self.stock.model {
            m.translate(Vec2::new(0.0, 0.0) - p);
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::ZeroPick), tr!("Zero programu"));
    }

    /// Punkt przyciągania do wskazywania: środek okręgu lub wierzchołek w promieniu `tol`.
    pub fn snap_point(&self, p: Vec2, tol: f64) -> Vec2 {
        let mut best = (p, tol);
        for c in &self.contours {
            if !c.bbox.expanded(tol).contains_point(p) && c.contour.as_circle().is_none() {
                continue;
            }
            if let Some((center, _)) = c.contour.as_circle() {
                if center.distance(p) < best.1 {
                    best = (center, center.distance(p));
                }
            }
            for v in &c.contour.verts {
                if v.p.distance(p) < best.1 {
                    best = (v.p, v.p.distance(p));
                }
            }
        }
        best.0
    }

    pub fn total_minutes(&self) -> f64 {
        self.ops
            .iter()
            .zip(&self.op_cache)
            .filter(|(op, _)| op.enabled)
            .filter_map(|(_, c)| c.as_ref())
            .map(|c| c.minutes)
            .sum()
    }

    /// Program G-code ze wszystkich włączonych operacji.
    /// Program G-code: jeden plik albo (tryb „osobne pliki”) kilka — (przyrostek, treść).
    pub fn gcode(&self, lib: &ToolLibrary, machine: &Machine) -> Result<Vec<(String, String)>, String> {
        // obróbka dwustronna: osobne pliki dla strony A i B
        let two = self.stock.two_sided.is_some() && self.ops.iter().any(|o| o.enabled && o.side == cam_ops::Side::B);
        if !two {
            let files = self.gcode_side(lib, machine, None)?;
            if files.is_empty() {
                return Err(tr!("Brak włączonych operacji ze ścieżkami").into());
            }
            return Ok(files);
        }
        let mut out = Vec::new();
        for (side, name) in [(cam_ops::Side::A, "strona_A"), (cam_ops::Side::B, "strona_B")] {
            match self.gcode_side(lib, machine, Some(side)) {
                Ok(files) => {
                    for (suffix, text) in files {
                        let s = if suffix.is_empty() { name.to_string() } else { format!("{name}_{suffix}") };
                        out.push((s, text));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        if out.is_empty() {
            return Err(tr!("Brak włączonych operacji ze ścieżkami").into());
        }
        Ok(out)
    }

    fn gcode_side(&self, lib: &ToolLibrary, machine: &Machine, side: Option<cam_ops::Side>) -> Result<Vec<(String, String)>, String> {
        let mut prog = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            if !op.enabled || side.is_some_and(|s| s != op.side) {
                continue;
            }
            let tool = lib.get(op.tool_id).ok_or_else(|| tr!("Operacja „{}”: brak narzędzia w bibliotece", op.name))?;
            let Some(cache) = self.op_cache.get(i).and_then(|c| c.as_ref()) else {
                return Err(tr!("Operacja „{}” nie ma wygenerowanej ścieżki", op.name));
            };
            if cache.result.toolpath.moves.is_empty() {
                continue;
            }
            prog.push(cam_post::ProgramOp {
                name: &op.name,
                tool,
                rpm: op.cut.rpm,
                safe_z: op.cut.safe_z,
                toolpath: &cache.result.toolpath,
            });
        }
        if prog.is_empty() {
            return Ok(Vec::new()); // brak operacji tej strony
        }
        let mut title = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "program".into());
        if let Some(s) = side {
            title.push_str(if s == cam_ops::Side::A { tr!(" - strona A") } else { tr!(" - strona B (po odwróceniu, Z0 na nowej górze)") });
        }
        let post = machine.post_settings();
        if post.tool_change == cam_post::ToolChange::SeparateFiles {
            Ok(cam_post::emit_split(&title, &prog, &post))
        } else {
            Ok(vec![(String::new(), cam_post::emit(&title, &prog, &post))])
        }
    }
}

/// Ostrzeżenia wynikające z możliwości maszyny.
fn check_machine(op: &Operation, tp: &cam_ops::Toolpath, m: &Machine, warnings: &mut Vec<String>) {
    if op.cut.feed > m.max_feed_xy + 1e-6 {
        warnings.push(tr!("Posuw {} mm/min większy niż maks. maszyny ({})", op.cut.feed, m.max_feed_xy));
    }
    if op.cut.plunge_feed > m.max_feed_z + 1e-6 {
        warnings.push(tr!("Posuw wgłębny {} mm/min większy niż maks. Z maszyny ({})", op.cut.plunge_feed, m.max_feed_z));
    }
    if m.spindle != cam_post::SpindleKind::Manual && m.rpm_max > 0.0 && op.cut.rpm > m.rpm_max + 0.5 {
        warnings.push(tr!("Obroty {} większe niż maks. wrzeciona ({}) — zostaną ograniczone", op.cut.rpm, m.rpm_max));
    }
    if op.cut.depth > m.travel_z {
        warnings.push(tr!("Głębokość większa niż zakres Z maszyny ({} mm)", m.travel_z));
    }
    let mut bb = Aabb2::EMPTY;
    for mv in &tp.moves {
        let e = mv.end();
        bb.add_point(e.xy());
    }
    if !bb.is_empty() {
        let s = bb.size();
        if s.x > m.travel_x + 1e-6 || s.y > m.travel_y + 1e-6 {
            warnings.push(tr!("Ścieżka {:.0} × {:.0} mm większa niż pole robocze {:.0} × {:.0} mm",
                s.x, s.y, m.travel_x, m.travel_y
            ));
        }
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use cam_geom::Vertex;

    /// Dokument z prostokątem w × h zaczynającym się w (x, y).
    fn rect_doc(name: &str, x: f64, y: f64, w: f64, h: f64) -> Document {
        let mut c = Contour::new(true);
        for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
            c.verts.push(Vertex::new(px, py, 0.0));
        }
        let mut d = Document { path: Some(PathBuf::from(name)), contours: vec![DocContour::new(c, 0)], ..Default::default() };
        d.finish_load();
        d
    }

    #[test]
    fn opened_drawing_lands_at_zero() {
        // rysunek narysowany wokół środka układu, jak na zgłoszonym zrzucie
        let mut d = rect_doc("a.dxf", -150.0, -120.0, 300.0, 240.0);
        d.place_at_origin();
        let sb = d.stock_box().unwrap();
        assert!(sb.min.distance(Vec2::new(0.0, 0.0)) < 1e-9, "lewy dolny róg półfabrykatu w 0,0: {:?}", sb.min);
        assert!((d.bbox.unwrap().min.x - d.stock.margin).abs() < 1e-9);
    }

    #[test]
    fn added_files_go_right_then_to_the_next_row() {
        let mut d = rect_doc("a.dxf", 0.0, 0.0, 100.0, 50.0);
        let width = Some(250.0);
        d.merge(rect_doc("b.dxf", 500.0, 500.0, 100.0, 50.0), width);
        d.merge(rect_doc("c.dxf", -40.0, 7.0, 100.0, 50.0), width);
        d.merge(rect_doc("d.dxf", 0.0, 0.0, 100.0, 50.0), width);
        let at = |s: usize| d.contours.iter().filter(|c| c.source == s).map(|c| c.bbox.min).next().unwrap();
        assert_eq!(at(1), Vec2::new(110.0, 0.0), "drugi obok pierwszego");
        assert_eq!(at(2), Vec2::new(0.0, 60.0), "trzeci się nie mieści — rząd wyżej");
        assert_eq!(at(3), Vec2::new(110.0, 60.0), "czwarty obok trzeciego");
    }
}

#[cfg(test)]
mod two_sided_tests {
    use super::*;
    use cam_geom::Vertex;

    fn doc_with_rect() -> (Document, ToolLibrary) {
        let mut c = Contour::new(true);
        for (x, y) in [(10.0, 10.0), (60.0, 10.0), (60.0, 40.0), (10.0, 40.0)] {
            c.verts.push(Vertex::new(x, y, 0.0));
        }
        let mut d = Document { path: Some(PathBuf::from("a.dxf")), contours: vec![DocContour::new(c, 0)], ..Default::default() };
        d.finish_load();
        let lib = ToolLibrary::with_defaults();
        let tool = lib.tools.iter().find(|t| t.kind == cam_ops::ToolKind::FlatEnd).unwrap();
        let mut cut = cam_ops::CutParams { depth: 3.0, ..Default::default() };
        cut.apply_tool(tool, cam_ops::DEFAULT_MATERIAL);
        d.add_op(Operation {
            side: cam_ops::Side::A,
            name: "Profil".into(),
            enabled: true,
            kind: cam_ops::OpKind::Profile(Default::default()),
            tool_id: tool.id,
            cut,
            contours: vec![0],
        });
        (d, lib)
    }

    #[test]
    fn side_b_is_mirrored_for_the_machine_and_drawn_in_place() {
        let (mut d, lib) = doc_with_rect();
        d.stock.two_sided = Some(TwoSided { axis: FlipAxis::Y, pos: 50.0, ..Default::default() });
        d.regenerate(&lib, None);
        let a_disp = d.op_cache[0].as_ref().unwrap().display.clone();
        let a_tp = d.op_cache[0].as_ref().unwrap().result.toolpath.clone();
        d.ops[0].side = cam_ops::Side::B;
        d.regenerate(&lib, None);
        let b = d.op_cache[0].as_ref().unwrap();
        // ścieżka maszyny odbita: x → 100 − x (prostokąt 10..60 → 40..90 plus promień)
        let xs = |tp: &cam_ops::Toolpath| {
            let v: Vec<f64> = tp.moves.iter().map(|m| m.end().x).collect();
            (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max))
        };
        let (a_min, a_max) = xs(&a_tp);
        let (b_min, b_max) = xs(&b.result.toolpath);
        assert!((b_min - (100.0 - a_max)).abs() < 1e-6 && (b_max - (100.0 - a_min)).abs() < 1e-6);
        // na rysunku strona B leży tam, gdzie krzywa (zakres X jak strony A)
        let dx = |pts: &[(P3, bool)]| pts.iter().filter(|(p, r)| !r && p.z < 0.0).map(|(p, _)| p.x).fold(f64::MAX, f64::min);
        assert!((dx(&b.display) - dx(&a_disp)).abs() < 1e-6);

        // G-code: osobne pliki dla stron
        let mut d2 = d;
        let op_a = Operation { side: cam_ops::Side::A, ..d2.ops[0].clone() };
        d2.add_op(op_a);
        d2.regenerate(&lib, None);
        let files = d2.gcode(&lib, &Machine::default()).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["strona_A", "strona_B"]);
    }

    #[test]
    fn pins_sit_on_the_axis_outside_the_part() {
        let (mut d, _) = doc_with_rect();
        d.stock.two_sided = Some(TwoSided { axis: FlipAxis::X, pos: 25.0, pin_diameter: 6.0, pin_gap: 8.0, ..Default::default() });
        let [a, b] = d.pin_centers().unwrap();
        assert_eq!(a, Vec2::new(10.0 - 8.0 - 3.0, 25.0));
        assert_eq!(b, Vec2::new(60.0 + 8.0 + 3.0, 25.0));
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;

    #[test]
    fn model_sets_the_stock_and_moves_with_the_zero() {
        let mesh = cam_io::Mesh {
            tris: vec![[[0.0, 0.0, 0.0], [40.0, 0.0, 0.0], [40.0, 30.0, 12.0]], [[0.0, 30.0, 12.0], [0.0, 0.0, 0.0], [40.0, 30.0, 0.0]]],
        };
        let mut d = Document::default();
        d.finish_load();
        d.set_model(crate::model3d::Model3D::new("m.stl".into(), mesh, 1.0));
        assert_eq!(d.stock.thickness, 12.0, "grubość według wysokości modelu");
        d.place_at_origin();
        let sb = d.stock_box().unwrap();
        assert!(sb.min.distance(Vec2::new(0.0, 0.0)) < 1e-9, "róg półfabrykatu w zerze");
        let fp = d.stock.model.as_ref().unwrap().footprint();
        assert!((fp.min.x - d.stock.margin).abs() < 1e-9 && (fp.size().x - 40.0).abs() < 1e-6);
        // zero w środku: model przesuwa się razem z rysunkiem
        let c = sb.center();
        d.set_zero(c);
        let fp2 = d.stock.model.as_ref().unwrap().footprint();
        assert!((fp2.center().x).abs() < 1e-6 && (fp2.center().y).abs() < 1e-6);
    }
}
