//! Automatyzacja: operacje dla krzywych bez operacji (według warstw albo
//! kształtu), uszka w narożnikach, mostki według obwodu, kolejność według
//! narzędzi. Najpierw plan z podsumowaniem, potem zastosowanie jednym krokiem.

use cam_i18n::tr;
use crate::doc::{count_tool_changes, grouped_order, Document};
use crate::toolbar::Sub;
use cam_ops::{CutParams, OpKind, Operation, ProfileSide, Tabs, ToolKind, ToolLibrary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Co zrobić z krzywymi warstwy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    /// Rozpoznaj z kształtu: okrąg w rozmiarze wiertła → wiercenie, kontur
    /// zewnętrzny → profil zewnętrzny, kontur w środku detalu → profil
    /// wewnętrzny, linia otwarta → grawer.
    Auto,
    Drill,
    Pocket,
    ProfileOut,
    ProfileIn,
    Engrave,
    Skip,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Auto,
        Category::Drill,
        Category::Pocket,
        Category::ProfileOut,
        Category::ProfileIn,
        Category::Engrave,
        Category::Skip,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::Auto => tr!("z kształtu"),
            Category::Drill => tr!("wiercenie"),
            Category::Pocket => tr!("kieszeń"),
            Category::ProfileOut => tr!("profil zewnętrzny"),
            Category::ProfileIn => tr!("profil wewnętrzny"),
            Category::Engrave => tr!("grawer"),
            Category::Skip => tr!("pomiń"),
        }
    }

    /// Podpowiedź z nazwy warstwy.
    pub fn from_layer_name(name: &str) -> Category {
        let n = name.to_lowercase();
        let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
        if has(&["otw", "wierc", "hole", "drill"]) {
            Category::Drill
        } else if has(&["kiesz", "pocket"]) {
            Category::Pocket
        } else if has(&["wewn", "inside", "inner"]) {
            Category::ProfileIn
        } else if has(&["obrys", "kontur", "outline", "profil", "cut", "ciec", "cięc"]) {
            Category::ProfileOut
        } else if has(&["graw", "engrav", "napis", "text", "tekst"]) {
            Category::Engrave
        } else {
            Category::Auto
        }
    }

    /// Kolejność tworzenia operacji: wiercenie, kieszenie, grawer, otwory, wycięcie.
    fn rank(&self) -> usize {
        match self {
            Category::Drill => 0,
            Category::Pocket => 1,
            Category::Engrave => 2,
            Category::ProfileIn => 3,
            Category::ProfileOut => 4,
            Category::Auto | Category::Skip => 5,
        }
    }

    fn sub(&self) -> Option<Sub> {
        Some(match self {
            Category::Drill => Sub::DrillSimple,
            Category::Pocket => Sub::PocketOffset,
            Category::ProfileOut => Sub::ProfileOut,
            Category::ProfileIn => Sub::ProfileIn,
            Category::Engrave => Sub::EngCenter,
            Category::Auto | Category::Skip => return None,
        })
    }
}

/// Ustawienia okna automatyzacji.
pub struct AutoOptions {
    pub create_ops: bool,
    pub corners: bool,
    pub tabs: bool,
    pub order: bool,
    /// Wybór dla każdej warstwy (po nazwie).
    pub layers: BTreeMap<String, Category>,
}

impl Default for AutoOptions {
    fn default() -> Self {
        AutoOptions { create_ops: true, corners: true, tabs: true, order: true, layers: BTreeMap::new() }
    }
}

/// Plan: nowe operacje i liczby do podsumowania.
pub struct Plan {
    pub new_ops: Vec<Operation>,
    /// Krzywe pominięte (np. okrąg bez pasującego wiertła przy wyborze „wiercenie”).
    pub unmatched: usize,
    pub corners: usize,
    pub tabs: usize,
    pub changes_before: usize,
    pub changes_after: usize,
}

