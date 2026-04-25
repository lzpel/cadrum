use super::ffi;
use super::solid::Solid;
#[cfg(feature = "color")]
use crate::common::color::Color;

/// A compound shape wrapping multiple solids into a single `TopoDS_Compound`.
///
/// Provides type-safe distinction from individual `Solid` handles.
pub(crate) struct CompoundShape {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<u64, Color>,
	history: Vec<u64>,
}

impl CompoundShape {
	/// Assemble solids into a compound, merging their colormaps.
	///
	/// Inputs' `history` is intentionally dropped — a compound assembled for
	/// a boolean call has no meaningful history of its own; the boolean
	/// result will populate one fresh.
	pub fn new<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> Self {
		let mut inner = ffi::make_empty();
		#[cfg(feature = "color")]
		let mut colormap = std::collections::HashMap::new();
		for s in solids {
			ffi::compound_add(inner.pin_mut(), s.inner());
			#[cfg(feature = "color")]
			colormap.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
		}
		CompoundShape {
			inner,
			#[cfg(feature = "color")]
			colormap,
			history: Default::default(),
		}
	}

	/// Create a compound from a raw `TopoDS_Shape` (e.g. from I/O or boolean ops).
	pub fn from_raw(
		inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
		#[cfg(feature = "color")] colormap: std::collections::HashMap<u64, Color>,
		history: Vec<u64>,
	) -> Self {
		CompoundShape {
			inner,
			#[cfg(feature = "color")]
			colormap,
			history,
		}
	}

	/// Borrow the underlying `TopoDS_Shape`.
	pub fn inner(&self) -> &ffi::TopoDS_Shape {
		&self.inner
	}

	/// Borrow the merged colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<u64, Color> {
		&self.colormap
	}

	/// Decompose into individual solids, consuming the compound.
	///
	/// Each result solid receives a clone of the full `history` — over-inclusion
	/// is harmless because `iter_history()` consumers filter pairs by checking
	/// `src_id` against the original input's face IDs.
	pub fn decompose(self) -> Vec<Solid> {
		let solid_shapes = ffi::decompose_into_solids(&self.inner);
		solid_shapes
			.iter()
			.map(|s| {
				Solid::new(
					ffi::clone_shape_handle(s),
					#[cfg(feature = "color")]
					self.colormap.clone(),
					self.history.clone(),
				)
			})
			.collect()
	}
}
