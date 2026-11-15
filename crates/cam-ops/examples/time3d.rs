//! Pomiar czasu obróbki 3D na pliku STL: `cargo run --release -p cam-ops --example time3d -- plik.stl`
use cam_ops::model::ModelHeights;
use cam_ops::*;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap_or("samples/model.stl".into());
    let mesh = cam_io::load_stl(std::path::Path::new(&path)).expect("STL");
    let (lo, hi) = mesh.bounds().unwrap();
    // góra modelu na Z 0
    let tris: Vec<[[f32; 3]; 3]> = mesh.tris.iter().map(|t| t.map(|p| [p[0] - lo[0], p[1] - lo[1], p[2] - hi[2]])).collect();
    let size = [(hi[0] - lo[0]) as f64, (hi[1] - lo[1]) as f64, (hi[2] - lo[2]) as f64];
    let t = Instant::now();
    let cell = (size[0].max(size[1]) / 2000.0).clamp(0.05, 0.5);
    let h = ModelHeights::from_triangles(&tris, cell).unwrap();
    println!("mapa {}×{} oczko {cell:.3}: {:?}", h.nx, h.ny, t.elapsed());
    let stock = cam_geom::Aabb2 { min: cam_geom::Vec2::new(-5.0, -5.0), max: cam_geom::Vec2::new(size[0] + 5.0, size[1] + 5.0) };
    let tool = Tool { diameter: 6.0, ..Default::default() };
    let op = Operation {
        name: "z".into(),
        side: Side::A,
        enabled: true,
        kind: OpKind::Rough3D(Rough3DParams::default()),
        tool_id: 1,
        cut: CutParams { depth: size[2], step_down: 2.0, ..Default::default() },
        contours: vec![],
    };
    let t = Instant::now();
    let r = generate_with_model(&op, &tool, &[], Some(&ModelInput { heights: &h, stock }));
    println!("zgrubna: {} ruchów, {:?}, {:?}", r.toolpath.moves.len(), t.elapsed(), r.warnings);
    for (name, kind) in [("płaski", ToolKind::FlatEnd), ("kulowy", ToolKind::BallEnd)] {
        let tool = Tool { diameter: 6.0, kind, ..Default::default() };
        let op = Operation { kind: OpKind::Finish3D(Finish3DParams::default()), ..op.clone() };
        let t = Instant::now();
        let r = generate_with_model(&op, &tool, &[], Some(&ModelInput { heights: &h, stock }));
        println!("wykańczająca ({name}): {} ruchów, {:?}, {:?}", r.toolpath.moves.len(), t.elapsed(), r.warnings);
        let op = Operation {
            kind: OpKind::Finish3D(Finish3DParams { strategy: Finish3DStrategy::Waterline, ..Default::default() }),
            ..op.clone()
        };
        let t = Instant::now();
        let r = generate_with_model(&op, &tool, &[], Some(&ModelInput { heights: &h, stock }));
        println!("warstwice ({name}): {} ruchów, {:?}, {:?}", r.toolpath.moves.len(), t.elapsed(), r.warnings);
    }
}
