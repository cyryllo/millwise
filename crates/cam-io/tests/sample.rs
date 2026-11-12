//! Wczytanie przykładowego pliku samples/test.dxf.
#[test]
fn loads_sample_dxf() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/test.dxf");
    let imp = cam_io::load_dxf(&path, &Default::default()).expect("load");
    let closed = imp.contours.iter().filter(|c| c.contour.closed).count();
    let open = imp.contours.len() - closed;
    for c in &imp.contours {
        eprintln!("{:8} closed={} verts={} len={:.2}", c.layer, c.contour.closed, c.contour.verts.len(), c.contour.length());
    }
    // obrys + 4 otwory + kieszeń + wyspa + fasolka = 8 zamkniętych, spline + łamana = 2 otwarte
    assert_eq!(closed, 8);
    assert_eq!(open, 2);
    assert!(imp.warnings.is_empty(), "{:?}", imp.warnings);
}
