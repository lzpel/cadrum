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
colors through topology correspondence. Generated thickening faces inherit available
styled face ancestry; other generated faces remain unassigned. Shape BRep I/O stores exact geometry and the root category but
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
one-to-many descendants and separate operand ancestry. `operand_count` retains inputs
even when no subshape relations are available. `History::compose` joins relations
across an intermediate operand, preserves unrelated next operands, and confirms
deletion only when every known descendant is removed.

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

## 11. Measurements and geometric frames

Geometry classification preserves analytic parameters. `curve_info` and
`surface_info` identify elementary, Bezier, B-spline, and other native families,
parameter domains, periodicity, and available analytic frames/radii. Ellipse
parameters are major/minor radii; cone parameters are reference radius and signed
semi-angle. Curve evaluation supplies tangents through its first derivative.

Measurements use native geometry. `Edge::length` and `sample_by_length` accept
absolute integration/inversion tolerances; samples include trimmed endpoints.
`mass_properties` integrates lengths, trimmed areas, or closed-solid volumes and
returns center, inertia, and the native relative-error estimate. Surface/volume
accuracy controls adaptive Gauss integration; length properties use OCCT's native
linear-properties integration. Empty measurements have zero mass.

Differential queries respect orientation and trimming. `curvature` evaluates curve
curvature, while `curvature_uv` returns principal curvatures/directions, mean, and
Gaussian curvature using the oriented face normal and first/second fundamental
forms. Singular parameterizations return errors. `classify_uv` checks the actual
trimmed domain, including holes; `boundary_wires` identifies outer/hole loops.

Closest-point reports retain supporting topology. `distance` returns all minimum
solutions reported by OCCT, their point pairs, native support shapes, and inner
solution status; continuous families are represented by the kernel's finite set.
`classify_point` supports solids, and `ancestors` traverses native adjacency while
rejecting foreign subshapes. Existing convenience measurements remain available.

## 12. Advanced sweeps

Sweep controls map to native pipe-shell settings. `Wire::sweep` accepts ordered
wire/endpoint-vertex sections with optional spine-vertex locations and per-section
contact/correction flags. Options expose transition policy, three tolerances, C1
approximation requests, maximum degree/segments, simulation count, and explicit
shell/solid output. Results include pipe status, approximation error, history, and
simulated section wires, with cooperative cancellation during native building.

Orientation policies remain distinct. Corrected Frenet, Frenet, fixed frame,
fixed binormal, discrete, support-surface, and auxiliary-spine modes are available.
Support mode requires native pcurves on the supplied support; auxiliary mode
exposes curvilinear matching and no-contact/contact/boundary contact choices.
Round corners work best with section correction; transformed sharp corners may
self-intersect, and right-corner intersections retain kernel limitations.

Scaling laws are homothetic. Positive scale samples interpolate over normalized
spine position from zero to one, with matching endpoint values on closed spines.
A scaling law requires one section and is incompatible with auxiliary-spine mode.
It does not represent arbitrary twist. Invalid combinations return errors before
native evaluation; incompatible geometry reports the native pipe status.

Scaling-law history is unavailable in the pinned kernel. OCCT 8.0.1 crashes while
building that history, so scaling sweeps disable native history and return an
explicit incomplete-history warning. Legacy `Solid::volume` retains its original
integration policy; `Shape::mass_properties` accepts an accuracy target and reports
the native estimate for callers needing controlled integration.

## 13. Loft controls and healing reports

Lofts expose their approximation policy. `Shape::loft` accepts shell/solid choice,
ruled/smooth construction, compatibility/orientation normalization, tolerance,
smoothing, parameterization, requested continuity, maximum degree, and optimization
weights. Native input topology is kept immutable. Point vertices are permitted at
ends when at least one wire section exists; direct wire lofts expose native history.

Profile correspondence is explicit by order. Face and compound-profile sections
must have matching region and hole counts; regions and hole loops correspond in
native traversal order. Solid construction lofts each outer region and subtracts
its corresponding hole lofts. Shell construction retains outer and reversed inner
shells separately. Incompatible topology is rejected, and composite profile lofts
report incomplete history rather than claiming correspondence through subtraction.

