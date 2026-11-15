//! Operacje CAM 2.5D: profil, kieszeń, wiercenie, grawer → ścieżki narzędzia.

pub mod operation;
pub mod path;
pub mod tool;
pub mod toolpath;

mod drill;
mod engrave;
mod pocket;
mod profile;
mod vcarve;

pub use operation::{
    generate, CutParams, Direction, DrillParams, EngraveParams, EngraveSide, Entry, OpKind, OpResult, Operation,
    PocketParams, PocketStrategy, ProfileParams, ProfileSide, Side, Tabs, VCarveParams,
};
pub use tool::{CutPreset, FluteDir, Tool, ToolKind, ToolLibrary, DEFAULT_MATERIAL};
pub use toolpath::{Move, Toolpath, P3};
