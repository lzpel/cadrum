---
title: 'cadrum: parametric CAD modeling in Rust on a statically linked OpenCASCADE kernel'
tags:
  - Rust
  - CAD
  - parametric modeling
  - OpenCASCADE
  - STEP
  - neutronics
authors:
  - name: Satoshi Misumi
    orcid: 0009-0007-4840-0926
    affiliation: 1
affiliations:
  - name: Independent Researcher, Japan
    index: 1
date: 30 September 2026
bibliography: paper.bib
---

# Summary

<!--target of this-->

cadrum is a Rust library for parametric 3D CAD in code. It is open source and designed as a clean, scriptable foundation for scientific and engineering work.

<!--technology and design-->

cadrum tracks topology in Rust's type system. Topological entities are exposed as the types Solid, Face and Edge, and the operations that manipulate them are methods of these types. cadrum wraps the OpenCASCADE Technology (OCCT) kernel [@occt], where the result of an operation is a generic shape: a solid can silently come back as a shell, vanish, or split into several pieces. cadrum reflects these topological changes in its return types, so unexpected geometry is caught where it appears.

cadrum covers a wide range of CAD operations. It provides primitives, extrusion, revolution, lofting, sweeping along guided paths, B-spline surfaces, shelling, offsetting, filleting, chamfering and Boolean operations. Models are read and written as STEP [@iso10303], and exported as STL, binary glTF, SVG and PNG.

<!-- save time and universal -->

# Statement of need

Research increasingly generates geometry from computation. An optimizer or a physics equilibrium produces curves and surfaces, and a downstream solver needs watertight solids with exact surfaces. Radiation-transport codes that track particles on CAD geometry [@wilson2010dagmc; @romano2015openmc] are a typical consumer. For such work the geometry has to be scripted, reproducible, exchangeable in STEP, and easy to run inside automated pipelines.

cadrum brings this workflow to Rust. It gives researchers an exact B-rep kernel with a small, typed API, and removes the usual cost of adopting OCCT from a compiled language: there is no system installation, no CMake step for supported targets, and the result is a single self-contained binary. Errors raised inside the kernel are returned as Rust Result values instead of invalid shapes, so a failed Boolean or sweep can be handled by the calling program.

# State of the field

Existing scripted CAD tools each give up exactness, language choice or maturity. The most widely used are Python libraries built on OCCT, such as CadQuery [@cadquery] and build123d [@build123d]; they are mature and expressive, but tie a pipeline to a Python environment and to the OCCT shared libraries installed with it. OpenSCAD [@openscad] is popular for constructive solid geometry, but works on polygon meshes rather than exact surfaces. In Rust, opencascade-rs [@opencascaders] also binds OCCT, while truck [@truck] and Fornjot are kernels written in pure Rust whose coverage of advanced operations is still growing.

cadrum combines OCCT's operation set with three distinguishing choices. It is distributed as prebuilt static archives for desktop and WebAssembly targets, it keeps a deliberately small public surface of three shape types, and it gives faces and edges stable identifiers that follow shapes through modeling history.

# Software design

The public API is three types: Solid, Face and Edge. Every modeling operation is a constructor or a method of one of them, and its return type states the topology of the result.

Boolean operations use disjunctive normal form (DNF) for normalization, efficiency and lazy evaluation. They are written with the addition, subtraction and multiplication operators for union, difference and intersection, and nothing is computed until a final build call. The composed expression is normalized to DNF, and a single call to OCCT's cells builder intersects all operands at once and selects the resulting cells. Any combination of unions, differences and intersections is therefore resolved in one pass instead of one kernel call per operator.

Faces and edges keep stable identifiers. The identifiers survive Boolean and editing operations through OCCT's history, and cadrum uses them to keep per-face and per-solid colours across operations and through STEP, BRep, STL, glTF and SVG input and output. Users can also locate, by these identifiers, the faces and edges that an operation produced and modify them further; rounding the edges of a cut face is a typical example.

