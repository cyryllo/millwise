//! Grawer / prowadzenie po linii: środek narzędzia na linii albo narzędzie
//! z lewej / prawej strony linii (offset o promień).

use crate::operation::{order_inside_first, start_near, CutParams, EngraveParams, EngraveSide, OpResult};
use crate::path::z_levels;
use crate::profile::{cut_closed, cut_open};
use crate::tool::Tool;
use cam_geom::Contour;
use cavalier_contours::polyline::PlineSource;

pub(crate) fn generate(cut: &CutParams, p: &EngraveParams, tool: &Tool, contours: &[Contour], res: &mut OpResult) {
    let levels = z_levels(cut.top_z, cut.bottom_z(), cut.step_down);
    for i in order_inside_first(contours) {
        let c = &contours[i];
        let paths: Vec<Contour> = match p.side {
            EngraveSide::Center => vec![c.clone()],
            side => {
                // cavalier: dodatni offset = w lewo od kierunku krzywej
                let r = tool.radius();
                let d = if side == EngraveSide::Left { r } else { -r };
                let out: Vec<Contour> = c.to_cavc().parallel_offset(d).iter().map(Contour::from_cavc).collect();
                if out.is_empty() {
                    res.warnings.push(format!("Krzywa #{}: brak miejsca na narzędzie po tej stronie", i + 1));
                }
                out
            }
        };
        for mut path in paths {
            let pos = res.toolpath.pos().map_or(cam_geom::Vec2::ZERO, |q| q.xy());
            start_near(&mut path, pos, false);
            if path.closed {
                cut_closed(&mut res.toolpath, &path, &levels, cut, p.entry, None, &mut res.warnings);
            } else {
                cut_open(&mut res.toolpath, &path, &levels, cut);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::operation::*;
    use crate::tool::Tool;
    use cam_geom::{Contour, Vertex};

    fn line_op(side: EngraveSide) -> Operation {
        Operation {
            name: "g".into(),
            enabled: true,
            kind: OpKind::Engrave(EngraveParams { side, ..Default::default() }),
            tool_id: 1,
            cut: CutParams { depth: 1.0, step_down: 1.0, ..Default::default() },
            contours: vec![0],
        }
    }

    #[test]
    fn left_and_right_offset_open_line() {
        let tool = Tool { diameter: 4.0, ..Default::default() };
        let line = Contour { verts: vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(50.0, 0.0, 0.0)], closed: false };
        for (side, y) in [(EngraveSide::Left, 2.0), (EngraveSide::Right, -2.0), (EngraveSide::Center, 0.0)] {
            let res = generate(&line_op(side), &tool, std::slice::from_ref(&line));
            let cutting: Vec<_> = res.toolpath.polyline(0.01).into_iter().filter(|(p, r)| !r && p.z < -0.5).collect();
            assert!(!cutting.is_empty());
            assert!(cutting.iter().all(|(p, _)| (p.y - y).abs() < 1e-6), "{side:?}: {:?}", cutting[0]);
        }
    }
}