Healing reports measured changes. Repair reports now include context-replaced or
tolerance-changed subshape indices, boundary counts, tolerance maxima, sample count,
and maximum sampled deviation. Vertices and five curve samples per edge are checked
against repaired geometry, and exceeding the requested sampled limit rejects the
repair. This measure is not a continuous Hausdorff bound on surface interiors.
Intentional openings remain unless closure is explicitly allowed.

## 14. Additional edge treatments

Chamfers identify their reference side. `Shape::chamfer` accepts selected edge/face
pairs with two distances or a distance and angle in radians; the first distance is
measured on the reference face. Foreign or non-adjacent topology and invalid local
dimensions return named errors. `Shape::reversed` exposes explicit orientation
reversal while retaining native topology identity.

Variable fillets use normalized radius laws. `variable_fillet` interpolates positive
radii at strictly increasing relative spine parameters from zero to one and returns
native generated/modified history. Native building supports cancellation and reports
excessive radii or incompatible local geometry without returning partial results.

Planar corner treatments preserve hole topology. `Face::fillet_corners` selects
vertices, while `chamfer_corners` selects adjacent edges and their distances. The
native planar algorithm supports line/circular-arc boundaries and reports its
status on failure. Generated arc/chamfer edges have explicit provenance. Fillets
provide tangential G1 joins where the kernel succeeds; C2 continuity is not promised,
and chamfers introduce C0 corners.

## 15. Draft and projection

Draft uses an explicit neutral plane and pull direction. `Shape::draft` applies a
signed angle in radians to selected faces and returns per-selection success codes
and native history; rejected additions identify their selection and kernel status.
`Face::tapered_extrude` is a separate planar-profile construction that extrudes,
drafts generated side faces, and composes provenance through both operations.

Projection returns reusable native wires. `project_onto` supports infinite
parallel-line projection or central projection from a point onto faces, shells,
solids, and compounds. Options choose trimmed clipping versus the underlying face,
retain or reject multiple connected results, and select empty/error no-hit policy.
Planar directional projection preserves curve parameters and clips exact topology.
General projection retains the native kernel's intersection limitations; an
undifferentiated kernel failure/no-intersection status remains an error.

Projection policies preserve all retained hits. Results are connected into wires
within the explicit tolerance, and no first-hit ordering is implied. Incomplete
projection history is explicit. Curved target trimming is enforced by the native
projector; untrimmed mode requires a single target face.

## 16. STEP units, products, and assemblies

STEP units are explicit at both boundaries. `read_step_with_options` reports
source length-unit names, selected normalization, candidate roots, and transferred
roots. Missing units and partial transfers are errors unless explicitly allowed;
allowed fallback and partial transfer produce warnings. `write_step_with_options`
separates model units from export units and selects AP203, AP214, or AP242.
Millimetres, metres, inches, feet, kilometres, micrometres, and centimetres are supported.

Supported styling survives controlled STEP I/O. Solid and face RGB colors use the
existing style machinery. AP214/AP242 preserve those styles, while AP203 export
omits them with a warning. Instance-specific style flattening and unsupported
transparency are reported. Geometry-only `Shape` and existing `Solid` streams stay
available without product-document APIs or extra document-oriented native libraries.

Assembly documents are optional. Enable `step-document` to use `StepDocument`,
`StepDefinition`, and `StepInstance`; the implementation uses the existing STEP
toolkit and adds no OCAF/XCAF libraries. Definitions retain product IDs, names,
native leaf geometry, and component links. Repeated components reference the same
definition, while each occurrence retains its ID, name, reference designator, and
proper rigid transform. Nested assemblies and independent roots round-trip.

Unresolved metadata stays explicit. Imported placements that cannot be represented
are `None` with warnings and cannot be re-exported until resolved. Cycles, invalid
references, non-rigid placements, and mixed geometry/component definitions are
rejected. Regions correspond to STEP product definitions and NAUO links; arbitrary
OCAF attributes, external references, layers, materials, and mapped-item-only
assembly conventions are outside this document representation.

Root products have no assembly occurrence link. Root placements may be absorbed
into their representation geometry, and root-only occurrence labels are omitted
with a warning; product IDs/names remain. Repeated root definitions must instead
be represented as components of an assembly. Unreferenced definitions are omitted
with warnings. Import preserves the full transferred native result even when
some metadata or placement is unavailable.

