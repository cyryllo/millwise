//! Obróbka zgrubna 3D: zbieranie materiału warstwami Z. Na każdej wysokości
//! obszar do wybrania = granica (wybrane krzywe albo cały półfabrykat) bez
//! modelu wystającego ponad tę wysokość (z naddatkiem) — i ten obszar jest
//! wybierany jak kieszeń (te same strategie, wejścia i łączenia).

use crate::model::ModelHeights;
use crate::operation::{CutParams, OpResult, PocketParams, Rough3DParams};
use crate::path::z_levels;
use crate::tool::Tool;
use cam_geom::{Aabb2, Contour};
use cam_i18n::tr;

/// Model 3D dla operacji: mapa wysokości i zarys półfabrykatu z góry.
pub struct ModelInput<'a> {
    pub heights: &'a ModelHeights,
    pub stock: Aabb2,
    /// Narzędzie odniesienia dla obróbki resztek (z biblioteki).
    pub ref_tool: Option<Tool>,
}

pub(crate) fn generate(cut: &CutParams, p: &Rough3DParams, tool: &Tool, contours: &[Contour], m: &ModelInput, res: &mut OpResult) {
    let r = tool.radius();
    let a = p.allowance.max(0.0);
    let closed: Vec<Contour> = contours.iter().filter(|c| c.closed && c.verts.len() >= 3).cloned().collect();
    if closed.len() < contours.len() {
        res.warnings.push(tr!("Pominięto krzywe otwarte — granicą obróbki są tylko zamknięte").into());
    }
    // granica: wybrane krzywe albo półfabrykat poszerzony o średnicę — środek freza
    // dochodzi wtedy do krawędzi półfabrykatu i mieści się w wąskim pasie przy modelu
    let area = if closed.is_empty() {
        m.stock.expanded(2.0 * r)
    } else {
        closed.iter().map(|c| c.bbox()).reduce(|x, y| x.union(&y)).unwrap()
    };
    let grid = m.heights.padded(area.expanded(m.heights.cell * 2.0));
    // model pogrubiony o naddatek i zapas na dokładność rastra i upraszczania obrysów
    let tol = grid.cell.max(0.02);
    let grown = grid.dilated(a + grid.cell + tol);
    let inside = if closed.is_empty() {
        let mut v = vec![false; grid.nx * grid.ny];
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let c = grid.min + cam_geom::Vec2::new((i as f64 + 0.5) * grid.cell, (j as f64 + 0.5) * grid.cell);
                v[j * grid.nx + i] = area.contains_point(c);
            }
        }
        v
    } else {
        grid.inside_mask(&closed)
    };
    let pp = PocketParams {
        strategy: p.strategy,
        zigzag_angle: 0.0,
        stepover: p.stepover,
        direction: p.direction,
        entry: p.entry,
        stock_to_leave: 0.0,
        finish: None,
        corners: false,
        ..Default::default()
    };
    let bottom = cut.bottom_z();
    if bottom < m.heights.z_min - 1e-6 {
        res.warnings.push(tr!("Głębokość sięga poniżej modelu ({:.2} mm) — obrabiane jest też tło wokół niego", m.heights.z_min));
    }
    let mut prev = cut.top_z;
    let mut empty = 0;
    // wysepki niższe niż ¼ naddatku (najwyżej 0,1 mm) nie są omijane — frez zbiera
    // tyle z naddatku; bez tego płaskorzeźba (odcienie szarości co setne milimetra)
    // daje na warstwie tysiące maleńkich wysp
    let island_tol = (0.25 * a).min(0.1);
    for z in levels(cut, bottom, a, &grid, tool) {
        let limit = (z - a + island_tol + 1e-4) as f32;
        let region: Vec<Contour> = grid.outline_where_tol(|k, _| inside[k] && !(grown.z[k] > limit), tol);
        if region.is_empty() {
            // niżej model zajmuje jeszcze więcej — koniec
            empty += 1;
            break;
        }
        let lc = CutParams { top_z: prev, depth: prev - z, step_down: prev - z + 1.0, ..cut.clone() };
        let mut lr = OpResult { toolpath: std::mem::take(&mut res.toolpath), warnings: Vec::new() };
        crate::pocket::generate(&lc, &pp, tool, &region, &mut lr);
        res.toolpath = lr.toolpath;
        prev = z;
    }
    if empty > 0 && res.toolpath.moves.is_empty() {
        res.warnings.push(tr!("Nie ma czego wybierać: model zajmuje cały obszar obróbki").into());
    }
}

