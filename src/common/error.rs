/// Errors that can occur during CAD operations.
#[derive(Debug)]
pub enum Error {
	/// Caller-side misuse: an argument that cannot be interpreted, such as an invalid color string.
	Validation(String),

	/// Read/Writel step brep gltf etc
	Io(std::io::Error),

	/// Triangulation/meshing failed.
	Tesselation,

	/// Boolean operation (fuse/cut/common) failed.
	Boolean,

	/// Got not one solids although expecting one solid, typically as a result of boolean operation.
	NotOne(usize),

	/// Edge construction failed on degenerate input (collinear arc points, zero-length line, negative radius).
	Edge(String),

	/// Shape cleaning (UnifySameDomain) failed.
	Clean,

	/// Extrusion (`Solid::extrude`) failed: empty profile, zero-length direction, or profile not closed.
	Extrude,

	/// Pipe sweep (`Solid::sweep`) failed: profile not closed, or edges not connectable into a wire.
	Sweep(String),

	/// Shell (`Solid::shell`) failed: thickness incompatible with the geometry, or self-intersecting offset.
	Shell(String),

	/// Fillet (`Solid::fillet_edges`) failed: radius too large, tangent discontinuity, or foreign edge.
	Fillet(String),

	/// Chamfer (`Solid::chamfer_edges`) failed: distance too large, tangent discontinuity, or foreign edge.
	Chamfer(String),

	/// Loft (`Solid::loft`) failed: too few sections, or an ill-formed section wire.
	Loft(String),

	/// Sewing (`Solid::sew`) failed: the faces do not form exactly one closed shell within the tolerance.
	Sew(String),

	/// Surface offset (`Solid::offset_surface`) failed: the offset surfaces self-intersect.
	Offset(String),

	/// B-spline solid (`Solid::bspline`) failed: grid too small, or interpolation/sewing rejected the input.
	Bspline(String),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::Validation(msg) => write!(f, "Validation failed: {msg}"),
			Error::Io(e) => write!(f, "IO failed: {e}"),
			Error::Tesselation => write!(f, "Tesselation failed"),
			Error::Boolean => write!(f, "Boolean operation failed"),
			Error::NotOne(n) => write!(f, "Expected exactly one resulting Solid, got {n}"),
			Error::Edge(msg) => write!(f, "Edge failed: {msg}"),
			Error::Clean => write!(f, "Clean failed"),
			Error::Extrude => write!(f, "Extrude failed"),
			Error::Sweep(msg) => write!(f, "Sweep failed: {msg}"),
			Error::Shell(msg) => write!(f, "Shell failed: {msg}"),
			Error::Fillet(msg) => write!(f, "Fillet failed: {msg}"),
			Error::Chamfer(msg) => write!(f, "Chamfer failed: {msg}"),
			Error::Loft(msg) => write!(f, "Loft failed: {msg}"),
			Error::Sew(msg) => write!(f, "Sew failed: {msg}"),
			Error::Offset(msg) => write!(f, "Offset failed: {msg}"),
			Error::Bspline(msg) => write!(f, "Bspline failed: {msg}"),
		}
	}
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::Io(e)
	}
}
