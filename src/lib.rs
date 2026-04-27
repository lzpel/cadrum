//! # cadrum
//!
//! Rust CAD library powered by OpenCASCADE (OCCT 8.0.0-rc5).
//!
//! ## Core Types
//! - [`Solid`] — a single solid shape (wraps `TopoDS_Shape` / `TopAbs_SOLID`)
//! - [`Solid`] has all methods directly (no trait import needed)

pub mod common;
#[cfg(not(feature = "pure"))]
pub mod occt;
#[cfg(feature = "pure")]
pub mod pure;
pub(crate) mod traits;
// `Transform` is intentionally NOT re-exported. It remains reachable only as
// `crate::traits::Transform` for internal use; external callers reach the same
// surface through `Compound` / `Wire` forwarder default methods (auto-generated
// from the `Transform` trait surface by `examples/codegen.rs`).
pub use traits::{BSplineEnd, Compound, ProfileOrient, Wire};

// Re-export backend types at crate root
#[cfg(not(feature = "pure"))]
pub use occt::edge::Edge;
#[cfg(not(feature = "pure"))]
pub use occt::face::Face;
#[cfg(not(feature = "pure"))]
use occt::io::Io; // private: referenced by the codegen-emitted IoModule free fns below
#[cfg(not(feature = "pure"))]
pub use occt::solid::Solid;

// Re-export common types
#[cfg(feature = "color")]
pub use common::color::Color;
pub use common::error::Error;
pub use common::mesh::{EdgeData, Mesh};
// Re-export glam types used in cadrum's public API. Users should reach glam
// through these re-exports (or the `cadrum::glam` module below) instead of
// adding a direct `glam` dependency — otherwise a mismatched glam minor
// version pulls in two incompatible `glam` crates and call sites fail with
// "expected `cadrum::DVec3`, found `glam::DVec3`".
pub use glam::{DMat3, DMat4, DQuat, DVec2, DVec3};
pub use glam;

