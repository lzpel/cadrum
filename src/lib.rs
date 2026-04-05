//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 7.9.3).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`Solid`] has all methods directly (no trait import needed)

pub mod common;
pub(crate) mod traits;
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;

// Re-export OCCT types at crate root
pub use occt::edge::Edge;
pub use occt::face::Face;
pub use occt::boolean::Boolean;
pub use occt::solid::Solid;

// Re-export common types
pub use glam::DVec3;
pub use common::error::Error;
pub use common::mesh::{EdgeData, Mesh};
#[cfg(feature = "color")]
pub use common::color::Color;

// I/O (cadrum::io::read_step(...) etc.)
pub use occt::io::io;

// Re-export submodules
pub use occt::utils;

// Auto-generated inherent method delegations (trait methods → pub fn on concrete types)
include!(concat!(env!("OUT_DIR"), "/generated_delegation.rs"));
