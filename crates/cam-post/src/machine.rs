//! Profile maszyn: pole robocze, dynamika, wrzeciono, zmiana narzędzia, ustawienia G-code.

use crate::{Dialect, PostSettings, ToolChange};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Controller {
    #[default]
    Grbl,
    FluidNc,
    LinuxCnc,
}

impl Controller {
    pub const ALL: [Controller; 3] = [Controller::Grbl, Controller::FluidNc, Controller::LinuxCnc];

    pub fn label(&self) -> &'static str {
        match self {
            Controller::Grbl => "GRBL",
            Controller::FluidNc => "FluidNC",
            Controller::LinuxCnc => "LinuxCNC",
        }
    }
    pub fn dialect(&self) -> Dialect {
        match self {
            Controller::Grbl | Controller::FluidNc => Dialect::Grbl,
            Controller::LinuxCnc => Dialect::LinuxCnc,
        }
    }
}

/// Narożnik stołu, w którym maszyna się bazuje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HomeCorner {
    #[default]
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
}

impl HomeCorner {
    pub const ALL: [HomeCorner; 4] = [HomeCorner::BackLeft, HomeCorner::BackRight, HomeCorner::FrontLeft, HomeCorner::FrontRight];

    pub fn label(&self) -> &'static str {
        match self {
            HomeCorner::FrontLeft => "Lewy przód",
            HomeCorner::FrontRight => "Prawy przód",
            HomeCorner::BackLeft => "Lewy tył",
            HomeCorner::BackRight => "Prawy tył",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpindleKind {
    /// Frezarka ręczna — obroty ustawiane pokrętłem.
    Manual,
    #[default]
    Pwm,
    /// Falownik (VFD).
    Vfd,
}

impl SpindleKind {
    pub fn label(&self) -> &'static str {
        match self {
            SpindleKind::Manual => "Ręczne (frezarka)",
            SpindleKind::Pwm => "PWM",
            SpindleKind::Vfd => "Falownik (VFD)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Machine {
    pub id: u32,
    pub name: String,
    pub controller: Controller,
    // --- pole robocze ---
    pub travel_x: f64,
    pub travel_y: f64,
    pub travel_z: f64,
    pub home: HomeCorner,
    pub safe_z: f64,
    pub wcs: String,
    /// Maks. wysunięcie narzędzia pod bramą [mm].
    pub max_tool_length: f64,
    // --- ruch ---
    pub max_feed_xy: f64,
    pub max_feed_z: f64,
    pub rapid_xy: f64,
    pub rapid_z: f64,
    /// Przyspieszenia [mm/s²].
    pub accel_xy: f64,
    pub accel_z: f64,
    // --- wrzeciono i narzędzia ---
    pub spindle: SpindleKind,
    pub rpm_min: f64,
    pub rpm_max: f64,
    pub spindle_delay: f64,
    pub tool_change: ToolChange,
    /// Pozycja zmiany narzędzia (współrzędne maszyny, G53).
    pub tool_change_pos: Option<[f64; 3]>,
    pub coolant_mist: bool,
    pub coolant_flood: bool,
    pub probe: bool,
    pub probe_plate: f64,
    // --- G-code ---
    pub decimals: usize,
    pub arcs: bool,
    pub line_numbers: bool,
    pub comments: bool,
    pub linuxcnc_path_tolerance: f64,
    pub extension: String,
    pub header: String,
    pub footer: String,
}

impl Default for Machine {
    fn default() -> Self {
        Machine {
            id: 0,
            name: "Frezarka 6040".into(),
            controller: Controller::Grbl,
            travel_x: 600.0,
            travel_y: 400.0,
            travel_z: 80.0,
            home: HomeCorner::BackLeft,
            safe_z: 5.0,
            wcs: "G54".into(),
            max_tool_length: 60.0,
            max_feed_xy: 5000.0,
            max_feed_z: 1500.0,
            rapid_xy: 6000.0,
            rapid_z: 2000.0,
            accel_xy: 500.0,
            accel_z: 300.0,
            spindle: SpindleKind::Pwm,
            rpm_min: 6000.0,
            rpm_max: 24000.0,
            spindle_delay: 2.0,
            tool_change: ToolChange::ManualPause,
            tool_change_pos: None,
            coolant_mist: false,
            coolant_flood: false,
            probe: false,
            probe_plate: 20.0,
            decimals: 3,
            arcs: true,
            line_numbers: false,
            comments: true,
            linuxcnc_path_tolerance: 0.01,
            extension: "nc".into(),
            header: String::new(),
            footer: String::new(),
        }
    }
}

impl Machine {
    pub fn summary(&self) -> String {
        format!(
            "{} · {} × {} × {}",
            self.controller.label(),
            fmt(self.travel_x),
            fmt(self.travel_y),
            fmt(self.travel_z)
        )
    }

    pub fn post_settings(&self) -> PostSettings {
        let manual_spindle = self.spindle == SpindleKind::Manual;
        PostSettings {
            dialect: self.controller.dialect(),
            decimals: self.decimals.clamp(1, 6),
            arcs: self.arcs,
            line_numbers: self.line_numbers,
            comments: self.comments,
            spindle_delay: if manual_spindle { 0.0 } else { self.spindle_delay },
            linuxcnc_path_tolerance: self.linuxcnc_path_tolerance,
            wcs: if self.wcs.trim().is_empty() { "G54".into() } else { self.wcs.trim().to_string() },
            tool_change: self.tool_change,
            tool_change_pos: self.tool_change_pos,
            coolant_mist: self.coolant_mist,
            coolant_flood: self.coolant_flood,
            rpm_min: if manual_spindle { 0.0 } else { self.rpm_min },
            rpm_max: if manual_spindle { 0.0 } else { self.rpm_max },
            header: self.header.clone(),
            footer: self.footer.clone(),
        }
    }

    pub fn extension(&self) -> &str {
        let e = self.extension.trim().trim_start_matches('.');
        if e.is_empty() { self.controller.dialect().extension() } else { e }
    }
}

fn fmt(v: f64) -> String {
    let t = format!("{v:.1}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineLibrary {
    pub machines: Vec<Machine>,
    /// Identyfikator aktywnej maszyny.
    pub active: u32,
}

impl MachineLibrary {
    pub fn with_defaults() -> Self {
        let mut lib = MachineLibrary::default();
        let a = lib.add(Machine::default());
        lib.add(Machine {
            name: "Mini CNC 3018".into(),
            controller: Controller::Grbl,
            travel_x: 300.0,
            travel_y: 180.0,
            travel_z: 45.0,
            max_feed_xy: 1500.0,
            max_feed_z: 500.0,
            rapid_xy: 2000.0,
            rapid_z: 600.0,
            rpm_min: 0.0,
            rpm_max: 10000.0,
            ..Machine::default()
        });
        lib.add(Machine {
            name: "Router 1325 (LinuxCNC)".into(),
            controller: Controller::LinuxCnc,
            travel_x: 2500.0,
            travel_y: 1300.0,
            travel_z: 200.0,
            max_feed_xy: 15000.0,
            max_feed_z: 5000.0,
            rapid_xy: 20000.0,
            rapid_z: 6000.0,
            spindle: SpindleKind::Vfd,
            tool_change: ToolChange::Automatic,
            extension: "ngc".into(),
            ..Machine::default()
        });
        lib.active = a;
        lib
    }

    pub fn add(&mut self, mut m: Machine) -> u32 {
        m.id = self.machines.iter().map(|m| m.id).max().map_or(1, |x| x + 1);
        let id = m.id;
        self.machines.push(m);
        id
    }

    pub fn get(&self, id: u32) -> Option<&Machine> {
        self.machines.iter().find(|m| m.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Machine> {
        self.machines.iter_mut().find(|m| m.id == id)
    }

    /// Aktywna maszyna (pierwsza, jeśli wskazana nie istnieje).
    pub fn active(&self) -> Option<&Machine> {
        self.get(self.active).or(self.machines.first())
    }

    pub fn remove(&mut self, id: u32) {
        self.machines.retain(|m| m.id != id);
        if self.get(self.active).is_none() {
            self.active = self.machines.first().map_or(0, |m| m.id);
        }
    }
}
