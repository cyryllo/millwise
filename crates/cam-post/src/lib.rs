//! Postprocesor G-code: GRBL / FluidNC oraz LinuxCNC, profile maszyn.

pub mod machine;
pub use machine::{Controller, HomeCorner, Machine, MachineKind, MachineLibrary, SpindleKind};

use cam_ops::toolpath::{arc_points, Move, Toolpath, P3};
use cam_ops::Tool;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dialect {
    /// GRBL 1.1 i FluidNC: brak M6 — zmiana narzędzia przez pauzę M0.
    Grbl,
    /// LinuxCNC: T M6 + G43, tryb ścieżki G64.
    LinuxCnc,
}

impl Dialect {
    pub fn label(&self) -> &'static str {
        match self {
            Dialect::Grbl => "GRBL / FluidNC",
            Dialect::LinuxCnc => "LinuxCNC",
        }
    }
    pub fn extension(&self) -> &'static str {
        match self {
            Dialect::Grbl => "nc",
            Dialect::LinuxCnc => "ngc",
        }
    }
}

/// Sposób zmiany narzędzia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ToolChange {
    /// Pauza M0 i ręczna wymiana (GRBL nie obsługuje M6).
    #[default]
    ManualPause,
    /// T# M6 (FluidNC z ATC, LinuxCNC).
    Automatic,
    /// Osobny plik dla każdego narzędzia.
    SeparateFiles,
}

impl ToolChange {
    pub fn label(&self) -> &'static str {
        match self {
            ToolChange::ManualPause => "Ręczna (pauza M0)",
            ToolChange::Automatic => "Automatyczna (M6)",
            ToolChange::SeparateFiles => "Osobne pliki",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PostSettings {
    pub dialect: Dialect,
    /// Liczba miejsc po przecinku dla współrzędnych.
    pub decimals: usize,
    /// G2/G3 (true) czy łuki zamieniane na odcinki.
    pub arcs: bool,
    pub line_numbers: bool,
    pub comments: bool,
    /// Pauza na rozpędzenie wrzeciona [s] (0 = brak).
    pub spindle_delay: f64,
    /// Tolerancja ścieżki G64 P… dla LinuxCNC [mm].
    pub linuxcnc_path_tolerance: f64,
    /// Układ współrzędnych (G54…G59).
    pub wcs: String,
    pub tool_change: ToolChange,
    /// Pozycja zmiany narzędzia we współrzędnych maszyny (G53) X, Y, Z.
    pub tool_change_pos: Option<[f64; 3]>,
    pub coolant_mist: bool,
    pub coolant_flood: bool,
    /// Ograniczenie obrotów (0 = bez ograniczeń).
    pub rpm_min: f64,
    pub rpm_max: f64,
    /// Własny początek programu (puste = domyślny).
    pub header: String,
    /// Własny koniec programu (puste = domyślny).
    pub footer: String,
}

impl Default for PostSettings {
    fn default() -> Self {
        Self {
            dialect: Dialect::Grbl,
            decimals: 3,
            arcs: true,
            line_numbers: false,
            comments: true,
            spindle_delay: 2.0,
            linuxcnc_path_tolerance: 0.01,
            wcs: "G54".into(),
            tool_change: ToolChange::ManualPause,
            tool_change_pos: None,
            coolant_mist: false,
            coolant_flood: false,
            rpm_min: 0.0,
            rpm_max: 0.0,
            header: String::new(),
            footer: String::new(),
        }
    }
}

/// Jedna operacja programu.
pub struct ProgramOp<'a> {
    pub name: &'a str,
    pub tool: &'a Tool,
    pub rpm: f64,
    pub safe_z: f64,
    pub toolpath: &'a Toolpath,
}

struct Writer<'s> {
    out: String,
    s: &'s PostSettings,
    line_no: u32,
    pos: Option<P3>,
    motion: Option<u8>,
    feed: Option<f64>,
}

