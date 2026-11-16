//! Sprawdzanie G-code tak, jak robią to sterowniki: każdy łuk G2/G3 jest
//! liczony z zaokrąglonych liczb z programu, z regułami LinuxCNC 2.9
//! (`interp_arc.cc`) i GRBL 1.1 (`gcode.c`). Do testów i przed wysłaniem
//! programu na maszynę.

use cam_ops::toolpath::{arc_points, P3};
use cam_geom::Vec2;

/// Domyślna tolerancja LinuxCNC [mm]: `CENTER_ARC_RADIUS_TOLERANCE_MM` = 2 · 0,01 · √2.
pub const LINUXCNC_DEFAULT_TOL: f64 = 2.0 * 0.01 * std::f64::consts::SQRT_2;
/// Najmniejsza tolerancja, jaką da się ustawić w INI LinuxCNC [mm].
pub const LINUXCNC_STRICTEST_TOL: f64 = 0.001;

/// Problem z łukiem: numer wiersza (od 1) i opis.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcIssue {
    pub line: usize,
    pub message: String,
}

/// Ruch odczytany z programu (współrzędne robocze, mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    Rapid(P3),
    Line(P3),
    Arc { to: P3, center: Vec2, ccw: bool },
}

/// Ruchy programu po kolei (tak, jak wykona je sterownik). Linie z G53
/// (współrzędne maszyny — zmiana narzędzia) są pomijane.
pub fn parse(gcode: &str) -> Vec<(usize, Motion, bool)> {
    let mut out = Vec::new();
    let mut pos = P3::new(0.0, 0.0, 0.0);
    let mut motion: Option<u8> = None;
    for (n, raw) in gcode.lines().enumerate() {
        let line = strip_comments(raw);
        let words = words(&line);
        if words.is_empty() {
            continue;
        }
        let mut machine = false;
        let (mut x, mut y, mut z, mut i, mut j) = (None, None, None, None, None);
        for &(l, v) in &words {
            match l {
                'G' => {
                    let g = (v * 10.0).round() as i32;
                    match g {
                        0 | 10 | 20 | 30 => motion = Some((g / 10) as u8),
                        530 => machine = true,
                        800 => motion = None,
                        _ => {}
                    }
                }
                'X' => x = Some(v),
                'Y' => y = Some(v),
                'Z' => z = Some(v),
                'I' => i = Some(v),
                'J' => j = Some(v),
                _ => {}
            }
        }
        if machine {
            continue;
        }
        let has_axes = x.is_some() || y.is_some() || z.is_some();
        let has_xy = x.is_some() || y.is_some();
        let to = P3::new(x.unwrap_or(pos.x), y.unwrap_or(pos.y), z.unwrap_or(pos.z));
        let m = match motion {
            Some(0) if has_axes => Motion::Rapid(to),
            Some(1) if has_axes => Motion::Line(to),
            Some(c @ (2 | 3)) if has_axes || i.is_some() || j.is_some() => {
                let center = Vec2::new(pos.x + i.unwrap_or(0.0), pos.y + j.unwrap_or(0.0));
                Motion::Arc { to, center, ccw: c == 3 }
            }
            _ => continue,
        };
        out.push((n + 1, m, has_xy));
        pos = to;
    }
    out
}

/// Łuki, których sterownik nie przyjmie albo wykona inaczej niż zwykle się
/// oczekuje. `lcnc_tol` — tolerancja LinuxCNC (domyślna albo z INI).
pub fn check_arcs(gcode: &str, lcnc_tol: f64) -> Vec<ArcIssue> {
    let mut issues = Vec::new();
    let mut pos = P3::new(0.0, 0.0, 0.0);
    for (line, m, has_xy) in parse(gcode) {
        if let Motion::Arc { to, center, .. } = m {
            let r1 = pos.xy().distance(center);
            let r2 = to.xy().distance(center);
            let abs = (r1 - r2).abs();
            let rel = abs / r1.max(r2).max(1e-12);
            let mut add = |message: String| issues.push(ArcIssue { line, message });
            if !has_xy {
                add("brak X/Y: LinuxCNC wykona pełny okrąg, GRBL zgłosi błąd 26".into());
            }
            if r1 < 0.00127 || r2 < 0.00127 {
                add(format!("LinuxCNC: łuk o zerowym promieniu (r1 = {r1:.5}, r2 = {r2:.5})"));
            }
            if abs > lcnc_tol * 100.0 || (rel > 0.001 && abs > lcnc_tol) {
                add(format!("LinuxCNC: promień do końca różni się od promienia do początku o {abs:.5} mm"));
            }
            if abs > 0.5 || (abs > 0.005 && abs > 0.001 * r1) {
                add(format!("GRBL: błąd 33 — różnica promieni {abs:.5} mm"));
            }
        }
        pos = match m {
            Motion::Rapid(p) | Motion::Line(p) | Motion::Arc { to: p, .. } => p,
        };
    }
    issues
}

/// Droga narzędzia z programu jako punkty (łuki rozpisane jak w sterowniku) z
/// informacją, czy dojście do punktu to ruch szybki.
pub fn trace(gcode: &str) -> Vec<(P3, bool)> {
    let mut out = Vec::new();
    let mut pos = P3::new(0.0, 0.0, 0.0);
    for (_, m, _) in parse(gcode) {
        match m {
            Motion::Rapid(p) => out.push((p, true)),
            Motion::Line(p) => out.push((p, false)),
            Motion::Arc { to, center, ccw } => {
                // początek == koniec: pełny okrąg — tak jak w sterowniku
                out.extend(arc_points(pos, to, center, ccw, 0.002).into_iter().map(|p| (p, false)));
            }
        }
        pos = match m {
            Motion::Rapid(p) | Motion::Line(p) | Motion::Arc { to: p, .. } => p,
        };
    }
    out
}

