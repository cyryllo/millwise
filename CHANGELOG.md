# Millwise changelog

Version numbers:

- **1.0.0** will be the first stable release; until then versions are `0.x.y`;
- the **second number** goes up with every release that adds features (by 10
  when a new module is added);
- the **third number** goes up for releases that only fix bugs.

## Unreleased

### Added

- **Program zero is clearly marked:** the zero symbol (a circle with two filled
  quarters), X and Y arrows and an "X0 Y0" label in the 2D view; X, Y and Z in
  the 3D view. When the zero is outside the 2D view, a marker at the edge
  shows which way it is.

## 0.1.0 (2026-11-13)

First version. **Early development: not ready for running a machine.**

### Added

- **DXF import:** lines, arcs, circles, polylines with arcs, splines, ellipses
  and blocks (with rotation and scale). Loose pieces are joined into contours
  (small gaps are closed), inches are converted to millimetres.
- **2D project view** with the drawing, the stock and the toolpaths; select
  curves by clicking or with a box (to the right: whole curves inside, to the
  left: curves crossing it).
- **Toolbar in two columns** (as in Estlcam): the groups Select, Profile,
  Pocket, Drilling, Engraving and Zero, each opening its variants. Pick a tool
  and an operation, then click curves: the first click creates the operation,
  the next ones add or remove curves.
- **Operations:**
  - **Profile** outside / inside / on the line, climb or conventional, ramp or
    plunge entry, stock to leave, **tabs**; holes are cut before the outline.
  - **Pocket** offset (from the centre out) or **zigzag** (with an angle), with
    islands.
  - **Drilling** single, **peck**, or **helical hole milling** for holes larger
    than the tool.
  - **Engraving** with the tool centre on the line, or on its left or right.
- **Editing:** move (drag or by numbers), rotate, mirror, scale, align,
  array; **program zero** at a stock corner, its centre or a picked point.
- **Tool library** shared by all projects: visual choice of the cutter type
  (flat, ball, bull nose, V, engraving, drill), a drawing with the diameter
  and flute length, flute direction, **cutting data per material** (wood, MDF,
  aluminium, acrylic) with chip load and cutting speed.
- **Machines:** work area and homing corner, controller (GRBL, FluidNC,
  LinuxCNC), feed, rapid and acceleration limits, spindle (manual, PWM, VFD)
  and its speed range, tool change (M0 pause, M6, one file per tool), coolant
  M7/M8, coordinate system G54–G59, custom program start and end. Operations
  are checked against the machine (feeds, spindle speed, work area, Z travel).
- **G-code** for GRBL / FluidNC and LinuxCNC: G2/G3 arcs (helical on ramps),
  ASCII comments, preview window and export.
- **3D visualisation** (preview only, Esc closes it): the stock, the toolpaths
  and a **simulation of material removal on a real solid**, computed in the
  background before playback, with play, seek, speed, the progress of each
  operation and **rapid moves into the material** reported before you play.
  A fast height-map engine can be chosen for very large programs.
- **Projects** (`.millwise`): the drawing, the stock, the operations and copies
  of the tools used, so a project opens on another computer.
- Icon, logo, the About window, Linux desktop file and a local install script.