impl Plan {
    /// Podsumowanie do pokazania przed zastosowaniem.
    pub fn summary(&self, opts: &AutoOptions) -> Vec<String> {
        let mut out = Vec::new();
        if opts.create_ops {
            if self.new_ops.is_empty() {
                out.push(tr!("Nowe operacje: brak (wszystkie krzywe mają już operacje albo są pominięte)").into());
            }
            for op in &self.new_ops {
                out.push(tr!("Nowa operacja: {} — {}, gł. {} mm",
                    op.name,
                    pl(op.contours.len(), tr!("krzywa"), tr!("krzywe"), tr!("krzywych")),
                    op.cut.depth
                ));
            }
            if self.unmatched > 0 {
                out.push(tr!("Bez operacji zostaje {} (brak pasującego narzędzia)",
                    pl(self.unmatched, tr!("krzywa"), tr!("krzywe"), tr!("krzywych"))
                ));
            }
        }
        if opts.corners {
            out.push(tr!("Uszka w narożnikach: {}", pl(self.corners, tr!("operacja"), tr!("operacje"), tr!("operacji"))));
        }
        if opts.tabs {
            out.push(tr!("Mostki według obwodu: {}", pl(self.tabs, tr!("operacja"), tr!("operacje"), tr!("operacji"))));
        }
        if opts.order {
            if self.changes_after < self.changes_before {
                out.push(tr!("Kolejność według narzędzi: {} zamiast {}",
                    pl(self.changes_after, tr!("zmiana narzędzia"), tr!("zmiany narzędzia"), tr!("zmian narzędzia")),
                    self.changes_before
                ));
            } else {
                out.push(tr!("Kolejność: bez zmian ({})",
                    pl(self.changes_before, tr!("zmiana narzędzia"), tr!("zmiany narzędzia"), tr!("zmian narzędzia"))
                ));
            }
        }
        out
    }
}

/// Polska liczba mnoga: 1 operacja, 2 operacje, 5 operacji.
fn pl(n: usize, one: &str, few: &str, many: &str) -> String {
    let w = if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    };
    tr!("{} {}", n, w)
}

/// Wiertło z biblioteki o średnicy okręgu (±0,1 mm).
fn drill_for(lib: &ToolLibrary, d: f64) -> Option<u32> {
    lib.tools.iter().find(|t| matches!(t.kind, ToolKind::Drill { .. }) && (t.diameter - d).abs() <= 0.1).map(|t| t.id)
}

/// Narzędzie do graweru: grawerskie albo V, a gdy ich nie ma — frez główny.
fn engraver(lib: &ToolLibrary, main: u32) -> u32 {
    lib.tools
        .iter()
        .find(|t| matches!(t.kind, ToolKind::Engraver { .. } | ToolKind::VBit { .. }))
        .map_or(main, |t| t.id)
}

/// Liczba mostków dla obwodu [mm]: około jeden na 150 mm, od 2 do 12.
pub fn tabs_for_perimeter(len: f64) -> u32 {
    if len < 60.0 {
        2
    } else {
        ((len / 150.0).round() as u32).clamp(3, 12)
    }
}

