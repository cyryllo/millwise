# Millwise changelog

Version numbers:

- **1.0.0** will be the first stable release; until then versions are `0.x.y`;
- the **second number** goes up with every release that adds features (by 10
  when a new module is added);
- the **third number** goes up for releases that only fix bugs.

## Unreleased

### Added

- **Machining order:** the operations on the left are numbered in the order
  they are cut and can be **dragged** to a new place (a line shows where the
  operation will land); the change is one step in the history.
- **Change an array, a scale or a rotation later:** a click on its icon in the
  history bar opens its values (columns, rows and gaps; percent; angle); a new
  value rebuilds it at once and the history keeps one step. Editing an older
  one warns that the later steps will be dropped.

- **Finishing pass** for profiles and pockets: the roughing passes leave a
  small allowance (0.3 mm to start with), and at the end one pass at full
  depth takes it off to size, for smoother walls. Tabs are kept.
- **Arc lead-in and lead-out** for profiles: the tool goes down in the waste,
  away from the wall, reaches the wall along a tangent arc, goes round and
  leaves the same way, so the entry leaves no mark. If the arc does not fit
  (a small hole), its radius is reduced or the tool plunges as before.

### Changed

- **Shorter rapid moves inside an operation:** each contour starts at the
  corner nearest to where the tool is (with lead-in: in the middle of the
  nearest edge), and open contours are cut from the nearer end.
- Clicking an empty spot of the drawing also closes the box of an editing
  tool (array, move, rotate, scale, align) and the box for changing an older
  step from the history.
- The boxes of the editing tools grow to the right with longer values instead
  of wrapping their fields onto a second line.
- **„Podgląd 3D” is now „Symulacja”** — the tab at the top, the menu and the
  hints: it shows the machining simulation, not a 3D preview of a model. Its
  button moved from the toolbar (which now holds only drawing tools) to the
  operations panel, as a green „Symulacja obróbki” button above the G-code
  buttons: operations, then simulation, then export. The two G-code buttons
  are side by side, each half as wide, in their own colours (preview amber,
  export blue). The same two buttons are at the bottom of the simulation
  panel, so the G-code can be checked or saved right after the simulation.
- The window opens maximized (1600 × 1000 when restored).
- Layers on the right are folded at start, like the list of curves.

## 0.2.0 (2026-11-13)

First version to download: an AppImage for Linux. **Still early development:
not ready for running a machine.**

### Added

- **AppImage for Linux** (Debian 12 / 13, Ubuntu 22.04 or newer): one file,
  nothing to install; made with `packaging/linux/build-appimage-docker.sh`.
- `--version` and `--help` on the command line.
- **Program zero is clearly marked:** the zero symbol (a circle with two filled
  quarters), X and Y arrows and an "X0 Y0" label in the 2D view; X, Y and Z in
  the 3D view. When the zero is outside the 2D view, a marker at the edge
  shows which way it is.
- **Undo and redo** (Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z) for every change to the
  curves, operations, stock and G-code settings. A drag or a slider move is
  one step.
- **History bar** at the bottom of the 2D view: one icon per action, with a
  small mark for added / removed / edited. Steps that belong together share
  one icon with their number in the corner — creating an operation, adding
  its curves and setting its depth, or several moves in a row. The pointer
  over an icon lists the steps; a click goes back to that state and, for an
  operation, opens its settings. Ctrl+Z still undoes one step at a time.
  Widok → Historia zmian hides the bar.
- **Clicking an empty spot of the drawing** finishes the operation being made
  and closes its settings; it opens again from the list on the left or from
  the history bar.
- **Cut, copy and paste curves** (Ctrl+X, Ctrl+C, Ctrl+V); pasted curves land
  under the mouse pointer. **Delete** removes the selected curves (operations
  forget them); with no curves selected it removes the selected operation.
- **Clearer operations panel:** the list of operations and the settings of
  the selected one are separate; each operation in the list shows its icon,
  name, tool and depth. **Depth and step down** (for drilling: depth and
  peck retract) are at the top of the settings in a highlighted box, with the
  number of passes; the rest is grouped (operation, tool and cutting), and
  the rarely changed heights are folded away.
- The right panel is divided into boxes: stock, layers, curves. On top is the
  **active machine** in its own box, with a picture, its controller and work
  area, a list to switch machines and a button to its settings (it used to be
  a small label in the menu bar).
- **Add files to a project** (Plik → Dodaj DXF / projekt…, Ctrl+I, or drop
  files on the window; several at once): the new drawing is placed next to
  the existing ones and selected, ready to move. An added project brings its
  operations and tools along. Layers with the same name are joined. Several
  files can also be given on the command line: the first one is opened, the
  others are added.
- **Elements list** on the right: the files the project is made of, each with
  its number of curves; a click shows its size, layers and operations, with
  buttons to select or remove it. The right panel is more compact: layers and
  the list of curves fold away.
- **Machine types with pictures:** gantry, moving table, column mill and
  sheet router; chosen in the machine settings and shown in the list of
  machines and in the machine box.
- **Edycja** menu with these commands, **Ctrl+Shift+S** for "save as", and a
  list of all keyboard shortcuts under Pomoc → Skróty klawiszowe.

## 0.1.0 (2026-11-13)

First version. **Early development: not ready for running a machine.**

### Added

- **DXF import:** lines, arcs, circles, polylines with arcs, splines, ellipses
  and blocks (with rotation and scale). Loose pieces are joined into contours
  (small gaps are closed), inches are converted to millimetres.
- **2D project view** with the drawing, the stock and the toolpaths; select
  curves by clicking or with a box (to the right: whole curves inside, to the
  left: curves crossing it).
- **Toolbar in two columns:** the groups Select, Profile,
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
