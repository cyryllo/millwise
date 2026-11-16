//! Import plików wejściowych (DXF, później STL).

pub mod dxf_import;

pub use dxf_import::{load_dxf, DxfImport, ImportOptions, ImportedContour};

pub mod stl;
pub use stl::{load_stl, parse_stl, Mesh};

pub mod relief;
pub use relief::{decode_image, is_image_path, load_image, relief_mesh, GrayImage, ReliefParams};