pub fn plan(doc: &Document, lib: &ToolLibrary, main_tool: Option<u32>, safe_z: f64, opts: &AutoOptions) -> Plan {
    let mut new_ops: Vec<Operation> = Vec::new();
    let mut unmatched = 0;
    let main = main_tool
        .filter(|id| lib.get(*id).is_some())
        .or_else(|| lib.tools.iter().find(|t| t.kind == ToolKind::FlatEnd).map(|t| t.id))
        .or_else(|| lib.tools.first().map(|t| t.id));

    if opts.create_ops {
        if let Some(main) = main {
            let used: Vec<usize> = doc.ops.iter().flat_map(|o| o.contours.iter().copied()).collect();
            // (kategoria, narzędzie) → krzywe
            // (kolejność, narzędzie, frezowanie otworu po helisie) → krzywe
            let mut groups: BTreeMap<(usize, u32, bool), (Category, Vec<usize>)> = BTreeMap::new();
            let main_d = lib.get(main).map_or(0.0, |t| t.diameter);
            for i in 0..doc.contours.len() {
                if used.contains(&i) || !doc.is_visible(i) {
                    continue;
                }
                let c = &doc.contours[i].contour;
                let layer = doc.layers.get(doc.contours[i].layer).map_or("", |l| l.name.as_str());
                let chosen = opts.layers.get(layer).copied().unwrap_or_else(|| Category::from_layer_name(layer));
                let circle_drill = c.as_circle().and_then(|(_, r)| drill_for(lib, r * 2.0));
                let cat = match chosen {
                    Category::Skip => continue,
                    Category::Auto => {
                        if !c.closed {
                            Category::Engrave
                        } else if circle_drill.is_some() {
                            Category::Drill
                        } else {
                            let p0 = c.start().unwrap_or_default();
                            let depth = (0..doc.contours.len())
                                .filter(|&j| j != i && doc.contours[j].contour.closed && doc.contours[j].contour.contains_point(p0))
                                .count();
                            if depth % 2 == 0 { Category::ProfileOut } else { Category::ProfileIn }
                        }
                    }
                    other => other,
                };
                let mut helix = false;
                let tool = match cat {
                    Category::Drill => match (circle_drill, c.as_circle()) {
                        (Some(t), _) => t,
                        // brak wiertła tej średnicy: otwór większy niż frez — frezowanie po helisie
                        (None, Some((_, r))) if r * 2.0 > main_d * 1.05 => {
                            helix = true;
                            main
                        }
                        _ => {
                            unmatched += 1;
                            continue;
                        }
                    },
                    Category::Pocket | Category::ProfileIn | Category::ProfileOut if !c.closed => {
                        unmatched += 1;
                        continue;
                    }
                    Category::Engrave => engraver(lib, main),
                    _ => main,
                };
                groups.entry((cat.rank(), tool, helix)).or_insert((cat, Vec::new())).1.push(i);
            }
            for ((_, tool_id, helix), (cat, contours)) in groups {
                let sub = if helix { Some(Sub::DrillHelix) } else { cat.sub() };
                let (Some(sub), Some(tool)) = (sub, lib.get(tool_id)) else { continue };
                let Some((kind, depth, label)) = sub.op_template(doc.stock.thickness) else { continue };
                let mut cut = CutParams { depth, safe_z, ..Default::default() };
                cut.apply_tool(tool, &doc.stock.material);
                let n = doc.ops.iter().chain(&new_ops).filter(|o| o.name.starts_with(label)).count() + 1;
                new_ops.push(Operation { side: cam_ops::Side::A, name: format!("{label} {n}"), enabled: true, kind, tool_id, cut, contours });
            }
        }
    }

    let all: Vec<&Operation> = doc.ops.iter().chain(&new_ops).collect();
    let corners = all
        .iter()
        .filter(|o| matches!(&o.kind, OpKind::Pocket(_)) || matches!(&o.kind, OpKind::Profile(p) if p.side == ProfileSide::Inside))
        .count();
    let tabs = all.iter().filter(|o| matches!(&o.kind, OpKind::Profile(p) if p.side == ProfileSide::Outside)).count();
    let owned: Vec<Operation> = all.iter().map(|o| (*o).clone()).collect();
    let changes_before = count_tool_changes(owned.iter());
    let changes_after = count_tool_changes(grouped_order(&owned).into_iter().map(|i| &owned[i]));
    Plan { new_ops, unmatched, corners, tabs, changes_before, changes_after }
}