// Inherent method delegations (trait methods → pub fn on concrete types) and
// IoModule free fns. The `////////// codegen.rs` regions below are regenerated
// by `cargo run --example codegen` whenever src/traits.rs is modified.
impl Edge{
    ////////// codegen.rs
    pub fn id(&self) -> u64 {<Self as crate::traits::EdgeStruct>::id(self)}
    pub fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Result<crate::Edge, Error> {<Self as crate::traits::EdgeStruct>::helix(radius, pitch, height, axis, x_ref)}
    pub fn polygon<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Result<Vec<crate::Edge>, Error> {<Self as crate::traits::EdgeStruct>::polygon(points)}
    pub fn circle(radius: f64, axis: DVec3) -> Result<crate::Edge, Error> {<Self as crate::traits::EdgeStruct>::circle(radius, axis)}
    pub fn line(a: DVec3, b: DVec3) -> Result<crate::Edge, Error> {<Self as crate::traits::EdgeStruct>::line(a, b)}
    pub fn arc_3pts(start: DVec3, mid: DVec3, end: DVec3) -> Result<crate::Edge, Error> {<Self as crate::traits::EdgeStruct>::arc_3pts(start, mid, end)}
    pub fn bspline<'a>(points: impl IntoIterator<Item = &'a DVec3>, end: BSplineEnd) -> Result<crate::Edge, Error> {<Self as crate::traits::EdgeStruct>::bspline(points, end)}
    pub fn start_point(&self) -> DVec3 {<Self as crate::traits::Wire>::start_point(self)}
    pub fn end_point(&self) -> DVec3 {<Self as crate::traits::Wire>::end_point(self)}
    pub fn start_tangent(&self) -> DVec3 {<Self as crate::traits::Wire>::start_tangent(self)}
    pub fn end_tangent(&self) -> DVec3 {<Self as crate::traits::Wire>::end_tangent(self)}
    pub fn is_closed(&self) -> bool {<Self as crate::traits::Wire>::is_closed(self)}
    pub fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {<Self as crate::traits::Wire>::approximation_segments(self, tolerance)}
    pub fn project(&self, p: DVec3) -> (DVec3, DVec3) {<Self as crate::traits::Wire>::project(self, p)}
    pub fn translate(self, translation: DVec3) -> crate::Edge {<Self as crate::traits::Wire>::translate(self, translation)}
    pub fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> crate::Edge {<Self as crate::traits::Wire>::rotate(self, axis_origin, axis_direction, angle)}
    pub fn rotate_x(self, angle: f64) -> crate::Edge {<Self as crate::traits::Wire>::rotate_x(self, angle)}
    pub fn rotate_y(self, angle: f64) -> crate::Edge {<Self as crate::traits::Wire>::rotate_y(self, angle)}
    pub fn rotate_z(self, angle: f64) -> crate::Edge {<Self as crate::traits::Wire>::rotate_z(self, angle)}
    pub fn scale(self, center: DVec3, factor: f64) -> crate::Edge {<Self as crate::traits::Wire>::scale(self, center, factor)}
    pub fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> crate::Edge {<Self as crate::traits::Wire>::mirror(self, plane_origin, plane_normal)}
    pub fn align_x(self, new_x: DVec3, y_hint: DVec3) -> crate::Edge {<Self as crate::traits::Wire>::align_x(self, new_x, y_hint)}
    pub fn align_y(self, new_y: DVec3, z_hint: DVec3) -> crate::Edge {<Self as crate::traits::Wire>::align_y(self, new_y, z_hint)}
    pub fn align_z(self, new_z: DVec3, x_hint: DVec3) -> crate::Edge {<Self as crate::traits::Wire>::align_z(self, new_z, x_hint)}
}
impl Face{
    ////////// codegen.rs
    pub fn id(&self) -> u64 {<Self as crate::traits::FaceStruct>::id(self)}
    pub fn project(&self, p: DVec3) -> (DVec3, DVec3) {<Self as crate::traits::FaceStruct>::project(self, p)}
    pub fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {<Self as crate::traits::FaceStruct>::iter_edge(self)}
}
impl Solid{
    ////////// codegen.rs
    pub fn id(&self) -> u64 {<Self as crate::traits::SolidStruct>::id(self)}
    pub fn cube(x: f64, y: f64, z: f64) -> crate::Solid {<Self as crate::traits::SolidStruct>::cube(x, y, z)}
    pub fn sphere(radius: f64) -> crate::Solid {<Self as crate::traits::SolidStruct>::sphere(radius)}
    pub fn cylinder(r: f64, axis: DVec3, h: f64) -> crate::Solid {<Self as crate::traits::SolidStruct>::cylinder(r, axis, h)}
    pub fn cone(r1: f64, r2: f64, axis: DVec3, h: f64) -> crate::Solid {<Self as crate::traits::SolidStruct>::cone(r1, r2, axis, h)}
    pub fn torus(r1: f64, r2: f64, axis: DVec3) -> crate::Solid {<Self as crate::traits::SolidStruct>::torus(r1, r2, axis)}
    pub fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> crate::Solid {<Self as crate::traits::SolidStruct>::half_space(plane_origin, plane_normal)}
    pub fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {<Self as crate::traits::SolidStruct>::iter_edge(self)}
    pub fn iter_face(&self) -> impl Iterator<Item = &Face> + '_ {<Self as crate::traits::SolidStruct>::iter_face(self)}
    pub fn iter_history(&self) -> impl Iterator<Item = [u64; 2]> + '_ {<Self as crate::traits::SolidStruct>::iter_history(self)}
    pub fn extrude<'a>(profile: impl IntoIterator<Item = &'a Edge>, dir: DVec3) -> Result<crate::Solid, Error> where Edge: 'a {<Self as crate::traits::SolidStruct>::extrude(profile, dir)}
    pub fn shell<'a>(&self, thickness: f64, open_faces: impl IntoIterator<Item = &'a Face>) -> Result<crate::Solid, Error> where Face: 'a {<Self as crate::traits::SolidStruct>::shell(self, thickness, open_faces)}
    pub fn fillet_edges<'a>(&self, radius: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<crate::Solid, Error> where Edge: 'a {<Self as crate::traits::SolidStruct>::fillet_edges(self, radius, edges)}
    pub fn chamfer_edges<'a>(&self, distance: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<crate::Solid, Error> where Edge: 'a {<Self as crate::traits::SolidStruct>::chamfer_edges(self, distance, edges)}
    pub fn sweep<'a, 'b, 'c>(profile: impl IntoIterator<Item = &'a Edge>, spine: impl IntoIterator<Item = &'b Edge>, orient: ProfileOrient<'c>) -> Result<crate::Solid, Error> where Edge: 'a + 'b {<Self as crate::traits::SolidStruct>::sweep(profile, spine, orient)}
    pub fn loft<'a, S, I>(sections: S) -> Result<crate::Solid, Error> where S: IntoIterator<Item = I>, I: IntoIterator<Item = &'a Edge>, Edge: 'a {<Self as crate::traits::SolidStruct>::loft(sections)}
    pub fn bspline(u: usize, v: usize, u_periodic: bool, point: impl Fn(usize, usize) -> DVec3) -> Result<crate::Solid, Error> {<Self as crate::traits::SolidStruct>::bspline(u, v, u_periodic, point)}
    pub fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a crate::Solid>, b: impl IntoIterator<Item = &'b crate::Solid>) -> Result<Vec<crate::Solid>, Error> where Self: 'a + 'b {<Self as crate::traits::SolidStruct>::boolean_union(a, b)}
    pub fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a crate::Solid>, b: impl IntoIterator<Item = &'b crate::Solid>) -> Result<Vec<crate::Solid>, Error> where Self: 'a + 'b {<Self as crate::traits::SolidStruct>::boolean_subtract(a, b)}
    pub fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a crate::Solid>, b: impl IntoIterator<Item = &'b crate::Solid>) -> Result<Vec<crate::Solid>, Error> where Self: 'a + 'b {<Self as crate::traits::SolidStruct>::boolean_intersect(a, b)}
    pub fn clean(&self) -> Result<crate::Solid, Error> {<Self as crate::traits::Compound>::clean(self)}
    pub fn volume(&self) -> f64 {<Self as crate::traits::Compound>::volume(self)}
    pub fn bounding_box(&self) -> [DVec3; 2] {<Self as crate::traits::Compound>::bounding_box(self)}
    pub fn contains(&self, point: DVec3) -> bool {<Self as crate::traits::Compound>::contains(self, point)}
    pub fn area(&self) -> f64 {<Self as crate::traits::Compound>::area(self)}
    pub fn center(&self) -> DVec3 {<Self as crate::traits::Compound>::center(self)}
    pub fn inertia(&self) -> DMat3 {<Self as crate::traits::Compound>::inertia(self)}
    #[cfg(feature = "color")]
    pub fn color(self, color: impl Into<Color>) -> crate::Solid {<Self as crate::traits::Compound>::color(self, color)}
    #[cfg(feature = "color")]
    pub fn color_clear(self) -> crate::Solid {<Self as crate::traits::Compound>::color_clear(self)}
    pub fn union<'a>(&self, tool: impl IntoIterator<Item = &'a crate::Solid>) -> Result<Vec<crate::Solid>, Error> where crate::Solid: 'a {<Self as crate::traits::Compound>::union(self, tool)}
    pub fn subtract<'a>(&self, tool: impl IntoIterator<Item = &'a crate::Solid>) -> Result<Vec<crate::Solid>, Error> where crate::Solid: 'a {<Self as crate::traits::Compound>::subtract(self, tool)}
    pub fn intersect<'a>(&self, tool: impl IntoIterator<Item = &'a crate::Solid>) -> Result<Vec<crate::Solid>, Error> where crate::Solid: 'a {<Self as crate::traits::Compound>::intersect(self, tool)}
    pub fn translate(self, translation: DVec3) -> crate::Solid {<Self as crate::traits::Compound>::translate(self, translation)}
    pub fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> crate::Solid {<Self as crate::traits::Compound>::rotate(self, axis_origin, axis_direction, angle)}
    pub fn rotate_x(self, angle: f64) -> crate::Solid {<Self as crate::traits::Compound>::rotate_x(self, angle)}
    pub fn rotate_y(self, angle: f64) -> crate::Solid {<Self as crate::traits::Compound>::rotate_y(self, angle)}
    pub fn rotate_z(self, angle: f64) -> crate::Solid {<Self as crate::traits::Compound>::rotate_z(self, angle)}
    pub fn scale(self, center: DVec3, factor: f64) -> crate::Solid {<Self as crate::traits::Compound>::scale(self, center, factor)}
    pub fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> crate::Solid {<Self as crate::traits::Compound>::mirror(self, plane_origin, plane_normal)}
    pub fn align_x(self, new_x: DVec3, y_hint: DVec3) -> crate::Solid {<Self as crate::traits::Compound>::align_x(self, new_x, y_hint)}
    pub fn align_y(self, new_y: DVec3, z_hint: DVec3) -> crate::Solid {<Self as crate::traits::Compound>::align_y(self, new_y, z_hint)}
    pub fn align_z(self, new_z: DVec3, x_hint: DVec3) -> crate::Solid {<Self as crate::traits::Compound>::align_z(self, new_z, x_hint)}
}
////////// codegen.rs Io
pub fn read_step<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {<crate::Io as crate::traits::IoModule>::read_step(reader)}
pub fn read_brep_binary<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {<crate::Io as crate::traits::IoModule>::read_brep_binary(reader)}
pub fn read_brep_text<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {<crate::Io as crate::traits::IoModule>::read_brep_text(reader)}
pub fn write_step<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> where Solid: 'a {<crate::Io as crate::traits::IoModule>::write_step(solids, writer)}
pub fn write_brep_binary<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> where Solid: 'a {<crate::Io as crate::traits::IoModule>::write_brep_binary(solids, writer)}
pub fn write_brep_text<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> where Solid: 'a {<crate::Io as crate::traits::IoModule>::write_brep_text(solids, writer)}
pub fn mesh<'a>(solids: impl IntoIterator<Item = &'a Solid>, tolerance: f64) -> Result<Mesh, Error> where Solid: 'a {<crate::Io as crate::traits::IoModule>::mesh(solids, tolerance)}