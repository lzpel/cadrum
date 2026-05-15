/// Errors that can occur during CAD operations.
#[derive(Debug)]
pub enum Error {
	/// STEP file read failed (invalid format or corrupted data).
	StepReadFailed,

	/// BRep file read failed (invalid format or corrupted data).
	BrepReadFailed,

	/// STEP file write failed.
	StepWriteFailed,

	/// BRep file write failed.
	BrepWriteFailed,

	/// Triangulation/meshing failed.
	TriangulationFailed,

	/// Boolean operation (fuse/cut/common) failed.
	BooleanOperationFailed,

	/// 単一 Solid を期待する演算 (`+`/`-`/`*` 演算子) で結果の Solid 数が 1 でなかった。
	/// `usize` は実際の結果 Solid 数 (0 = 非交差/全削除、2+ = 結果が複数ピースに分割)。
	/// 戻り値が複数ピースになりうる場合は `Solid::boolean_union/subtract/intersect` を
	/// 直接使い `Vec<Solid>` を受け取ること。
	OneFailed(usize),

	/// Shape cleaning (UnifySameDomain) failed.
	CleanFailed,

	/// Helix edge construction failed (e.g. degenerate parameters).
	HelixFailed,

	/// Extrusion (`Solid::extrude`) failed: empty profile, zero-length
	/// direction, or profile not closed.
	ExtrudeFailed,

	/// Pipe sweep (`Solid::sweep`) failed: profile not closed, edges not
	/// connectable into a wire, or `BRepOffsetAPI_MakePipe` returned no shape.
	SweepFailed,

	/// Shell / hollow (`Solid::shell` via `BRepOffsetAPI_MakeThickSolid`)
	/// failed: thickness sign incompatible with geometry, sharp corners
	/// yielding a self-intersecting offset surface, or OCCT internal failure.
	ShellFailed,

	/// Fillet (`Solid::fillet_edges` via `BRepFilletAPI_MakeFillet`) failed:
	/// radius too large for the local geometry, tangent discontinuity along
	/// the selected edge chain, or an edge not belonging to `self` was passed.
	FilletFailed,

	/// Chamfer (`Solid::chamfer_edges` via `BRepFilletAPI_MakeChamfer`) failed:
	/// distance too large for the local geometry, tangent discontinuity along
	/// the selected edge chain, or an edge not belonging to `self` was passed.
	ChamferFailed,

	/// Lofting (`Solid::loft` / `BRepOffsetAPI_ThruSections`) failed: section
	/// count too low, section wire ill-formed, or OCCT internal failure.
	/// The string identifies which precondition or stage failed.
	LoftFailed(String),

	/// B-spline solid (`Solid::bspline`) construction failed: grid too small,
	/// surface interpolation rejected the input, or sewing/capping failed.
	/// The string identifies which stage failed and with what parameters.
	BsplineFailed(String),

	/// Edge construction failed due to degenerate input (e.g. collinear arc
	/// points, zero-length line, negative radius). The string describes which
	/// constructor failed and with which parameters.
	InvalidEdge(String),

	/// SVG export (HLR projection) failed.
	SvgExportFailed,

	/// PNG export failed (rasterizer / encoder / writer).
	PngExportFailed,

	/// STL write failed.
	StlWriteFailed,

	/// Invalid color string (unrecognized name or invalid hex format).
	InvalidColor(String),

	/// Unknown error.
	Unknown(String),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::StepReadFailed => write!(f, "STEP read failed"),
			Error::BrepReadFailed => write!(f, "BRep read failed"),
			Error::StepWriteFailed => write!(f, "STEP write failed"),
			Error::BrepWriteFailed => write!(f, "BRep write failed"),
			Error::TriangulationFailed => write!(f, "Triangulation failed"),
			Error::BooleanOperationFailed => write!(f, "Boolean operation failed"),
			Error::OneFailed(n) => write!(f, "Expected exactly one resulting Solid, got {}", n),
			Error::CleanFailed => write!(f, "Shape clean failed"),
			Error::HelixFailed => write!(f, "Helix failed"),
			Error::ExtrudeFailed => write!(f, "Extrude failed"),
			Error::SweepFailed => write!(f, "Sweep failed"),
			Error::ShellFailed => write!(f, "Shell failed"),
			Error::FilletFailed => write!(f, "Fillet failed"),
			Error::ChamferFailed => write!(f, "Chamfer failed"),
			Error::LoftFailed(msg) => write!(f, "Loft failed: {}", msg),
			Error::BsplineFailed(msg) => write!(f, "Bspline failed: {}", msg),
			Error::InvalidEdge(msg) => write!(f, "Invalid edge: {}", msg),
			Error::SvgExportFailed => write!(f, "SVG export failed"),
			Error::PngExportFailed => write!(f, "PNG export failed"),
			Error::StlWriteFailed => write!(f, "STL write failed"),
			Error::InvalidColor(s) => write!(f, "Invalid color: \"{}\"", s),
			Error::Unknown(msg) => write!(f, "Unknown error: {}", msg),
		}
	}
}

impl std::error::Error for Error {}
