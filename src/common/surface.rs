use glam::DVec3;

/// The elementary surface a face lies on: STEP's `axis2_placement_3d` plus the
/// parameters of the subtype it places.
///
/// Reported as the surface itself carries it — the face's own orientation is not
/// folded in, so `axis` is not necessarily the outward normal of a planar face.
/// `Face::project` reports that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
	/// STEP `location`. Not a point on the face: a cylinder places it on the
	/// axis and a sphere at the centre.
	pub origin: DVec3,
	/// STEP `axis`, the placement's Z direction.
	pub axis: DVec3,
	/// STEP `ref_direction`, already made orthogonal to `axis` and normalised.
	/// It carries no shape, only where `u = 0` sits on the surface.
	pub ref_dir: DVec3,
	/// Whether the placement is right-handed, i.e. whether its Y direction is
	/// `axis × ref_dir` rather than its negation. Mirroring produces the latter.
	pub right_handed: bool,
	pub kind: SurfaceKind,
}

/// The parameters that distinguish one elementary surface from another.
#[derive(Debug, Clone, Copy, PartialEq)]
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
}

impl Surface {
	/// The placement's Y direction, completing the frame with `axis` and `ref_dir`.
	pub fn y_dir(&self) -> DVec3 {
		let y = self.axis.cross(self.ref_dir);
		if self.right_handed {
			y
		} else {
			-y
		}
	}
}
