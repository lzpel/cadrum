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

## 4. Authored curves and surfaces

Control data defines geometry directly. `CurveControlData::Bezier` and `BSpline`
construct edges from poles, optional positive weights, degree, strictly increasing
knots, multiplicities, and periodicity; `SurfaceControlData` does the same for
independent U/V axes. Empty weights mean all ones. Control nets use U-major order:
`poles[u * v_count + v]`. These APIs do not interpolate their poles.

Trims use native parameters. `edge(Some([first,last]))` and
`face(Some([umin,umax,vmin,vmax]))` preserve exact underlying native geometry.
`Edge::ellipse` uses a perpendicular right-handed `Frame`, major/minor radii, and
optional increasing angular bounds in radians; its frame directions are normalized.
`Edge::trimmed` restricts an existing edge, while `Face::surface_patch` creates a
rectangular patch of its underlying surface rather than retaining old trim wires.

Authored data remains inspectable. `control_data`, `parameter_range`, `evaluate`,
and `evaluate_uv` read poles, weights, spline axes, positions, and first/second
derivatives. Surface normals follow face orientation. Existing native profile and
solid operations accept these edges and faces; BRep round trips preserve authored data.

## 5. Reusable profiles

Profiles normalize planar loop nesting. `Shape::profile` accepts wires in either
winding and constructs faces for outer loops and islands, with direct child loops
as holes. It rejects open, non-planar, crossing, touching, and coincident contours.
`Face::from_wires` additionally requires the designated outer loop to contain every
hole and rejects nested holes or disconnected outer regions.

Native faces remain reusable. `Shape::extrude` retains holes and disconnected
regions under oblique displacement; `revolve` takes an explicit axis and signed
angle in radians, limited to one full turn. Both operations return native output
and builder-provided vertex/edge/face history, including profile-edge to side-face
relations. General profile construction currently marks history as incomplete.

## 6. Faceted mesh and STL import

Mesh validation precedes reconstruction. `TriangleMesh::import` checks finite
positions, index bounds, area, duplicate triangles, edge incidence, winding,
orientability, components, and signed component volumes. Its report records
boundary/non-manifold edges, removed triangles, weld counts, orientation changes,
and component volumes. Invalid index and resource inputs return named errors.

Reconstruction is an explicit outcome. `MeshOnly` retains triangle data and
reports topology defects; `OpenShell` builds sewn faceted BRep, while `ClosedSolid`
requires closed manifold components with outward winding. Every disconnected
component survives in a compound. Sewing tolerance is explicit; attaching a
triangulation alone never establishes a solid or reconstructs analytic surfaces.

Repair is opt-in. Welding, duplicate/degenerate removal, and component orientation
have separate flags. Welding uses a spatial grid and the selected tolerance;
closed components are oriented outward only when requested. Import limits bound
vertices, triangles, and STL bytes before native construction.

STL units require an explicit scale. `TriangleMesh::read_stl(reader, scale,
options)` accepts ASCII and binary files, validates complete facets, and converts
file coordinates into model length units. Exact repeated STL coordinates become
shared indexed vertices; tolerance-based welding remains an explicit repair.
Reconstructed solids support existing native Booleans and STEP export.

## 7. Topology correspondence

History describes explicit relations. Operation results carry modified, generated,
removed, unchanged, and unknown relations for vertices, edges, and faces; unknown
relations make `complete` false. Builder-confirmed deletion is the only source of
`Removed`. Copies, restricted/affine transforms, profile operations, Booleans,
blends, sewing, and face unification expose available builder correspondence.

References identify their operand and traversal index. `topology_at` resolves a
one-based index against the original operand or result; references are runtime
operation data, not durable document names or serialized labels. Relations retain
one-to-many descendants and separate operand ancestry. `History::compose` joins
relations across an intermediate operand and preserves unrelated next operands.

Styles propagate through available ancestry. Split descendants inherit source
face styles and generated faces inherit available styled ancestors; when multiple
styled ancestors merge, the last input operand wins. Missing history is exposed
rather than treated as deletion. Existing `Solid::iter_history` remains available
with its original face-pair contract; the general APIs expose the richer history.

## 8. Section, split, and surface operations

Sections remain native curves. `Shape::section` computes intersection topology with
explicit tolerance, approximation, and pcurve policies; disjoint or tangent inputs
may produce valid empty or lower-dimensional results. `Plane::face` constructs an
unbounded positioned plane without moving the requested origin or normal.

Splitting retains every partition. `Shape::split` accepts native face, shell, and
solid tools and returns native partitions plus history. `split_by_plane` additionally
classifies partition centroids into positive/negative/on-plane sides and records
available ancestor references against its combined result. `children` traverses
direct children, while typed accessors traverse all matching subshapes.

Offsets and thickening are distinct operations. `offset_surface` constructs a
parallel face/shell; `thicken_with_options` adds boundary walls and requires a solid
result. Both expose signed distance, intersection/arc joins, and coincidence
tolerance. Face removal stays separate; existing solid hollowing remains available.
Kernel limitations on excessive offsets and self-intersections still apply.

## 9. Boolean controls and cancellation

Boolean execution policies are explicit. `Shape::boolean_batch` accepts argument
and tool lists, fuzzy tolerance, glue policy, non-destructive mode, and native
parallel execution. Defaults preserve inputs, disable glue and parallelism, and
use the kernel's default numerical floor; diagnostics record effective options.
Native errors, warnings, empty output, and history remain separate.

Progress callbacks are synchronous and serialized. `OperationControl` owns a
`Send + 'static` callback that may execute on an OCCT worker thread; it must return
promptly and must not reuse its active control. Returning false requests
cancellation. A cloneable `CancellationToken` can also cancel from another thread.
Callback panics are caught in Rust and reported as operation errors without
unwinding across native frames. No callback survives the synchronous call.

Cancellation is cooperative. The native progress indicator polls the token during
supported kernel phases and discards partial results on cancellation. The error
is `Cancelled`, rather than a valid empty result; callback failures remain errors.
Callback reentrancy or concurrent reuse of one active control is rejected.

## 10. Tessellation and native data boundaries

Tessellation is disposable output. `Shape::tessellate` returns the existing mesh,
per-vertex optional UVs, OCCT status flags, warnings, and cooperative progress or
cancellation. Linear and angular deflection retain the existing absolute/relative
policy. Face orientation determines normals and winding, including reflections;
native triangulation is cleaned on completion and failure.

Native I/O preserves open geometry. `Shape::read_step` and `write_step` operate on
memory buffers without solid filtering, retaining open surfaces and disconnected
components. They support the same operation control and report partial transfer.
Geometry-only STEP and binary BRep omit styles and history; existing color-aware
Solid I/O remains available. Binary BRep compatibility follows the linked OCCT
format version rather than a portable cross-version serialization guarantee.

Native handles remain thread-confined. `Shape`, `Solid`, `Face`, `Edge`, and the
typed topology views implement neither `Send` nor `Sync`, since extracted topology
can share mutable geometry even when each wrapper owns its handle. Earlier unsafe
`Send` implementations have been removed. Independent workers may construct their
own native shapes; mesh data and cancellation tokens remain transferable.