The kernel boundary is thin and turns failures into errors. The Rust side talks to a thin C++ layer through the cxx bridge library, and OCCT exceptions are converted to Rust errors at this boundary.

OCCT is linked statically without a system installation. The build script downloads a prebuilt, statically linkable OCCT archive for the target, or builds OCCT from the upstream sources when its source feature is enabled. Two fixes to OCCT's sweeping algorithms, prepared for upstream, are applied to the bundled kernel as patches.

Rendering is headless. Meshing and rendering to SVG and PNG are done in Rust without a display, so figures for documentation and papers can be produced in continuous integration.

# Research impact statement

cadrum is the geometry engine of alphastell [@alphastell], an open workflow that assesses the feasibility of stellarator fusion reactors from a VMEC equilibrium [@hirshman1983vmec]. VMEC gives the plasma boundary as a Fourier series in the poloidal angle $\theta$ and the toroidal angle $\phi$,

$$\mathbf{x}(\theta,\phi) = (R\cos\phi,\ R\sin\phi,\ Z),\quad R = \sum_{m,n} R_{mn}\cos(m\theta - n\phi),\quad Z = \sum_{m,n} Z_{mn}\sin(m\theta - n\phi),$$

where $n$ runs over multiples of the number of field periods. alphastell samples this series on a grid of $(\theta,\phi)$ points, and cadrum's Solid::bspline interpolates the grid with a B-spline surface periodic in both angles, giving an exact closed solid whose surface passes through every sampled point. The breeding blanket is the Boolean difference of two such solids, the boundary and a copy offset outward from it. Coil filaments optimized with SIMSOPT [@landreman2021simsopt] are interpolated as periodic B-spline curves in the same way, and Solid::sweep sweeps a rectangular cross-section along each of them. Its auxiliary-spine mode takes a second B-spline curve offset from the filament toward the plasma, so the cross-section stays normal to the filament and keeps facing the plasma, where a Frenet frame would flip at inflection points. The solids are written to STEP, converted to DAGMC geometry, and transported with OpenMC [@romano2015openmc] including secondary photons.

![Modular coils and plasma of a four-period quasi-helically symmetric stellarator built with cadrum in alphastell, shown in four views.\label{fig:coils}](al_09_coil_heating.geometry.png)

The calculation shows that a neutron shield is mandatory. With only a 50 cm lead-lithium breeder between plasma and coils, OpenMC gives a nuclear heating of 94 MW in the coils at 3.1 GW fusion power, about a thousand times the volume-averaged target used for DEMO toroidal field coils.

![Nuclear heating in the coils of \autoref{fig:coils} at 3.1 GW fusion power, computed with OpenMC on the geometry built by cadrum (logarithmic color scale).\label{fig:heating}](al_09_coil_heating.heating.png)

Because the geometry step is an ordinary, deterministic program, the whole chain from the equilibrium file to the heating tallies is rerun in continuous integration.

## Use outside the author's projects

cadrum has been on crates.io since March 2026 with over 5,000 downloads; 11 public repositories by other developers depend on it, including two simulation tools. Valurile [@valurile], a lattice Boltzmann flow solver for CPUs and CUDA GPUs, reads STEP geometry through cadrum and uses the face identifiers carried by its meshes to assign boundary conditions to individual CAD faces. Oxiprep [@oxiprep], a computer-aided engineering preprocessor, imports and builds geometry with cadrum and starts its surface and volume meshing for analysis from cadrum's tessellation of each solid. The others are design, modeling and file-conversion applications, such as a STEP-to-STL converter and a desktop 3D model library.

# AI usage disclosure

The author used generative AI in writing the software and this paper. Claude (Anthropic) was used through Claude Code to assist in writing the software, its documentation and a draft of this paper. All generated code was reviewed and tested by the author, and the text was checked against the source code and the cited works. The author takes full responsibility for the software and for the content of this manuscript.

# References
