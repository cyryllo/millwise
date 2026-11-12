//! Podstawowa geometria 2D dla Millwise.
//!
//! Kontur to polilinia z łukami zapisanymi jako `bulge` (tak jak w DXF i w
//! `cavalier_contours`): segment od wierzchołka `i` do `i+1` jest łukiem,
//! jeśli `verts[i].bulge != 0`. `bulge = tan(kąt_łuku / 4)`, dodatni = CCW.

pub mod chain;
pub mod contour;
pub mod math;

pub use chain::{chain_segments, Segment};
pub use contour::{Contour, Vertex};
pub use math::{Aabb2, Vec2};
