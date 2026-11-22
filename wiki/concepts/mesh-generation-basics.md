---
type: concept
tags: [foundation, mesh]
sources: [ref-venkatakrishnan-1995-icase-95-3, ref-mit-ocw-2-29-numerics, ref-jasak-1996-phd-thesis]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mesh generation basics (structured, unstructured, cell types)

## In short

Before any finite-volume solve, the flow domain has to be cut into cells. Structured meshes arrange cells along grid lines that can be numbered $(i, j, k)$; unstructured meshes allow any arrangement and number cells one by one. Structured meshes are efficient and accurate but hard to fit to complex shapes; unstructured meshes (triangles, tetrahedra, general polyhedra) fit anything and are easier to automate ([[ref-venkatakrishnan-1995-icase-95-3]] 1; [[ref-mit-ocw-2-29-numerics]] L15 s.12).

## Explanation

**Structured and block-structured.** In a structured grid the points lie on intersecting families of grid lines, so neighbours are known from the indices ([[ref-mit-ocw-2-29-numerics]] L15 s.12). Body-fitted structured grids in generalised coordinates were the norm by the mid-1980s; complex configurations led to multi-block grids (logically rectangular blocks joined together) and to overlapping (chimera) grids ([[ref-venkatakrishnan-1995-icase-95-3]] 1).

**Unstructured.** Cells are identified by a number and the connectivity is stored explicitly ([[ref-mit-ocw-2-29-numerics]] L15 s.12). Simplicial meshes (triangles in 2D, tetrahedra in 3D) give flexibility around complex geometry and for adapting to shocks and boundary layers, and their generation is "far more automatable" than multi-block generation ([[ref-venkatakrishnan-1995-icase-95-3]] 1). A face-based finite-volume formulation accepts cells of any polyhedral shape, each flat face shared by two cells ([[ref-jasak-1996-phd-thesis]] 3.1-3.2).

**Delaunay triangulation.** A common way to build simplicial meshes; Watson's incremental algorithm inserts a point and retriangulates locally so the result stays Delaunay, and edge swapping can restore the Delaunay property after other changes ([[ref-venkatakrishnan-1995-icase-95-3]] 7). Regenerating the mesh with an advancing-front method after error estimation is the alternative the survey names for adaptation ([[ref-venkatakrishnan-1995-icase-95-3]] 7).

**Boundary layers.** Viscous flows need very thin cells next to walls. The Advancing-Layers Method "produces thin-layer tetrahedral grids suitable for computing viscous flows" ([[ref-venkatakrishnan-1995-icase-95-3]] 3). rcfd uses layered inflation for the same purpose (below).

**Quality matters.** The accuracy of the discretisation depends on cell shape: non-orthogonality and skewness add errors and need corrections ([[non-orthogonal-and-skewness-correction]]); severe non-orthogonality can raise the solution effort "as much as an order of magnitude", which "should be kept in mind during the mesh generation process" ([[ref-jasak-1996-phd-thesis]] 3.7.4).

## Builds on

- Floor: elementary geometry (polygons, polyhedra, volumes, normals).
- [[finite-volume-method]]: what the cells are for.

## Leads to

- [[a-posteriori-error-indicators]], [[non-orthogonal-and-skewness-correction]]
- rcfd pages: [[mesh-generators]], [[fan-passage-mesh]], [[mesh-adaptation]], [[mesh-quality]]

## How rcfd uses it

- rcfd's own generators include a tetrahedral mesher (Delaunay with exact predicates; library only; `quality_sweeps` default 0), an octree Cartesian core with prism layers (partial; sphere non-orthogonality 77.4 deg against a 70 deg target, a known miss) and boundary-layer inflation (`rcfd inflate`, exit 4 on quality errors) ([[mesh-generators]]).
- Fan passages have their own generators in `rcfd-mesh` (`fan`, `fan_mesh`, `fan_cgrid`) ([[mesh-generators]]); their registered quality gates are mostly missed ([[mesh-quality]]; [[fan-passage-mesh]]).
- Adapted meshes are hanging-node octree polyhedra ([[mesh-adaptation]]).

## Open questions

- Exact geometric predicates and octree core construction are not covered by any foundation source read for this page.
