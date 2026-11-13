//! Model dokumentu: geometria, warstwy, zaznaczenie, półfabrykat, operacje.

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
}

impl Default for Stock {
    fn default() -> Self {
        Self { thickness: 18.0, margin: 5.0, material: "Drewno".into() }
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
        self.bbox = self.contours.iter().map(|c| c.bbox).reduce(|a, b| a.union(&b));
        self.selected = vec![false; self.contours.len()];
        self.op_cache = self.ops.iter().map(|_| None).collect();
        self.ops_dirty = true;
        self.revision = 1;
    }

    pub fn file_name(&self) -> String {
        self.project_path
            .as_ref()
            .or(self.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bez nazwy".into())
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
            cam_ops::OpKind::Pocket(_) => c.closed,
            cam_ops::OpKind::Drill(_) => c.as_circle().is_some(),
            _ => true,
        }
    }

    /// Dodaje lub usuwa krzywą z operacji. Zwraca komunikat, gdy krzywa nie pasuje.
    pub fn toggle_op_contour(&mut self, op: usize, i: usize) -> Result<(), &'static str> {
        if !self.contour_fits(&self.ops[op].kind, i) {
            return Err(match self.ops[op].kind {
                cam_ops::OpKind::Pocket(_) => "Kieszeń wymaga krzywej zamkniętej",
                cam_ops::OpKind::Drill(_) => "Wiercenie wymaga okręgu",
                _ => "Krzywa nie pasuje do operacji",
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
            self.ops.swap(i, j);
            self.op_cache.swap(i, j);
            if self.selected_op == Some(i) {
                self.selected_op = Some(j);
            }
            self.ops_dirty = true;
        }
    }

    /// Przelicza ścieżki wszystkich operacji.
    pub fn regenerate(&mut self, lib: &ToolLibrary, machine: Option<&Machine>) {
        self.op_cache = self
            .ops
            .iter()
            .map(|op| {
                let tool = lib.get(op.tool_id)?;
                let geom: Vec<Contour> =
                    op.contours.iter().filter_map(|&i| self.contours.get(i)).map(|c| c.contour.clone()).collect();
                let mut result = cam_ops::generate(op, tool, &geom);
                if geom.len() < op.contours.len() {
                    result.warnings.push("Część konturów operacji nie istnieje".into());
                }
                if let Some(m) = machine {
                    check_machine(op, &result.toolpath, m, &mut result.warnings);
                }
                let display = result.toolpath.polyline(0.01);
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
        self.bbox = self.contours.iter().map(|c| c.bbox).reduce(|a, b| a.union(&b));
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
        self.label(Mark::new(Icon::Delete), format!("Usunięto krzywe ({})", sel.len()));
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
            let layer = self.layers.iter().position(|l| l.name == it.layer).unwrap_or(0);
            let source = self.source_index(&it.source);
            self.contours.push(DocContour::new(c, layer).with_source(source));
            self.selected.push(true);
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::Paste).badge(Badge::Add), format!("Wklejono krzywe ({})", items.len()));
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
    pub fn merge(&mut self, other: Document) -> (usize, usize) {
        let name = other.file_name();
        let d = match (self.bbox, other.bbox) {
            (Some(a), Some(b)) if !self.contours.is_empty() => Vec2::new(a.max.x + 10.0 - b.min.x, a.min.y - b.min.y),
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
        self.label(Mark::new(Icon::Open).badge(Badge::Add), format!("Dodano element: {name}"));
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
            self.label(Mark::new(Icon::Move), "Przesunięcie");
        }
        ok
    }

    pub fn rotate_selected(&mut self, deg: f64) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        self.label(Mark::new(Icon::Rotate), format!("Obrót {deg}°"));
        self.step_param = Some(ParamEdit::Rotate { selection: self.selected_indices(), deg });
        self.transform_selected(|k| k.rotate(c, deg.to_radians()))
    }

    pub fn scale_selected(&mut self, factor: f64) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        let pct = (factor * 1e4).round() / 1e2;
        self.label(Mark::new(Icon::Scale), format!("Skalowanie {pct}%"));
        self.step_param = Some(ParamEdit::Scale { selection: self.selected_indices(), pct });
        self.transform_selected(|k| k.scale(c, factor))
    }

    pub fn flip_selected(&mut self, horizontal: bool) -> bool {
        let Some(bb) = self.selection_bbox() else { return false };
        let c = bb.center();
        if horizontal {
            self.label(Mark::new(Icon::FlipH), "Odbicie w poziomie");
        } else {
            self.label(Mark::new(Icon::FlipV), "Odbicie w pionie");
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
        self.label(Mark::new(Icon::Align), "Wyrównanie");
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
        self.label(Mark::new(Icon::Array), format!("Szyk {cols} × {rows}"));
        true
    }

    /// Przesuwa całą geometrię tak, by punkt `p` stał się zerem programu (X0 Y0).
    pub fn set_zero(&mut self, p: Vec2) {
        for i in 0..self.contours.len() {
            self.contours[i].contour.translate(-p);
            self.rebuild(i);
        }
        self.geometry_changed();
        self.label(Mark::new(Icon::ZeroPick), "Zero programu");
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
        let mut prog = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            if !op.enabled {
                continue;
            }
            let tool = lib.get(op.tool_id).ok_or_else(|| format!("Operacja „{}”: brak narzędzia w bibliotece", op.name))?;
            let Some(cache) = self.op_cache.get(i).and_then(|c| c.as_ref()) else {
                return Err(format!("Operacja „{}” nie ma wygenerowanej ścieżki", op.name));
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
            return Err("Brak włączonych operacji ze ścieżkami".into());
        }
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "program".into());
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
        warnings.push(format!("Posuw {} mm/min większy niż maks. maszyny ({})", op.cut.feed, m.max_feed_xy));
    }
    if op.cut.plunge_feed > m.max_feed_z + 1e-6 {
        warnings.push(format!("Posuw wgłębny {} mm/min większy niż maks. Z maszyny ({})", op.cut.plunge_feed, m.max_feed_z));
    }
    if m.spindle != cam_post::SpindleKind::Manual && m.rpm_max > 0.0 && op.cut.rpm > m.rpm_max + 0.5 {
        warnings.push(format!("Obroty {} większe niż maks. wrzeciona ({}) — zostaną ograniczone", op.cut.rpm, m.rpm_max));
    }
    if op.cut.depth > m.travel_z {
        warnings.push(format!("Głębokość większa niż zakres Z maszyny ({} mm)", m.travel_z));
    }
    let mut bb = Aabb2::EMPTY;
    for mv in &tp.moves {
        let e = mv.end();
        bb.add_point(e.xy());
    }
    if !bb.is_empty() {
        let s = bb.size();
        if s.x > m.travel_x + 1e-6 || s.y > m.travel_y + 1e-6 {
            warnings.push(format!(
                "Ścieżka {:.0} × {:.0} mm większa niż pole robocze {:.0} × {:.0} mm",
                s.x, s.y, m.travel_x, m.travel_y
            ));
        }
    }
}
