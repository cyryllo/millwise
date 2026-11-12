//! samples/test.dxf → operacje → G-code; walidacja geometrii łuków i zakresów.

use cam_geom::Contour;
use cam_ops::*;
use cam_post::*;

fn load() -> Vec<(String, Contour)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/test.dxf");
    let imp = cam_io::load_dxf(&path, &Default::default()).unwrap();
    imp.contours.into_iter().map(|c| (c.layer, c.contour)).collect()
}

fn on_layer(all: &[(String, Contour)], layer: &str) -> Vec<Contour> {
    all.iter().filter(|(l, _)| l == layer).map(|(_, c)| c.clone()).collect()
}

/// Prosty interpreter: sprawdza, że w G2/G3 promień do startu i końca jest zgodny,
/// i zwraca minimalne Z.
fn validate(g: &str) -> f64 {
    let (mut x, mut y, mut z) = (f64::NAN, f64::NAN, f64::NAN);
    let mut min_z = f64::MAX;
    let mut modal = 0;
    for line in g.lines() {
        let code = line.split('(').next().unwrap().trim();
        if code.is_empty() || code == "%" {
            continue;
        }
        let mut words = std::collections::HashMap::new();
        let mut gs = Vec::new();
        for w in code.split_whitespace() {
            let (k, v) = w.split_at(1);
            let v: f64 = v.parse().unwrap_or_else(|_| panic!("zła liczba w '{line}'"));
            if k == "G" {
                gs.push(v as i32);
            } else {
                words.insert(k.to_string(), v);
            }
        }
        for gv in &gs {
            if [0, 1, 2, 3].contains(gv) {
                modal = *gv;
            }
        }
        let has_xyz = words.contains_key("X") || words.contains_key("Y") || words.contains_key("Z");
        if !has_xyz || gs.contains(&43) {
            continue;
        }
        let nx = *words.get("X").unwrap_or(&x);
        let ny = *words.get("Y").unwrap_or(&y);
        let nz = *words.get("Z").unwrap_or(&z);
        if modal == 2 || modal == 3 {
            let (i, j) = (words["I"], words["J"]);
            let (cx, cy) = (x + i, y + j);
            let r0 = (i * i + j * j).sqrt();
            let r1 = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
            assert!((r0 - r1).abs() < 0.002, "niespójny łuk: '{line}' r0={r0} r1={r1}");
        }
        if modal != 0 {
            assert!(words.contains_key("F") || g.contains(" F"), "posuw bez F");
        }
        x = nx;
        y = ny;
        z = nz;
        if modal != 0 {
            min_z = min_z.min(z);
        }
    }
    min_z
}

#[test]
fn full_program_both_dialects() {
    let all = load();
    let lib = ToolLibrary::with_defaults();
    let by_no = |n: u32| lib.tools.iter().find(|t| t.number == n).unwrap();
    let t6 = by_no(1);
    let t3 = by_no(2);
    let vbit = by_no(5);
    let drill = by_no(6);

    let mk = |name: &str, kind: OpKind, tool: &Tool, depth: f64| {
        let mut cut = CutParams { depth, ..Default::default() };
        cut.apply_tool(tool, DEFAULT_MATERIAL);
        Operation { name: name.into(), enabled: true, kind, tool_id: tool.id, cut, contours: vec![] }
    };

    let ops = [
        (mk("Wiercenie", OpKind::Drill(DrillParams::default()), drill, 18.0), on_layer(&all, "OTWORY")),
        (mk("Kieszeń", OpKind::Pocket(PocketParams::default()), t6, 4.0), on_layer(&all, "KIESZEN")),
        (mk("Grawer", OpKind::Engrave(EngraveParams::default()), vbit, 0.5), on_layer(&all, "GRAWER")),
        (
            mk(
                "Profil",
                OpKind::Profile(ProfileParams { tabs: Some(Tabs { count: 4, width: 6.0, height: 2.0 }), ..Default::default() }),
                t3,
                18.0,
            ),
            on_layer(&all, "OBRYS"),
        ),
    ];

    let results: Vec<OpResult> = ops
        .iter()
        .map(|(op, geom)| {
            let tool = lib.get(op.tool_id).unwrap();
            let r = generate(op, tool, geom);
            assert!(!r.toolpath.moves.is_empty(), "{}: pusta ścieżka", op.name);
            eprintln!("{}: {} ruchów, {:?}", op.name, r.toolpath.moves.len(), r.warnings);
            r
        })
        .collect();

    // kieszeń z wyspą: 2 zamknięte kontury + fasolka → bez ostrzeżeń
    assert!(results[1].warnings.is_empty(), "{:?}", results[1].warnings);

    let prog: Vec<ProgramOp> = ops
        .iter()
        .zip(&results)
        .map(|((op, _), r)| ProgramOp {
            name: &op.name,
            tool: lib.get(op.tool_id).unwrap(),
            rpm: op.cut.rpm,
            safe_z: op.cut.safe_z,
            toolpath: &r.toolpath,
        })
        .collect();

    for dialect in [Dialect::Grbl, Dialect::LinuxCnc] {
        let g = emit("test", &prog, &PostSettings { dialect, ..Default::default() });
        let min_z = validate(&g);
        assert!((min_z + 18.0).abs() < 1e-6, "min Z = {min_z}");
        assert!(g.contains("G2") || g.contains("G3"));
        assert!(g.is_ascii());
        let out = std::env::temp_dir().join(format!("millwise_test.{}", dialect.extension()));
        std::fs::write(&out, &g).unwrap();
        eprintln!("{dialect:?}: {} linii → {}", g.lines().count(), out.display());
    }
}
