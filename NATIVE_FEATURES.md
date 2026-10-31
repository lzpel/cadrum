# Open native topology

`Shape` owns native geometry of any topology category. Convert existing `Solid`,
`Face`, or `Edge` values with `Shape::from(&value)`, then use typed `vertices`,
`edges`, `wires`, `faces`, `shells`, `solids`, and `compounds` accessors.
`Vertex::new`, `Wire::new`, `Face::face`, `Shell::new`, existing solid builders,
and `Shape::compound` construct these categories without intermediate files.

Open results remain open. `Shape::sew`, `extract_faces`, and `remove_faces`
preserve faces, open or closed shells, and disconnected compound results.
`Shape::to_solid` requires exactly one valid closed shell and orients it outward;
`as_solid` only accepts existing solid topology. `Shell::new` requires faces
that already share native edges; sewing connects independent faces within an
explicit positive tolerance in model length units.

Empty results are successful values. Empty sewing, empty compounds, and removal
of all faces return an empty compound; `is_empty` distinguishes it from an open
shape, while errors use `Error::Validation` or `Error::Shape`. Empty bounds are
`None`, and meshing empty topology produces no triangles. Subshape traversal
includes the root when its category matches and returns unique native subshapes.

Open geometry supports native feature chains. Translation, rotation in radians,
uniform signed scaling, reflection, bounds, validity checks, tessellation, and
binary BRep I/O accept every supported category. Meshing faces emits oriented
normals and triangle face IDs; wires and edges emit polylines. `thicken` accepts
valid faces or open shells and uses signed offsets with intersection joins and
boundary walls, at OCCT’s `Precision::Confusion()` coincidence tolerance. Kernel
failures and invalid output return errors; arbitrary self-intersections are not
guaranteed to be detected.

Ownership follows native topology lifetimes. Views own shared subshape handles,
so dropping a parent does not invalidate them; cloning copies geometry
independently. `Shape`, `Vertex`, `Wire`, `Shell`, and `Compound` implement neither
`Send` nor `Sync`. Native results remain directly usable through typed accessors
and the existing convenience APIs.

Styles follow existing faces in memory. Conversion from `Solid` expands whole-body
colors onto faces, and copying, transforms, extraction, and removal retain face
colors through topology correspondence. Newly generated thickening faces have no
assigned style. Shape BRep I/O stores exact geometry and the root category but
omits colors, history, and triangulation; binary compatibility follows the linked
OCCT version. Existing `Solid` color-aware I/O remains available.

The open-box example exercises the full chain. Run
`cargo run --example open_shape` to construct, open, transform, mesh, serialize,
restore, and thicken a box shell entirely through native shapes.

## 2. Validation and operation outcomes

Validation separates geometric validity from category expectations. `Shape::validate`
returns validity, closure, empty state, expected category matching, and indexed
BRepCheck status codes including contextual subshape errors and orientation issues.
Indices refer to the native traversal of the shape supplied to validation.

New operations report named errors. `Error::Operation` carries kernel diagnostics
and available implicated indices, `Unsupported` identifies unsupported categories,
and `Cancelled` identifies cooperative cancellation. `OperationResult` separates
successful values from warnings and diagnostics; empty geometry remains a value.
Legacy convenience APIs retain their existing error variants and signatures.

Repair is explicit. `Shape::repair` copies geometry before running permitted
ShapeFix tools, measures maximum topology tolerances and boundary counts, and
rejects tolerance overruns or unrequested closure. Its report includes validation
before and after repair; repair does not silently certify an invalid result.

## 3. General affine transforms

Affine matrices use column vectors. `Shape::affine` computes `M * [x,y,z,1]`,
accepts glam's column-major `DMat4`, and rejects perspective, non-finite, and
singular matrices before FFI. `A * B` applies B first; translation entries and
scaling origins use model length units. `scale_xyz` scales around an explicit origin.

General transforms may change geometry representation. OCCT's GTransform can
convert analytic geometry to B-splines while preserving native trim boundaries.
Results include indexed vertex, edge, and face correspondence and inherited face
styles; missing correspondence is explicitly unknown rather than deleted.
