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
    for z in levels(cut, bottom, a, &grid, tool) {
        let limit = (z - a + 1e-4) as f32;
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
fn levels(cut: &CutParams, bottom: f64, a: f64, grid: &ModelHeights, tool: &Tool) -> Vec<f64> {
    let mut zs = z_levels(cut.top_z, bottom, cut.step_down);
    // płaskie: wysokość (co 0,01 mm) zajmująca co najmniej tyle, ile przekrój freza
    let min_cells = ((tool.diameter * tool.diameter) / (grid.cell * grid.cell)).max(4.0) as usize;
    // BTreeMap: stała kolejność (od najniższych) — te same poziomy przy każdym liczeniu
    let mut hist: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for &z in &grid.z {
        if z.is_finite() {
            *hist.entry((z as f64 * 100.0).round() as i64).or_default() += 1;
        }
    }
    for (k, n) in hist {
        let z = k as f64 / 100.0 + a;
        if n >= min_cells && z < cut.top_z - 1e-3 && z > bottom + 1e-3 && zs.iter().all(|q| (q - z).abs() > 0.05) {
            zs.push(z);
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
        let m = ModelInput { heights: &h, stock };
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
    fn roughing_inside_selected_boundary_and_without_model() {
        let h = stepped();
        let tool = Tool { diameter: 6.0, ..Default::default() };
        let stock = Aabb2 { min: Vec2::new(-5.0, -5.0), max: Vec2::new(45.0, 35.0) };
        let m = ModelInput { heights: &h, stock };
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
