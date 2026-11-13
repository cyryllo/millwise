# Millwise roadmap

## Now: 2D / 2.5D milling — polishing and fixes

Before any new module, the existing 2.5D operations get corrected and finished.

## Later: STL and 3D machining (planned, not started)

The plan keeps reusing what already exists: the 2.5D offset and pocket engine,
the tool shapes and the solid simulation, which accepts any 3D toolpath.

1. **STL import and placement** (version 0.10.0, a new module)
   - binary and ASCII STL (`stl_io`), units mm / inch;
   - placing the model: rotate a chosen face onto the table, scale, position
     in the stock, choose Z0 (top of the model or of the stock); stock
     suggested from the model size plus a margin;
   - the model as a solid in the 3D view and as its top-view shadow in 2D.
2. **STL as a source of curves for 2.5D operations**
   - the model's outline seen from the top (silhouette) → profile cut-out;
   - a cross-section at a chosen Z → pocket floors, steps;
   - the curves go into the existing profile, pocket and tab tools.
3. **3D roughing (Z-level)**
   - at each Z: the area the cutter may not enter = the outline of the part of
     the model above that level, grown by the tool radius and the allowance;
   - the rest of the stock is cleared with the existing pocket engine (islands,
     ramps); undercuts are skipped automatically.
4. **3D finishing**
   - **parallel (raster), drop-cutter:** along lines at a chosen angle the tool
     is lowered onto the mesh until it touches; per triangle the vertex, facet
     and edge tests for flat, ball, bull nose and V tools (our own
     implementation; tool profiles as in the simulation); a bounding-volume
     tree for speed;
   - **waterline** (constant Z) for steep walls; later steep/shallow
     combined;
   - path simplification (collinear points within a tolerance) to keep the
     G-code small.
5. **Rest machining** (what a larger tool left) and **gouge check** against the
   STL (colour where stock is left or the model is cut into).

Toolbar: a new **3D** group (Roughing, Parallel, Waterline, later Rest) and
**Outline / Section** for 2.5D.

Every step gets tests on shapes with analytic results (hemisphere, cone,
steps) and drop-cutter checked against a brute-force reference, as was done
for the simulation.

## Other ideas

Adaptive (HSM) pocketing, V-carving, GitHub Actions builds (AppImage),
Linux packages from CI, update check, double-sided
machining.