impl<'s> Writer<'s> {
    fn line(&mut self, text: &str) {
        if self.s.line_numbers && !text.starts_with('%') {
            self.line_no += 10;
            let _ = write!(self.out, "N{} ", self.line_no);
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn comment(&mut self, text: &str) {
        if !self.s.comments {
            return;
        }
        let c = format!("({})", ascii_comment(text));
        self.line(&c);
    }

    fn num(&self, v: f64) -> String {
        fmt_num(v, self.s.decimals)
    }

    fn axes(&self, to: P3) -> String {
        let mut t = String::new();
        let changed = |a: f64, b: Option<f64>| b.is_none_or(|b| self.num(a) != self.num(b));
        if changed(to.x, self.pos.map(|p| p.x)) {
            let _ = write!(t, " X{}", self.num(to.x));
        }
        if changed(to.y, self.pos.map(|p| p.y)) {
            let _ = write!(t, " Y{}", self.num(to.y));
        }
        if changed(to.z, self.pos.map(|p| p.z)) {
            let _ = write!(t, " Z{}", self.num(to.z));
        }
        t
    }

    fn feed_word(&mut self, f: f64) -> String {
        let f = f.max(1.0);
        if self.feed.is_some_and(|c| (c - f).abs() < 1e-6) {
            String::new()
        } else {
            self.feed = Some(f);
            format!(" F{}", fmt_num(f, 1))
        }
    }

    fn rapid(&mut self, to: P3) {
        let axes = self.axes(to);
        if axes.is_empty() {
            return;
        }
        let g = if self.motion == Some(0) { String::new() } else { "G0".into() };
        self.motion = Some(0);
        let l = format!("{g}{axes}");
        self.line(l.trim_start());
        self.pos = Some(to);
    }

    fn linear(&mut self, to: P3, f: f64) {
        let axes = self.axes(to);
        if axes.is_empty() {
            return;
        }
        let g = if self.motion == Some(1) { String::new() } else { "G1".into() };
        self.motion = Some(1);
        let fw = self.feed_word(f);
        let l = format!("{g}{axes}{fw}");
        self.line(l.trim_start());
        self.pos = Some(to);
    }

    fn arc(&mut self, to: P3, center: cam_geom::Vec2, ccw: bool, f: f64) {
        let Some(from) = self.pos else {
            self.linear(to, f);
            return;
        };
        let r = from.xy().distance(center);
        // bardzo małe lub ogromne łuki → odcinki (unika błędów promienia w sterowniku)
        if !self.s.arcs || r < 0.05 || r > 5000.0 {
            for p in arc_points(from, to, center, ccw, 0.005) {
                self.linear(p, f);
            }
            return;
        }
        let code = if ccw { 3 } else { 2 };
        self.motion = Some(code);
        let axes = self.axes(to);
        let i = center.x - from.x;
        let j = center.y - from.y;
        let fw = self.feed_word(f);
        let l = format!("G{code}{axes} I{} J{}{fw}", self.num(i), self.num(j));
        self.line(&l);
        self.pos = Some(to);
    }
}

/// Generuje kompletny program G-code.
pub fn emit(title: &str, ops: &[ProgramOp], s: &PostSettings) -> String {
    let mut w = Writer { out: String::new(), s, line_no: 0, pos: None, motion: None, feed: None };
    let lcnc = s.dialect == Dialect::LinuxCnc;
    let auto_tc = lcnc || s.tool_change == ToolChange::Automatic;
    let coolant = s.coolant_mist || s.coolant_flood;

    if lcnc {
        w.line("%");
    }
    w.comment(&format!("Millwise - {title}"));
    w.comment(&format!("Sterownik: {}", s.dialect.label()));
    let mut tools: Vec<&Tool> = Vec::new();
    for op in ops {
        if !tools.iter().any(|t| t.id == op.tool.id) {
            tools.push(op.tool);
        }
    }
    for t in &tools {
        w.comment(&format!("T{} {} D={} {}", t.number, t.name, fmt_num(t.diameter, 3), t.kind.label()));
    }
    if !s.header.trim().is_empty() {
        for l in s.header.lines().map(str::trim).filter(|l| !l.is_empty()) {
            w.line(l);
        }
    } else if lcnc {
        w.line(&format!("G17 G21 G40 G49 {} G80 G90 G94", s.wcs));
        w.line(&format!("G64 P{}", fmt_num(s.linuxcnc_path_tolerance, 4)));
    } else {
        w.line("G17 G21 G90 G94");
        w.line(&s.wcs);
    }

    let max_safe = ops.iter().map(|o| o.safe_z).fold(5.0, f64::max);
    let mut cur_tool: Option<u32> = None;
    let mut cur_rpm: Option<f64> = None;
    for op in ops {
        w.comment(&format!("Operacja: {}", op.name));
        if cur_tool != Some(op.tool.id) {
            let first = cur_tool.is_none();
            if !first {
                w.line("M5");
                if coolant {
                    w.line("M9");
                }
                cur_rpm = None;
                if let Some(p) = w.pos {
                    w.rapid(P3 { z: max_safe.max(p.z), ..p });
                }
                if let Some([x, y, z]) = s.tool_change_pos {
                    w.line(&format!("G53 G0 Z{}", w.num(z)));
                    w.line(&format!("G53 G0 X{} Y{}", w.num(x), w.num(y)));
                }
            }
            if auto_tc {
                w.line(&format!("T{} M6", op.tool.number));
                if lcnc {
                    w.line(&format!("G43 H{}", op.tool.number));
                }
            } else if first {
                w.comment(&format!("Zaloz narzedzie T{}: {}", op.tool.number, op.tool.name));
            } else {
                w.comment(&format!("Zmien narzedzie na T{}: {}", op.tool.number, op.tool.name));
                w.line("M0");
            }
            // po zmianie narzędzia pozycja jest nieznana
            if !first {
                w.pos = None;
                w.motion = None;
            }
            cur_tool = Some(op.tool.id);
        }
        let mut rpm = op.rpm;
        if s.rpm_max > 0.0 {
            rpm = rpm.min(s.rpm_max);
        }
        if s.rpm_min > 0.0 && rpm > 0.0 {
            rpm = rpm.max(s.rpm_min);
        }
        if cur_rpm.is_none_or(|r| (r - rpm).abs() > 0.5) {
            w.line(&format!("M3 S{}", fmt_num(rpm, 0)));
            if s.spindle_delay > 0.0 {
                w.line(&format!("G4 P{}", fmt_num(s.spindle_delay, 1)));
            }
            if s.coolant_mist {
                w.line("M7");
            }
            if s.coolant_flood {
                w.line("M8");
            }
            cur_rpm = Some(rpm);
        }
        // pierwszy ruch: najpierw Z na wysokość bezpieczną
        if w.pos.is_none() {
            w.line(&format!("G0 Z{}", w.num(op.safe_z)));
            w.motion = Some(0);
            if let Some(first) = op.toolpath.moves.first() {
                let p = first.end();
                w.pos = Some(P3::new(f64::NAN, f64::NAN, op.safe_z));
                w.rapid(P3::new(p.x, p.y, op.safe_z));
            }
        }
        for m in &op.toolpath.moves {
            match *m {
                Move::Rapid(p) => w.rapid(p),
                Move::Feed { to, feed } => w.linear(to, feed),
                Move::Arc { to, center, ccw, feed } => w.arc(to, center, ccw, feed),
            }
        }
    }

    if !s.footer.trim().is_empty() {
        if let Some(p) = w.pos {
            w.rapid(P3 { z: max_safe.max(p.z), ..p });
        }
        for l in s.footer.lines().map(str::trim).filter(|l| !l.is_empty()) {
            w.line(l);
        }
        if lcnc && !s.footer.contains('%') {
            w.line("%");
        }
        return w.out;
    }
    w.line("M5");
    if coolant {
        w.line("M9");
    }
    if let Some(p) = w.pos {
        w.rapid(P3 { z: max_safe.max(p.z), ..p });
    }
    if lcnc {
        w.line("M2");
        w.line("%");
    } else {
        w.line("M30");
    }
    w.out
}

/// Program podzielony na pliki — nowy plik przy każdej zmianie narzędzia
/// (tryb „osobne pliki”). Zwraca (przyrostek nazwy, treść).
pub fn emit_split(title: &str, ops: &[ProgramOp], s: &PostSettings) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < ops.len() {
        let tool = ops[start].tool.id;
        let mut end = start;
        while end < ops.len() && ops[end].tool.id == tool {
            end += 1;
        }
        let part = &ops[start..end];
        let suffix = format!("{}_T{}", out.len() + 1, part[0].tool.number);
        out.push((suffix, emit(&format!("{title} cz. {}", out.len() + 1), part, s)));
        start = end;
    }
    out
}

/// Liczba bez zbędnych zer, bez "-0".
pub fn fmt_num(v: f64, decimals: usize) -> String {
    let mut t = format!("{v:.decimals$}");
    if t.contains('.') {
        t = t.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if t == "-0" {
        t = "0".into();
    }
    t
}

/// Komentarz w czystym ASCII bez nawiasów (bezpieczny dla każdego sterownika).
fn ascii_comment(s: &str) -> String {
    s.replace('°', " deg")
        .chars()
        .map(|c| match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l', 'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            'Ą' => 'A', 'Ć' => 'C', 'Ę' => 'E', 'Ł' => 'L', 'Ń' => 'N', 'Ó' => 'O', 'Ś' => 'S', 'Ź' | 'Ż' => 'Z',
            'Ø' | 'ø' => 'D',
            '(' | '[' => '<',
            ')' | ']' => '>',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '?',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cam_geom::Vec2;

    fn sample_path() -> Toolpath {
        let mut tp = Toolpath::default();
        tp.rapid(P3::new(0.0, 0.0, 5.0));
        tp.rapid(P3::new(10.0, 0.0, 1.0));
        tp.feed(P3::new(10.0, 0.0, -1.0), 200.0);
        tp.arc(P3::new(0.0, 10.0, -1.0), Vec2::new(0.0, 0.0), true, 1000.0);
        tp.feed(P3::new(0.0, 0.0, -1.0), 1000.0);
        tp.rapid(P3::new(0.0, 0.0, 5.0));
        tp
    }

    #[test]
    fn grbl_program() {
        let tool = Tool::default();
        let tp = sample_path();
        let ops = [ProgramOp { name: "Kieszeń", tool: &tool, rpm: 18000.0, safe_z: 5.0, toolpath: &tp }];
        let g = emit("test", &ops, &PostSettings::default());
        assert!(g.contains("G3 X0 Y10 I-10 J0 F1000"), "{g}");
        assert!(g.contains("M3 S18000"));
        assert!(g.trim_end().ends_with("M30"));
        assert!(!g.contains("M6"));
        assert!(g.is_ascii());
    }

    #[test]
    fn linuxcnc_program_and_toolchange() {
        let t1 = Tool { id: 1, number: 1, ..Default::default() };
        let t2 = Tool { id: 2, number: 2, ..Default::default() };
        let tp = sample_path();
        let ops = [
            ProgramOp { name: "a", tool: &t1, rpm: 18000.0, safe_z: 5.0, toolpath: &tp },
            ProgramOp { name: "b", tool: &t2, rpm: 12000.0, safe_z: 5.0, toolpath: &tp },
        ];
        let g = emit("test", &ops, &PostSettings { dialect: Dialect::LinuxCnc, ..Default::default() });
        assert!(g.starts_with('%'));
        assert!(g.contains("T1 M6\nG43 H1"));
        assert!(g.contains("T2 M6\nG43 H2"));
        assert!(g.contains("M3 S12000"));
        assert!(g.trim_end().ends_with("M2\n%"));

        let g = emit("test", &ops, &PostSettings::default());
        assert!(g.contains("M0"), "GRBL: pauza na zmianę narzędzia");
    }

    #[test]
    fn linearized_arcs() {
        let tool = Tool::default();
        let tp = sample_path();
        let ops = [ProgramOp { name: "a", tool: &tool, rpm: 1000.0, safe_z: 5.0, toolpath: &tp }];
        let g = emit("t", &ops, &PostSettings { arcs: false, ..Default::default() });
        assert!(!g.contains("G3") && !g.contains("G2 "));
    }

    #[test]
    fn coolant_wcs_and_split() {
        let t1 = Tool { id: 1, number: 1, ..Default::default() };
        let t2 = Tool { id: 2, number: 2, ..Default::default() };
        let tp = sample_path();
        let ops = [
            ProgramOp { name: "a", tool: &t1, rpm: 30000.0, safe_z: 5.0, toolpath: &tp },
            ProgramOp { name: "b", tool: &t2, rpm: 12000.0, safe_z: 5.0, toolpath: &tp },
        ];
        let s = PostSettings {
            wcs: "G55".into(),
            coolant_flood: true,
            rpm_max: 24000.0,
            tool_change: ToolChange::SeparateFiles,
            ..Default::default()
        };
        let files = emit_split("t", &ops, &s);
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].0, "2_T2");
        let g = &files[0].1;
        assert!(g.contains("G55") && g.contains("M8") && g.contains("M9"));
        assert!(g.contains("M3 S24000"), "obroty ograniczone do maks. maszyny");
        assert!(!g.contains("M0"));
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt_num(-0.0001, 3), "0");
        assert_eq!(fmt_num(12.5, 3), "12.5");
        assert_eq!(fmt_num(3.0, 3), "3");
    }
}
