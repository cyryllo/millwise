//! Zapis/odczyt: globalna biblioteka narzędzi i maszyn oraz pliki projektu (.millwise).

use crate::doc::{DocContour, Document, Layer, Stock};
use anyhow::{Context, Result};
use cam_geom::Contour;
use cam_ops::{Operation, Tool, ToolLibrary};
use cam_post::{MachineLibrary, PostSettings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------- biblioteka narzędzi ----------

/// Plik w katalogu ustawień: ~/.config/millwise (Linux), %APPDATA%\millwise (Windows).
/// Przy pierwszym uruchomieniu po zmianie nazwy kopiuje plik ze starego katalogu „camsoft”.
fn config_file(name: &str) -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let path = base.join("millwise").join(name);
    let old = base.join("camsoft").join(name);
    if !path.exists() && old.exists() {
        let _ = std::fs::create_dir_all(base.join("millwise"));
        let _ = std::fs::copy(&old, &path);
    }
    Some(path)
}

pub fn tools_path() -> Option<PathBuf> {
    config_file("tools.json")
}

pub fn load_tools() -> (ToolLibrary, Option<String>) {
    let Some(path) = tools_path() else {
        return (ToolLibrary::with_defaults(), Some("Brak katalogu konfiguracji — biblioteka nie będzie zapisywana".into()));
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<ToolLibrary>(&text) {
            Ok(lib) => (lib, None),
            Err(e) => {
                // nie nadpisujemy uszkodzonego pliku — zachowujemy kopię
                let backup = path.with_extension("json.bak");
                let _ = std::fs::copy(&path, &backup);
                (
                    ToolLibrary::with_defaults(),
                    Some(format!("Nie można odczytać {}: {e}. Kopia: {}", path.display(), backup.display())),
                )
            }
        },
        Err(_) => {
            let lib = ToolLibrary::with_defaults();
            let warn = save_tools(&lib).err().map(|e| format!("{e:#}"));
            (lib, warn)
        }
    }
}

