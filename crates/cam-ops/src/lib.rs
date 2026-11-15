//! Operacje CAM 2.5D: profil, kieszeń, wiercenie, grawer → ścieżki narzędzia.

pub mod model;
pub mod operation;
pub mod path;
pub mod tool;
pub mod toolpath;

mod drill;
mod engrave;
mod finish3d;
mod pocket;
mod profile;
mod rough3d;
mod vcarve;

pub use operation::{
    generate, generate_with_model, needs_model, CutParams, Direction, DrillParams, EngraveParams, EngraveSide, Entry, Finish3DParams, Finish3DStrategy, OpKind, OpResult, Operation,
    PocketParams, PocketStrategy, ProfileParams, ProfileSide, Rough3DParams, Side, Tabs, VCarveParams,
};
pub use rough3d::ModelInput;
pub use tool::{DEFAULTS_VERSION, CutPreset, FluteDir, Tool, ToolKind, ToolLibrary, DEFAULT_MATERIAL};
pub use toolpath::{Move, Toolpath, P3};
