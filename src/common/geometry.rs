use glam::DVec3;

/// The surface a face lies on: STEP's `axis2_placement_3d` plus the parameters of
/// the subtype it places, or the B-spline surface itself.
///
/// Reported as the surface itself carries it — the face's own orientation is not
/// folded in, so `axis_z` is not necessarily the outward normal of a planar face.
/// `Face::project` reports that.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
	/// STEP `location`. Not a point on the face: a cylinder places it on the
	/// axis and a sphere at the centre. The world origin for a B-spline.
	pub origin: DVec3,
	/// STEP `axis`, the placement's Z direction. World Z for a B-spline.
	pub axis_z: DVec3,
	/// The placement's X direction, fixing where `u = 0` sits on the surface.
	/// The frame is always right-handed, so Y is `axis_z.cross(axis_x)` and is
	/// not carried. A mirrored face is normalised into that convention, which
	/// negates STEP's `ref_direction` and moves `u = 0` by half a turn.
	pub axis_x: DVec3,
	pub kind: SurfaceKind,
}

/// The parameters that distinguish one surface from another.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceKind {
	Plane,
	Cylinder {
		radius: f64,
	},
	Cone {
		/// Radius where the cone crosses the plane through `origin`.
		radius: f64,
		/// Half-angle in radians, signed by the direction the cone widens.
		semi_angle: f64,
	},
	Sphere {
		radius: f64,
	},
	Torus {
		major_radius: f64,
		minor_radius: f64,
	},
	/// Any other surface, as the B-spline OCCT converts it to over the face's parameter
	/// range (exactly for B-spline, Bezier and ruled surfaces; approximated otherwise).
	Bspline(NurbsSurface),
}

/// A B-spline surface as STEP's `b_spline_surface_with_knots` and
/// `rational_b_spline_surface` carry it, in world coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsSurface {
	/// `[u_degree, v_degree]`.
	pub degree: [usize; 2],
	/// Distinct knot values in each direction, ascending.
	pub knots: [Vec<f64>; 2],
	/// Multiplicity of each knot in `knots`, so `knots` and `multiplicities` have equal lengths per direction.
	pub multiplicities: [Vec<usize>; 2],
	/// `poles[i][j]` is the control point at the `i`-th U and `j`-th V position.
	pub poles: Vec<Vec<DVec3>>,
	/// One weight per pole, all `1.0` for a non-rational surface.
	pub weights: Vec<Vec<f64>>,
	/// Whether the surface closes on itself in each direction, as a knot-vector property.
	pub periodic: [bool; 2],
}

/// The curve an edge lies on: STEP's `axis2_placement_3d` plus the parameters of the
/// conic it places, or the B-spline curve itself, together with the edge's parameter range.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
	/// The circle's or ellipse's centre, a point of the line, or the world origin for a B-spline.
	pub origin: DVec3,
	/// The conic's normal, the line's direction, or world Z for a B-spline.
	pub axis_z: DVec3,
	/// Where the conic's parameter is zero (its major axis for an ellipse). Arbitrary
	/// for a line, world X for a B-spline.
	pub axis_x: DVec3,
	pub kind: CurveKind,
	/// Parameter interval the edge covers: a length along a line, radians round a
	/// conic, knot parameters along a B-spline.
	pub range: [f64; 2],
}

/// The parameters that distinguish one curve from another.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveKind {
	Line,
	Circle {
		radius: f64,
	},
	Ellipse {
		major_radius: f64,
		minor_radius: f64,
	},
	/// Any other curve, as the B-spline OCCT converts it to over the edge's range
	/// (exactly for B-spline and Bezier curves; approximated otherwise).
	Bspline(NurbsCurve),
}

/// A B-spline curve as STEP's `b_spline_curve_with_knots` and `rational_b_spline_curve`
/// carry it, in world coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsCurve {
	pub degree: usize,
	/// Distinct knot values, ascending.
	pub knots: Vec<f64>,
	/// Multiplicity of each knot in `knots`; the two have equal lengths.
	pub multiplicities: Vec<usize>,
	pub poles: Vec<DVec3>,
	/// One weight per pole, all `1.0` for a non-rational curve.
	pub weights: Vec<f64>,
	/// Whether the curve closes on itself, as a knot-vector property.
	pub periodic: bool,
}