## Checked legacy boundaries and runnable examples

Checked primitive construction is available. `Solid::primitive(Primitive)` returns
typed validation or kernel errors for boxes, spheres, cylinders, cones, tori, and
half-spaces. Existing infallible convenience constructors use this checked path
and retain their signatures, panicking in Rust on invalid input. Legacy transforms,
meshing, spline tangents, surface frames, and blend sizes also reject non-finite
parameters before native evaluation; fallible native entry points catch exceptions.

The feature example runs every family. Use
`cargo run --example native_features --features step-document` to exercise items
2–16, or pass an item number after `--` to run that family in isolation. Each run
prints first/repeat operation times and Linux process high-water memory when
available. Timings exclude common fixture construction and do not flush OS caches;
peak memory includes the process, fixtures, and native kernel. Run one family per
process when comparing peaks. `open_shape` remains the item-1 acceptance example.

## Local timing sample

Each family was measured in a fresh Linux x86-64 process. The debug build used
Rust 1.99.1, OCCT 8.0.1 revision 3, and `step-document`; values are first-call
microseconds, repeated-call microseconds, and process peak KiB. They are local
measurements rather than performance guarantees and include the fixture/process
memory described above.

```text
item family             first_us repeat_us peak_KiB
2    validation              534       440    27428
3    affine                  806       622    27496
4    authored                 50         8    27448
5    profiles                335       208    27528
6    faceted                1580      1468    27692
7    history                 271       235    27532
8    split                  5351      4758    28344
9    boolean                2986      2716    28348
10   tessellation          13433     12510    31184
11   measurements           1087      1027    27476
12   sweep                   781       651    27600
13   loft-repair            7913      7925    27416
14   edge-treatments        11131     10408    37408
15   draft-projection       1346      1096    27756
16   STEP                  16786     13423    29224
```

Checked native chains stay in memory. This example validates a sphere, constructs
an ellipsoid, and tessellates it without intermediate files.

```rust
use cadrum::{DVec3, Error, OperationControl, Primitive, Shape, ShapeKind, Solid, Tessellation};
let sphere = Solid::primitive(Primitive::Sphere { radius: 2.0 })?;
let shape = Shape::from(&sphere);
assert!(shape.validate(Some(ShapeKind::Solid))?.valid);
let ellipsoid = shape.scale_xyz(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0))?;
let tessellation = ellipsoid.value.tessellate(Tessellation::default(), &OperationControl::default())?;
assert!(!tessellation.value.mesh.indices.is_empty());
# Ok::<(), Error>(())
```

Verification keeps feature boundaries explicit. PNG export statements in the
existing examples are feature-gated, and the PNG-dependent gallery is embedded
in crate documentation when PNG is enabled. This native guide is included in
all native builds. No README edits are required for these feature combinations.


## Verification

Linux acceptance tests pass. With `step-document`, `cargo test --all-targets`
passed 181 tests with two ignored, and the separate documentation run passed 17
with one ignored. The geometry-only `--no-default-features --offline` run passed
178 tests with two ignored. The complete `native_features` example also passed.

Windows acceptance tests pass under Wine. The GNU target used POSIX MinGW
compilers and passed all 24 selected tests for sweeps, history, mesh import,
STEP assemblies and units, and validation. These include rotated component
placements, coincident disconnected mesh components, and retained repair styles.

The source feature accepts a prepared OCCT installation. An offline
`cargo check --features source,step-document` passed with `OCCT_ROOT` pointing at
the existing Linux OCCT cache; this check does not establish a fresh source build.
macOS remains unverified because this host has no Apple SDK.


WebAssembly compilation and a runtime smoke test pass. The repository's published
cross-build image compiled the library with `--no-default-features` and
`step-document`. A separate linked module instantiated in Node with zero external
imports and exercised checked box construction, affine scaling, validation, and
tessellation, returning the expected volume of 24. This is a smoke test rather
than the complete native acceptance suite.

The pure Rust backend remains unavailable. Checks with `--features pure` fail
because the existing backend is unimplemented and native concrete types are
configured out; these OCCT additions do not supply that backend.
