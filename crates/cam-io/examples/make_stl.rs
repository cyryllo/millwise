//! Przykładowy model STL do testów: płyta z wzgórzem i pierścieniem
//! (siatka zamknięta: powierzchnia z góry, ściany i spód).
//! `cargo run -p cam-io --example make_stl` → samples/model.stl

use cam_io::Mesh;

fn height(x: f32, y: f32) -> f32 {
    let (cx, cy) = (40.0, 25.0);
    let r2 = (x - cx).powi(2) + (y - cy).powi(2);
    let hill = 14.0 * (-r2 / 180.0).exp();
    let r = r2.sqrt();
    let ring = 3.0 * (-((r - 18.0).powi(2)) / 4.0).exp();
    6.0 + hill + ring
}

fn main() {
    let (w, d, n, m) = (80.0f32, 50.0f32, 80usize, 50usize);
    let p = |i: usize, j: usize, top: bool| {
        let (x, y) = (w * i as f32 / n as f32, d * j as f32 / m as f32);
        [x, y, if top { height(x, y) } else { 0.0 }]
    };
    let mut tris = Vec::new();
    for i in 0..n {
        for j in 0..m {
            // góra (CCW z góry) i spód (odwrotnie)
            tris.push([p(i, j, true), p(i + 1, j, true), p(i + 1, j + 1, true)]);
            tris.push([p(i, j, true), p(i + 1, j + 1, true), p(i, j + 1, true)]);
            tris.push([p(i, j, false), p(i + 1, j + 1, false), p(i + 1, j, false)]);
            tris.push([p(i, j, false), p(i, j + 1, false), p(i + 1, j + 1, false)]);
        }
    }
    // ściany boczne
    let mut wall = |a: [usize; 2], b: [usize; 2]| {
        let (a0, a1, b0, b1) = (p(a[0], a[1], false), p(a[0], a[1], true), p(b[0], b[1], false), p(b[0], b[1], true));
        tris.push([a0, b0, b1]);
        tris.push([a0, b1, a1]);
    };
    for i in 0..n {
        wall([i, 0], [i + 1, 0]);
        wall([i + 1, m], [i, m]);
    }
    for j in 0..m {
        wall([n, j], [n, j + 1]);
        wall([0, j + 1], [0, j]);
    }
    let mesh = Mesh { tris };
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/model.stl");
    std::fs::write(&out, mesh.to_binary()).unwrap();
    println!("{}: {} trójkątów", out.display(), mesh.tris.len());
}
