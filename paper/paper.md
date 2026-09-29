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
  - name: Independent Researcher
    index: 1
date: 30 September 2026
bibliography: paper.bib
---

# Summary

<!--target of this-->

cadrum is an open-source Rust library for building parametric 3D CAD models in code, designed to be a clean, scriptable foundation for scientific and engineering work.

<!--technology and design-->

cadrum is designed to express topology in Rust's strong type system to avoid getting unexpcted geometries. the topological entities are exposed as the types Solid, Face and Edge, and the operations that manipulate them are methods of these types. cadrum wraps the OpenCASCADE Technology (OCCT) kernel [@occt]. In OCCT the result of an operation is a generic shape, so a solid can silently come back as a shell, vanish, or split into several pieces. cadrum tracks these topological changes through the return types.

A wide range of CAD operations is supperted. It has primitives, extrusion, revolution, lofting, sweeping along guided paths, B-spline surfaces, shelling, offsetting, filleting, chamfering and Boolean operations. Models are read and written as STEP [@iso10303], and exported as STL, binary glTF, SVG and PNG.

<!-- save time and universal -->

# Statement of need

Scientific and engineering studies increasingly generate geometry from computation. an optimizer or a physics equilibrium produces curves and surfaces, and a downstream solver needs watertight solids with exact surfaces. Radiation-transport codes that track particles on CAD geometry [@wilson2010dagmc; @romano2015openmc] are a typical consumer. For such work the geometry has to be scripted, reproducible, exchangeable in STEP, and easy to run inside automated pipelines.

cadrum targets researchers who want this workflow in Rust. It gives them an exact B-rep kernel with a small, typed API, and removes the usual cost of adopting OCCT from a compiled language: there is no system installation, no CMake step for supported targets, and the result is a single self-contained binary. Errors raised inside the kernel are returned as Rust Result values instead of invalid shapes, so a failed Boolean or sweep can be handled by the calling program.

# State of the field

The most widely used scripted CAD tools are Python libraries built on OCCT, such as CadQuery [@cadquery] and build123d [@build123d]. They are mature and expressive, but tie a pipeline to a Python environment and to OCCT shared libraries installed with it. OpenSCAD is popular for constructive solid geometry, but works on polygon meshes rather than exact surfaces. In Rust, opencascade-rs [@opencascaders] also binds OCCT, while truck [@truck] and Fornjot are kernels written in pure Rust whose coverage of advanced operations is still growing. cadrum differs from these by combining OCCT's operation set with distribution as prebuilt static archives for desktop and WebAssembly targets, a deliberately small public surface of three shape types, and stable identifiers for faces and edges that follow shapes through modeling history.

# Software design

The public API consists of Solid, Face and Edge. Boolean operations are written with the addition, subtraction and multiplication operators for union, difference and intersection, and evaluated lazily by a final build call, so chains of operations are composed before the kernel runs. Every face and edge carries an identifier that survives Boolean and editing operations through OCCT's history; cadrum uses it to keep per-face and per-solid colours across operations and through STEP, BRep, STL, glTF and SVG input and output.

The Rust side talks to a thin C++ layer through the cxx bridge library. OCCT exceptions are converted to Rust errors at this boundary. The build script downloads a prebuilt, statically linkable OCCT archive for the target, or builds OCCT from the upstream sources when its source feature is enabled. Two fixes to OCCT's sweeping algorithms, prepared for upstream, are applied to the bundled kernel as patches. Meshing and rendering to SVG and PNG are done in Rust without a display, so figures for documentation and papers can be produced in continuous integration.

# Research impact statement

cadrum is the geometry engine of alphastell [@alphastell], an open workflow that assesses the feasibility of stellarator fusion reactors from a VMEC equilibrium [@hirshman1983vmec]. In alphastell, modular coil filaments optimized with SIMSOPT [@landreman2021simsopt] are turned into solid coils with cadrum. For each point on a coil centerline the nearest point on the last closed flux surface is found, the direction to it is used as an auxiliary guide, and a 40 cm by 50 cm cross-section is swept along the centerline. Breeding blanket shells are produced by extruding the flux surface along its normal. The solids are written to STEP, converted to DAGMC geometry, and transported with OpenMC [@romano2015openmc] including secondary photons.

![Modular coils and plasma of a four-period quasi-helically symmetric stellarator built with cadrum in alphastell, shown in four views.\label{fig:coils}](al_09_coil_heating.geometry.png)

With only a 50 cm lead-lithium breeder between plasma and coils, the OpenMC calculation gives a nuclear heating of 94 MW in the coils at 3.1 GW fusion power, about a thousand times the volume-averaged target used for DEMO toroidal field coils; this quantifies that a neutron shield is mandatory. The whole chain, from the equilibrium file to the heating tallies, is reproduced by the repository's continuous integration, which is possible because the geometry step is an ordinary, deterministic program.

# AI usage disclosure

The author used Claude (Anthropic) through Claude Code to assist in writing the software, its documentation and a draft of this paper. All generated code was reviewed and tested by the author, and the text was checked against the source code and the cited works. The author takes full responsibility for the software and for the content of this manuscript.

# References