/// Poziomy Z: równe kroki od góry do dna i dodatkowo tuż nad płaskimi powierzchniami
/// modelu (wysokość + naddatek), żeby na półkach nie zostawał cały krok materiału.
///
/// Dodatkowy poziom powstaje tylko wtedy, gdy zbiera z półki co najmniej `min_gain`
/// (¼ kroku, najmniej 0,3 mm) — płaskorzeźba ze zdjęcia ma setki „półek” co jeden
/// odcień szarości (kilka setnych milimetra), a każda zbędna warstwa to objazd
/// całego obszaru. Zwykły poziom tuż pod dodatkowym jest pomijany, gdy krok niżej
/// wciąż się mieści.
fn levels(cut: &CutParams, bottom: f64, a: f64, grid: &ModelHeights, tool: &Tool) -> Vec<f64> {
    let step = cut.step_down.max(0.01);
    let uniform = z_levels(cut.top_z, bottom, step);
    let min_gain = (0.25 * step).max(0.3);
    // płaskie: komórki bez spadku do sąsiadów (do ok. 1°), zgrupowane co 0,01 mm;
    // wysokość jest półką, gdy zajmuje co najmniej tyle, ile przekrój freza
    let min_cells = ((tool.diameter * tool.diameter) / (grid.cell * grid.cell)).max(4.0) as usize;
    let flat_tol = (grid.cell * 0.02).max(0.002) as f32;
    // BTreeMap: stała kolejność — te same poziomy przy każdym liczeniu
    let mut hist: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    let (nx, ny) = (grid.nx, grid.ny);
    for j in 0..ny {
        for i in 0..nx {
            let z = grid.z[j * nx + i];
            if !z.is_finite() {
                continue;
            }
            let flat = [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)]
                .iter()
                .filter(|&&(x, y)| x < nx && y < ny)
                .all(|&(x, y)| {
                    let n = grid.z[y * nx + x];
                    !n.is_finite() || (n - z).abs() <= flat_tol
                });
            if flat {
                *hist.entry((z as f64 * 100.0).round() as i64).or_default() += 1;
            }
        }
    }
    let mut extra: Vec<f64> = Vec::new();
    // od najwyższych półek: zysk = odległość do najbliższego poziomu nad półką
    for (k, n) in hist.into_iter().rev() {
        let z = k as f64 / 100.0 + a;
        if n < min_cells || z >= cut.top_z - 1e-3 || z <= bottom + 1e-3 {
            continue;
        }
        let above = uniform.iter().chain(&extra).copied().chain(std::iter::once(cut.top_z)).filter(|&q| q >= z + 1e-6).fold(f64::INFINITY, f64::min);
        if above - z > min_gain {
            extra.push(z);
        }
    }
    let mut zs: Vec<f64> = extra.clone();
    for &u in &uniform {
        // zwykły poziom tuż pod dodatkowym — zbędny, jeśli następny niżej jest w zasięgu kroku
        let over = extra.iter().copied().filter(|&e| e >= u && e - u < min_gain).fold(f64::NAN, f64::max);
        if !over.is_nan() && (u - bottom).abs() > 1e-6 {
            let below = uniform.iter().chain(&extra).copied().filter(|&q| q < u - 1e-6).fold(f64::NEG_INFINITY, f64::max);
            if over - below.max(bottom) <= step + 1e-6 {
                continue;
            }
        }
        if zs.iter().all(|q| (q - u).abs() > 1e-6) {
            zs.push(u);
        }
    }
    zs.sort_by(|x, y| y.partial_cmp(x).unwrap());
    zs
}

#[cfg(test)]
mod tests {
    use crate::model::{ModelHeights, ToolProfile};
    use crate::operation::*;
    use crate::tool::Tool;
    use crate::toolpath::Move;
    use crate::ModelInput;
    use cam_geom::{Aabb2, Vec2};

    /// Płyta 40 × 30 z wierzchem na Z −5 i kostka 10 × 10 do Z 0 na środku.
    fn stepped() -> ModelHeights {
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32, z: f32| {
            [[[x0, y0, z], [x1, y0, z], [x1, y1, z]], [[x0, y0, z], [x1, y1, z], [x0, y1, z]]]
        };
        let mut t = Vec::new();
        t.extend(quad(0.0, 0.0, 40.0, 30.0, -5.0));
        t.extend(quad(15.0, 10.0, 25.0, 20.0, 0.0));
        t.extend(quad(0.0, 0.0, 40.0, 30.0, -10.0)); // dno (pod płytą — niewidoczne z góry)
        ModelHeights::from_triangles(&t, 0.1).unwrap()
    }

    fn rough_op(allowance: f64) -> Operation {
        Operation {
            name: "z".into(),
            side: Side::A,
            enabled: true,
            kind: OpKind::Rough3D(Rough3DParams { allowance, ..Default::default() }),
            tool_id: 1,
            cut: CutParams { top_z: 0.0, depth: 10.0, step_down: 3.0, ..Default::default() },
            contours: vec![],
        }
    }

    /// Punkty ruchów roboczych (gęsto, co 0,2 mm).
    fn feed_points(tp: &crate::Toolpath) -> Vec<(Vec2, f64)> {
        tp.polyline(0.01)
            .windows(2)
            .filter(|w| !w[1].1)
            .flat_map(|w| {
                let (a, b) = (w[0].0, w[1].0);
                let n = (a.distance(b) / 0.2).ceil().max(1.0) as usize;
                (0..=n).map(move |k| {
                    let t = k as f64 / n as f64;
                    (a.xy().lerp(b.xy(), t), a.z + (b.z - a.z) * t)
                })
            })
            .collect()
    }