fn strip_comments(line: &str) -> String {
    let line = line.split(';').next().unwrap_or("");
    let mut out = String::new();
    let mut depth = 0;
    for c in line.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Słowa „litera + liczba” (z odstępami albo bez).
fn words(line: &str) -> Vec<(char, f64)> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_alphabetic() {
            continue;
        }
        let mut num = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() || d == '.' || d == '-' || d == '+' || d == ' ' {
                if d != ' ' {
                    num.push(d);
                }
                chars.next();
                if d == ' ' && !num.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }
        if let Ok(v) = num.parse() {
            out.push((c.to_ascii_uppercase(), v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{emit, PostSettings, ProgramOp};
    use cam_ops::{Tool, Toolpath};

    #[test]
    fn reads_words_and_finds_bad_arcs() {
        assert_eq!(words("N10 G2X1.5 Y-2 I0.5J0 F1000"), vec![('N', 10.0), ('G', 2.0), ('X', 1.5), ('Y', -2.0), ('I', 0.5), ('J', 0.0), ('F', 1000.0)]);
        let g = "G0 X0 Y0 (start)\nG1 Z-1 F100\nG2 X10 Y0 I5 J0\nG3 I5 J0\nG2 X10 Y0.05 I5 J0\nG53 G0 Z0\n";
        let issues = check_arcs(g, LINUXCNC_DEFAULT_TOL);
        assert!(issues.iter().any(|i| i.line == 4 && i.message.contains("pełny okrąg")), "{issues:?}");
        // promień 5 do 5,00025 — mieści się; 0,05 w Y przy r 5: różnica 0,00025 → dobrze
        assert!(!issues.iter().any(|i| i.line == 5), "{issues:?}");
        // r 5 → 5,1: 0,1 mm i 2 % — oba sterowniki odrzucają
        let bad = "G0 X0 Y0\nG2 X10 Y1 I5 J0\n";
        let issues = check_arcs(bad, LINUXCNC_DEFAULT_TOL);
        assert!(issues.iter().any(|i| i.message.starts_with("LinuxCNC")) && issues.iter().any(|i| i.message.starts_with("GRBL")), "{issues:?}");
    }

    /// Prosty generator liczb (powtarzalny).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Łuki trudne dla zaokrąglania: promienie 0,05–3000 mm, kąty od 1e-6 rad do
    /// prawie 2π, w obie strony, środki w dowolnych miejscach, też śrubowe.
    fn nasty_arcs() -> Toolpath {
        let mut rnd = Lcg(7);
        let mut tp = Toolpath::default();
        tp.rapid(P3::new(0.0, 0.0, 5.0));
        let mut p = P3::new(0.0, 0.0, -1.0);
        tp.feed(p, 300.0);
        for k in 0..4000 {
            let r = 0.05 * (3000.0f64 / 0.05).powf(rnd.next());
            let sweep = match k % 4 {
                0 => 1e-6 * (1e6f64).powf(rnd.next()),     // bardzo małe
                1 => std::f64::consts::TAU - 1e-4 * rnd.next(), // prawie pełne
                _ => std::f64::consts::TAU * rnd.next(),
            };
            let ccw = rnd.next() < 0.5;
            let a0 = std::f64::consts::TAU * rnd.next();
            let c = Vec2::new(p.x - r * a0.cos(), p.y - r * a0.sin());
            let a1 = if ccw { a0 + sweep } else { a0 - sweep };
            let z = if k % 7 == 0 { p.z - 0.3 } else { p.z };
            let to = P3::new(c.x + r * a1.cos(), c.y + r * a1.sin(), z);
            tp.arc(to, c, ccw, 1000.0);
            p = to;
            // nie odjeżdżać za daleko
            if p.xy().length() > 500.0 {
                p = P3::new(0.0, 0.0, -1.0);
                tp.rapid(P3::new(p.x, p.y, 5.0));
                tp.feed(p, 300.0);
            }
        }
        tp
    }

    fn path_length(pts: &[(P3, bool)]) -> f64 {
        pts.windows(2).filter(|w| !w[1].1).map(|w| w[0].0.distance(w[1].0)).sum()
    }

    #[test]
    fn emitted_arcs_are_what_the_controller_runs() {
        let tool = Tool::default();
        let tp = nasty_arcs();
        let expected = path_length(&tp.polyline(0.002));
        for decimals in [2, 3, 4] {
            for dialect in [crate::Dialect::Grbl, crate::Dialect::LinuxCnc] {
                let s = PostSettings { decimals, dialect, ..Default::default() };
                let ops = [ProgramOp { name: "lukii", tool: &tool, rpm: 10000.0, safe_z: 5.0, toolpath: &tp }];
                let g = emit("t", &ops, &s);
                let issues = check_arcs(&g, LINUXCNC_STRICTEST_TOL);
                assert!(issues.is_empty(), "{decimals} miejsca, {dialect:?}: {:?}", &issues[..issues.len().min(5)]);
                // ta sama droga: żaden łuk nie zamienił się w (prawie) pełny okrąg
                let got = path_length(&trace(&g));
                assert!((got - expected).abs() < expected * 1e-3, "{decimals} miejsca: droga {got:.1} zamiast {expected:.1}");
                assert!(g.contains("G2") && g.contains("G3"), "łuki zostają łukami");
            }
        }
    }
}
