use glam::DVec3;

/// The elementary surface a face lies on: STEP's `axis2_placement_3d` plus the
/// parameters of the subtype it places.
///
/// Reported as the surface itself carries it — the face's own orientation is not
/// folded in, so `axis_z` is not necessarily the outward normal of a planar face.
/// `Face::project` reports that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
	/// STEP `location`. Not a point on the face: a cylinder places it on the
	/// axis and a sphere at the centre.
	pub origin: DVec3,
	/// STEP `axis`, the placement's Z direction.
	pub axis_z: DVec3,
	/// The placement's X direction, fixing where `u = 0` sits on the surface.
	/// The frame is always right-handed, so Y is `axis_z.cross(axis_x)` and is
	/// not carried. A mirrored face is normalised into that convention, which
	/// negates STEP's `ref_direction` and moves `u = 0` by half a turn.
	pub axis_x: DVec3,
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