    #[test]
    fn roughing_clears_around_the_model_without_touching_it() {
        let h = stepped();
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let stock = Aabb2 { min: Vec2::new(-5.0, -5.0), max: Vec2::new(45.0, 35.0) };
        let m = ModelInput { heights: &h, stock, ref_tool: None };
        let a = 0.5;
        let res = generate_with_model(&rough_op(a), &tool, &[], Some(&m));
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
        let pts = feed_points(&res.toolpath);
        assert!(pts.len() > 100);
        let prof = ToolProfile::of(&tool);
        // nigdzie nie wcina się w model ani w naddatek (z dokładnością oczka rastra)
        for &(p, z) in &pts {
            if let Some(zm) = h.drop_tool(p, &prof, a - h.cell) {
                assert!(z >= zm - 1e-6, "wcięcie w ({:.2}, {:.2}): z = {z:.3}, dopuszczalne {zm:.3}", p.x, p.y);
            }
        }
        // półka na Z −5: wybrana do −5 + naddatek (dodatkowy poziom), a obok płyty — do dna
        let near = |x: f64, y: f64, zz: f64| pts.iter().any(|(p, z)| p.distance(Vec2::new(x, y)) < 1.5 && (z - zz).abs() < 1e-6);
        assert!(near(5.0, 5.0, -4.5), "półka na −4,5");
        assert!(near(-5.0, 15.0, -10.0), "obok płyty do dna");
        // bez zbędnego zejścia nad kostką
        assert!(!pts.iter().any(|(p, z)| p.distance(Vec2::new(20.0, 15.0)) < 5.0 && *z < 0.0));
        // tylko ruchy liniowe lub łuki, kończy się wycofaniem
        assert!(matches!(res.toolpath.moves.last(), Some(Move::Rapid(_))));
    }

    #[test]
    fn photo_terraces_do_not_multiply_levels() {
        // łagodny spadek z Z −1 do −5 schodkami co 0,02 mm (jak odcienie szarości
        // płaskorzeźby): każdy schodek 0,5 mm szeroki (większy niż przekrój freza 3 mm)
        let mut t = Vec::new();
        let n = 200;
        for k in 0..n {
            let (x0, x1) = (k as f32 * 0.5, (k + 1) as f32 * 0.5);
            let z = -1.0 - 4.0 * k as f32 / n as f32;
            t.push([[x0, 0.0, z], [x1, 0.0, z], [x1, 30.0, z]]);
            t.push([[x0, 0.0, z], [x1, 30.0, z], [x0, 30.0, z]]);
        }
        let h = ModelHeights::from_triangles(&t, 0.05).unwrap();
        let tool = Tool { diameter: 3.0, ..Default::default() };
        let cut = CutParams { top_z: 0.0, depth: 6.0, step_down: 2.0, ..Default::default() };
        let zs = super::levels(&cut, cut.bottom_z(), 0.5, &h, &tool);
        // równe kroki 0 → −6 to 3 poziomy; dodatkowe tylko co ≥ 0,5 mm
        assert!(zs.len() <= 3 + 6, "{zs:?}");
        for w in zs.windows(2) {
            assert!(w[0] - w[1] <= 2.0 + 1e-9, "krok większy niż zadany: {zs:?}");
        }
        // a prawdziwa półka (kostka) dalej dostaje swój poziom
        let shelf = super::levels(&rough_op(0.5).cut, -10.0, 0.5, &stepped(), &Tool { diameter: 6.0, ..Default::default() });
        assert!(shelf.iter().any(|z| (z + 4.5).abs() < 1e-6), "{shelf:?}");
    }

    #[test]
    fn roughing_inside_selected_boundary_and_without_model() {
        let h = stepped();
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let stock = Aabb2 { min: Vec2::new(-5.0, -5.0), max: Vec2::new(45.0, 35.0) };
        let m = ModelInput { heights: &h, stock, ref_tool: None };
        let mut b = cam_geom::Contour::new(true);
        b.verts = [(0.0, 0.0), (20.0, 0.0), (20.0, 30.0), (0.0, 30.0)].iter().map(|&(x, y)| cam_geom::Vertex::new(x, y, 0.0)).collect();
        let res = generate_with_model(&rough_op(0.5), &tool, &[b], Some(&m));
        for (p, _) in feed_points(&res.toolpath) {
            assert!(p.x > 2.9 && p.x < 17.1 && p.y > 2.9 && p.y < 27.1, "poza granicą: {p:?}");
        }
        let none = generate_with_model(&rough_op(0.5), &tool, &[], None);
        assert!(!none.warnings.is_empty() && none.toolpath.moves.is_empty());
    }
}