// ==================== flat encoding shared with ffi.cpp ====================
//
// The bridge moves a NURBS as one `Vec<f64>`; the layouts below are mirrored by
// `encode_bspline_*` / `decode_bspline_*` in src/ffi.cpp.

/// Reads successive values, `None` once the data runs out.
struct Cursor<'a>(&'a [f64]);

impl Cursor<'_> {
	fn f(&mut self) -> Option<f64> {
		let (first, rest) = self.0.split_first()?;
		self.0 = rest;
		Some(*first)
	}
	fn n(&mut self) -> Option<usize> {
		Some(self.f()? as usize)
	}
	fn vec3(&mut self) -> Option<DVec3> {
		Some(DVec3::new(self.f()?, self.f()?, self.f()?))
	}
	fn many<T>(&mut self, count: usize, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
		(0..count).map(|_| item(self)).collect()
	}
}

/// `(origin, axis_z, axis_x, p1, p2)` and the rest of the data.
pub(crate) fn split_frame(data: &[f64]) -> Option<((DVec3, DVec3, DVec3, f64, f64), &[f64])> {
	let mut c = Cursor(data);
	let frame = (c.vec3()?, c.vec3()?, c.vec3()?, c.f()?, c.f()?);
	Some((frame, c.0))
}

impl NurbsSurface {
	/// `[udeg, vdeg, uper, vper, nuk, nvk, nu, nv, uknots, umults, vknots, vmults, poles xyz (u-major), weights]`.
	pub(crate) fn to_flat(&self, out: &mut Vec<f64>) {
		let (nu, nv) = (self.poles.len(), self.poles.first().map_or(0, Vec::len));
		out.extend([self.degree[0] as f64, self.degree[1] as f64, self.periodic[0] as u8 as f64, self.periodic[1] as u8 as f64]);
		out.extend([self.knots[0].len() as f64, self.knots[1].len() as f64, nu as f64, nv as f64]);
		for d in 0..2 {
			out.extend(&self.knots[d]);
			out.extend(self.multiplicities[d].iter().map(|&m| m as f64));
		}
		out.extend(self.poles.iter().flatten().flat_map(|p| p.to_array()));
		out.extend(self.weights.iter().flatten());
	}

	pub(crate) fn from_flat(data: &[f64]) -> Option<Self> {
		let mut c = Cursor(data);
		let degree = [c.n()?, c.n()?];
		let periodic = [c.f()? != 0.0, c.f()? != 0.0];
		let (nuk, nvk, nu, nv) = (c.n()?, c.n()?, c.n()?, c.n()?);
		let mut knots = [Vec::new(), Vec::new()];
		let mut multiplicities = [Vec::new(), Vec::new()];
		for (d, nk) in [nuk, nvk].into_iter().enumerate() {
			knots[d] = c.many(nk, Cursor::f)?;
			multiplicities[d] = c.many(nk, Cursor::n)?;
		}
		let poles = c.many(nu, |c| c.many(nv, Cursor::vec3))?;
		let weights = c.many(nu, |c| c.many(nv, Cursor::f))?;
		Some(NurbsSurface { degree, knots, multiplicities, poles, weights, periodic })
	}
}

impl NurbsCurve {
	/// `[deg, periodic, nk, np, knots, mults, poles xyz, weights]`.
	pub(crate) fn to_flat(&self, out: &mut Vec<f64>) {
		out.extend([self.degree as f64, self.periodic as u8 as f64, self.knots.len() as f64, self.poles.len() as f64]);
		out.extend(&self.knots);
		out.extend(self.multiplicities.iter().map(|&m| m as f64));
		out.extend(self.poles.iter().flat_map(|p| p.to_array()));
		out.extend(&self.weights);
	}

	pub(crate) fn from_flat(data: &[f64]) -> Option<Self> {
		let mut c = Cursor(data);
		let degree = c.n()?;
		let periodic = c.f()? != 0.0;
		let (nk, np) = (c.n()?, c.n()?);
		let knots = c.many(nk, Cursor::f)?;
		let multiplicities = c.many(nk, Cursor::n)?;
		let poles = c.many(np, Cursor::vec3)?;
		let weights = c.many(np, Cursor::f)?;
		Some(NurbsCurve { degree, knots, multiplicities, poles, weights, periodic })
	}
}