/// Wykonuje plan na dokumencie (jeden krok w historii).
pub fn apply(doc: &mut Document, plan: Plan, opts: &AutoOptions) {
    let added = plan.new_ops.len();
    for op in plan.new_ops {
        doc.add_op(op);
    }
    for op in &mut doc.ops {
        match &mut op.kind {
            OpKind::Pocket(p) if opts.corners => p.corners = true,
            OpKind::Profile(p) if opts.corners && p.side == ProfileSide::Inside => p.corners = true,
            OpKind::Profile(p) if opts.tabs && p.side == ProfileSide::Outside => {
                let len = op.contours.iter().filter_map(|&i| doc.contours.get(i)).map(|c| c.length).fold(0.0, f64::max);
                let count = tabs_for_perimeter(len);
                match &mut p.tabs {
                    Some(t) => t.count = count,
                    None => p.tabs = Some(Tabs { count, width: 6.0, height: 2.0_f64.min(op.cut.depth * 0.3) }),
                }
            }
            _ => {}
        }
    }
    if opts.order && doc.tool_changes_if_grouped() < doc.tool_changes() {
        doc.group_by_tool();
    }
    doc.selected_op = None;
    doc.ops_dirty = true;
    doc.revision += 1;
    doc.label(
        crate::history::Mark::new(crate::icons::Icon::Document).badge(crate::history::Badge::Add),
        tr!("Automatyzacja ({} nowych operacji)", added),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn layers_become_operations_and_extras_are_applied() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut doc = Document::load_dxf(&root.join("samples/test.dxf")).unwrap();
        let lib = ToolLibrary::with_defaults();
        let opts = AutoOptions::default();
        let plan = plan(&doc, &lib, None, 5.0, &opts);
        let kinds: Vec<&str> = plan.new_ops.iter().map(|o| o.kind.label()).collect();
        // test.dxf: OTWORY (okręgi), KIESZEN, OBRYS, GRAWER (linie otwarte)
        assert!(
            kinds.contains(&"Kieszeń") && kinds.contains(&"Profil") && kinds.contains(&"Grawer") && kinds.contains(&"Wiercenie"),
            "{kinds:?}"
        );
        // otwory Ø6,5 bez wiertła tej średnicy: frezowanie po helisie frezem Ø6
        assert!(plan.new_ops.iter().any(|o| matches!(&o.kind, OpKind::Drill(d) if d.helix) && o.contours.len() == 4));
        assert_eq!(plan.unmatched, 0);
        assert_eq!(*kinds.last().unwrap(), "Profil", "wycięcie detalu na końcu");
        let covered: usize = plan.new_ops.iter().map(|o| o.contours.len()).sum::<usize>() + plan.unmatched;
        assert_eq!(covered, doc.contours.len(), "każda krzywa ma operację albo jest policzona jako bez narzędzia");

        apply(&mut doc, plan, &opts);
        let last = doc.ops.last().unwrap();
        assert!(matches!(&last.kind, OpKind::Profile(p) if p.side == ProfileSide::Outside && p.tabs.is_some()));
        assert!(doc.ops.iter().all(|o| match &o.kind {
            OpKind::Pocket(p) => p.corners,
            OpKind::Profile(p) if p.side == ProfileSide::Inside => p.corners,
            _ => true,
        }));
        // drugi raz: nic nowego, krzywe mają już operacje
        assert!(super::plan(&doc, &lib, None, 5.0, &opts).new_ops.is_empty());
    }

    #[test]
    fn layer_names_and_tab_counts() {
        assert_eq!(Category::from_layer_name("OTWORY"), Category::Drill);
        assert_eq!(Category::from_layer_name("Kieszeń_3mm"), Category::Pocket);
        assert_eq!(Category::from_layer_name("OBRYS"), Category::ProfileOut);
        assert_eq!(Category::from_layer_name("0"), Category::Auto);
        assert_eq!(tabs_for_perimeter(40.0), 2);
        assert_eq!(tabs_for_perimeter(300.0), 3);
        assert_eq!(tabs_for_perimeter(900.0), 6);
    }
}