pub fn save_tools(lib: &ToolLibrary) -> Result<()> {
    let path = tools_path().context("Brak katalogu konfiguracji")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(lib)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ---------- maszyny ----------

pub fn machines_path() -> Option<PathBuf> {
    config_file("machines.json")
}

pub fn load_machines() -> (MachineLibrary, Option<String>) {
    let Some(path) = machines_path() else {
        return (MachineLibrary::with_defaults(), None);
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<MachineLibrary>(&text) {
            Ok(lib) if !lib.machines.is_empty() => (lib, None),
            Ok(_) => (MachineLibrary::with_defaults(), None),
            Err(e) => {
                let backup = path.with_extension("json.bak");
                let _ = std::fs::copy(&path, &backup);
                (
                    MachineLibrary::with_defaults(),
                    Some(format!("Nie można odczytać {}: {e}. Kopia: {}", path.display(), backup.display())),
                )
            }
        },
        Err(_) => {
            let lib = MachineLibrary::with_defaults();
            let warn = save_machines(&lib).err().map(|e| format!("{e:#}"));
            (lib, warn)
        }
    }
}

pub fn save_machines(lib: &MachineLibrary) -> Result<()> {
    let path = machines_path().context("Brak katalogu konfiguracji")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(lib)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ---------- projekt ----------

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: String,
    version: u32,
    source_file: Option<PathBuf>,
    unit_name: String,
    layers: Vec<String>,
    contours: Vec<ProjectContour>,
    stock: Stock,
    operations: Vec<Operation>,
    /// Kopie użytych narzędzi — żeby projekt dało się otworzyć na innym komputerze.
    tools: Vec<Tool>,
    post: PostSettings,
}

#[derive(Serialize, Deserialize)]
struct ProjectContour {
    layer: usize,
    contour: Contour,
}

const FORMAT: &str = "millwise-project";
/// Format projektów sprzed zmiany nazwy programu.
const FORMAT_OLD: &str = "camsoft-project";

pub fn save_project(doc: &Document, lib: &ToolLibrary, path: &Path) -> Result<()> {
    let mut tools: Vec<Tool> = Vec::new();
    for op in &doc.ops {
        if let Some(t) = lib.get(op.tool_id) {
            if !tools.iter().any(|x| x.id == t.id) {
                tools.push(t.clone());
            }
        }
    }
    let pf = ProjectFile {
        format: FORMAT.into(),
        version: 1,
        source_file: doc.path.clone(),
        unit_name: doc.unit_name.clone(),
        layers: doc.layers.iter().map(|l| l.name.clone()).collect(),
        contours: doc.contours.iter().map(|c| ProjectContour { layer: c.layer, contour: c.contour.clone() }).collect(),
        stock: doc.stock.clone(),
        operations: doc.ops.clone(),
        tools,
        post: doc.post.clone(),
    };
    std::fs::write(path, serde_json::to_string_pretty(&pf)?)
        .with_context(|| format!("Nie można zapisać {}", path.display()))?;
    Ok(())
}

/// Wczytuje projekt. Narzędzia z projektu są dopasowywane do biblioteki
/// (po id, potem po numerze i średnicy); brakujące są do niej dodawane.
/// Zwraca dokument i informację, czy biblioteka się zmieniła.
pub fn load_project(path: &Path, lib: &mut ToolLibrary) -> Result<(Document, bool)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("Nie można odczytać {}", path.display()))?;
    let pf: ProjectFile = serde_json::from_str(&text).context("Niepoprawny plik projektu")?;
    if pf.format != FORMAT && pf.format != FORMAT_OLD {
        anyhow::bail!("To nie jest plik projektu Millwise");
    }
    let mut lib_changed = false;
    let mut remap: Vec<(u32, u32)> = Vec::new();
    for t in &pf.tools {
        let same = |a: &Tool| (a.diameter - t.diameter).abs() < 1e-9 && a.kind == t.kind;
        let id = if lib.get(t.id).is_some_and(same) {
            t.id
        } else if let Some(m) = lib.tools.iter().find(|a| a.number == t.number && same(a)) {
            m.id
        } else {
            lib_changed = true;
            lib.add(t.clone())
        };
        remap.push((t.id, id));
    }
    let mut ops = pf.operations;
    for op in &mut ops {
        if let Some(&(_, new)) = remap.iter().find(|(old, _)| *old == op.tool_id) {
            op.tool_id = new;
        }
    }
    let mut doc = Document {
        path: pf.source_file,
        project_path: Some(path.to_path_buf()),
        unit_name: pf.unit_name,
        layers: pf.layers.into_iter().map(|name| Layer { name, visible: true }).collect(),
        contours: pf.contours.into_iter().map(|c| DocContour::new(c.contour, c.layer)).collect(),
        stock: pf.stock,
        ops,
        post: pf.post,
        ..Default::default()
    };
    doc.finish_load();
    Ok((doc, lib_changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cam_ops::{CutParams, DrillParams, OpKind, PocketParams, ProfileParams, Tabs};

    /// Buduje projekt demonstracyjny z samples/test.dxf, zapisuje, wczytuje
    /// ponownie i porównuje. Zapisuje też samples/demo.millwise.
    #[test]
    fn project_roundtrip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut lib = ToolLibrary::with_defaults();
        let mut doc = Document::load_dxf(&root.join("samples/test.dxf")).unwrap();
        let layer = |doc: &Document, name: &str| -> Vec<usize> {
            (0..doc.contours.len()).filter(|&i| doc.layers[doc.contours[i].layer].name == name).collect()
        };
        let mk = |name: &str, kind: OpKind, tool: &Tool, depth: f64, contours: Vec<usize>| {
            let mut cut = CutParams { depth, ..Default::default() };
            cut.apply_tool(tool, cam_ops::DEFAULT_MATERIAL);
            Operation { name: name.into(), enabled: true, kind, tool_id: tool.id, cut, contours }
        };
        let by_no = |n: u32| lib.tools.iter().find(|t| t.number == n).unwrap().clone();
        let (t6, t3, drill) = (by_no(1), by_no(2), by_no(6));
        doc.add_op(mk("Wiercenie", OpKind::Drill(DrillParams::default()), &drill, 18.0, layer(&doc, "OTWORY")));
        doc.add_op(mk("Kieszeń", OpKind::Pocket(PocketParams::default()), &t6, 4.0, layer(&doc, "KIESZEN")));
        doc.add_op(mk(
            "Profil",
            OpKind::Profile(ProfileParams { tabs: Some(Tabs { count: 4, width: 6.0, height: 2.0 }), ..Default::default() }),
            &t3,
            18.0,
            layer(&doc, "OBRYS"),
        ));
        doc.selected_op = Some(1);
        doc.regenerate(&lib, None);
        let machine = cam_post::Machine::default();
        let g1 = doc.gcode(&lib, &machine).unwrap();

        let out = root.join("samples/demo.millwise");
        save_project(&doc, &lib, &out).unwrap();
        // drugi komputer: pusta biblioteka → narzędzia dodane z projektu
        let mut other = ToolLibrary::default();
        let (mut doc2, changed) = load_project(&out, &mut other).unwrap();
        assert!(changed);
        assert_eq!(other.tools.len(), 3);
        doc2.regenerate(&other, None);
        assert_eq!(doc2.ops.len(), 3);
        assert_eq!(doc2.contours.len(), doc.contours.len());
        assert_eq!(doc2.gcode(&other, &machine).unwrap(), g1);
        // ta sama biblioteka → bez zmian
        let (_, changed) = load_project(&out, &mut lib).unwrap();
        assert!(!changed);
    }
}
