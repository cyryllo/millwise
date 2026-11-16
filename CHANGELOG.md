# Millwise changelog

Version numbers:

- **1.0.0** will be the first stable release; until then versions are `0.x.y`;
- the **second number** goes up with every release that adds features;
- the **third number** goes up for releases that only fix bugs.

## Unreleased

### Added

- **Relief from a photo.** Open (or add) a JPG, PNG or BMP image and it
  becomes a 3D model: bright places higher, dark ones lower — or the other
  way round with „Dark is higher”. The model card shows the photo and its
  settings: width (the height follows the proportions of the photo), depth
  between the darkest and the brightest place, base under the lowest point,
  smoothing (removes noise and the grain of the photo) and mesh spacing.
  From there it is machined like an STL model: roughing, finishing, rest
  finishing, cut-out, simulation. The project keeps the photo itself (in
  grey, at most 1600 px on the longer side) and its settings, not the
  mesh, so it stays small and the relief can be changed later.

### Fixed

- **A 3D operation whose computation failed no longer stays „computing…”
  forever** (starting again and again in the background); it shows a
  warning and an empty toolpath instead.

### Changed

- **Toolpaths in the 2D view are drawn by the graphics card.** They are sent
  to the card once, after the toolpaths change or another operation is
  selected; moving and zooming the view only redraws them. The tool-width
  band of the selected operation is drawn once into its own picture, so even
  where hundreds of dense passes overlap it costs nothing per frame. Dense
  3D finishing (e.g. 230 000 moves) now pans and zooms smoothly, at the
  refresh rate of the screen instead of about 40 ms per frame, and the gap
  grows with the size of the program. Overlapping passes no longer add up
  into a solid colour: the band stays see-through, so the toolpath lines
  inside it remain visible.

## 0.7.1 (2026-11-15)

### Added

- **Tapered ball cutters** — sides like a V-bit with a small ball at the
  tip, ideal for 3D finishing. A new type in the tool library (taper angle
  and ball diameter, with a drawing), supported in 3D toolpaths (the cutter
  dropped onto the model with its real shape), in every simulation view and
  in the scallop height shown for parallel finishing. The basic set gets one
  (R0.25, 20°, Ø3.175); an existing library receives it automatically, once.

### Changed

- **Tool lists grouped by type.** The tool library window and the drop-down
  lists of tools (for new operations, in the operation settings, for rest
  finishing) are grouped:
  flat, ball, bull-nose, V, engraving, drills — each group with a heading
  and sorted from the smallest diameter.
- **New tool drawings**: metal shank as wide as the cutter, a carbide
  cutting part with polished shading and spiral flutes (drills in gold), and
  a coloured ring on the shank for each tool type — in the tool library, the
  tool lists and the operations panel.
- **The tool comes first in the operation settings**, above the depth: it is
  what is usually chosen or changed first. Feeds and speed stay below, under
  „Cutting”.

## 0.7.0 (2026-11-15)

### Added

- **STL models (first step of 3D machining).** Open an STL file (binary or
  text) like a drawing — Plik → Otwórz, Dodaj, drag and drop or the command
  line — or add it to a project. The „3D model” box on the right shows its
  size and sets the file units (mm or inches), the scale, 90° rotations about
  X, Y and Z and the stock left above the model; the stock thickness follows
  the model's height. The model sits with its top at Z0 (or lower by the stock
  above it) and the program zero at the stock corner as usual. The 2D view
  shows it from the top, shaded by height; the simulation shows it as a
  see-through solid (with a switch). The project file keeps the model inside,
  so it opens without the STL. Changes to the model can be undone. A model
  with undercuts (places with air under the model, which a cutter coming
  from the top cannot reach) gets a warning with their area.

- **Curves from a 3D model.** „Silhouette to curves” adds the outline of the
  model seen from the top, and „Outline above Z to curves” the area where the
  model is higher than the given Z, as curves on a new MODEL layer. They work
  with all 2D operations — for example a profile that cuts the part out, or a
  pocket around an island down to that depth.

- **„3D model” tab.** With an STL model loaded, a third tab sits between
  „2D project” and „Simulation”: the model as a shaded solid in the stock,
  with the toolpaths (the selected operation highlighted), turned and zoomed
  with the mouse. The operations and the panels on both sides stay as in the
  2D tab. Opening an STL file shows this tab at once. Its toolbar has only
  what works there — the „3D” group and „Zero”; the 2D tools are in the 2D
  tab. A 3D operation is added with one click on its variant (the whole
  stock, or the selected closed curves as boundaries). Clicking an empty spot
  in the view closes the operation settings, as in 2D. „Cut-out” in the same
  group cuts the part free along the model silhouette: it makes the
  silhouette curve and an outside profile with tabs through the stock, at the
  end of the program.

- **3D roughing.** A new „3D” group in the toolbar. „Roughing” clears the
  stock around the model in Z layers, leaving the chosen stock on the model
  (sideways and downwards). Each layer is cleared like a pocket, with the
  same strategies (offset, zigzag, HSM), ramp entries and linking. Extra
  layers are added just above the model's flat areas, so a full step of
  material is not left on them. Without curves the operation covers the whole
  stock; clicked closed curves limit it to their inside. By default it goes
  down to the lowest surface seen from the top, so the stock is not cut
  through. The result is kept and only recalculated when something it
  depends on changes; 3D toolpaths are computed in the background, so the
  window keeps working (the operation shows „computing…” until it is ready;
  G-code export and the simulation wait for it).

- **3D finishing.** „Finishing” in the 3D group runs parallel passes over the
  model: the cutter is dropped onto the surface at close points (more of them
  where the surface bends), so it touches the model without cutting into it.
  Works with flat, ball, bull-nose and V cutters (a ball cutter is chosen when
  there is one); pass spacing with the scallop height for a ball cutter, pass
  direction, an optional second pass across, and stock left on the model.
  Neighbouring passes are joined along the surface without lifting.
  A second strategy, „Waterline”, goes around the model at constant heights
  (level spacing as a share of the cutter diameter, climb milling), for steep
  walls that parallel passes leave rough; selected curves limit it too.
  „Rest only” (with either strategy) machines only where a bigger tool chosen
  from the library could not reach — narrow valleys and inside corners —
  with a minimum rest thickness; the rest of the model is skipped.

- **English user interface.** Widok → Język / Language switches between
  Polski and English at once, without a restart; the choice is remembered.
  On the first start Millwise follows the system language (Polish for a
  Polish system, English otherwise). Operation warnings and the comments in
  the G-code follow the chosen language too. Material names saved in tool
  and project files stay as they are and are only shown translated.

- **Basic tool set.** The starting tool library now has flat, ball and V
  (60°) cutters and drills in 3, 4 and 6 mm (15 tools). An existing library
  gets the missing ones automatically, once, on the first start after the
  update (the user's own tools stay as they are, and a deleted one does not
  come back); „Add the basic set” in the tool library window adds them again
  on request.

### Changed

- **Faster solid simulation, also late in long programs.** The stock is
  split into tiles and each tool move only changes the tiles it touches;
  the others are taken from memory. Before, every frame recalculated the
  whole solid, so playback slowed down more and more as the material got
  more detailed (e.g. after 3D finishing: about 5 frames per second at the
  end, now 15–30+). Keyframes for seeking share unchanged tiles and are kept
  within a memory budget.
- **Very dense toolpaths** (e.g. 3D finishing every 0.1 mm with a fine
  engraving cutter) no longer stall the simulation. Dragging the time slider
  shows a quick preview and the exact solid follows when the slider is
  released. Without the GPU, a program with more than 40 000 moves, or a
  solid that grows beyond 2 million triangles, is shown as „Toolpaths” (with
  a note; „Solid (CPU)” can still be chosen by hand). 3D parallel finishing is computed on all processor
  cores. The finishing pass spacing is entered in millimetres.

- **„Toolpaths” simulation mode**: no material removal is computed — the
  target model is shown as a solid, the toolpaths done so far in the
  operation colour, the layer being cut now (the current Z level, or the
  current line of parallel finishing) in yellow, and the rest dimmed. Only
  the operation being run is shown — e.g. the roughing passes disappear
  during finishing. Playback and seeking are instant for any program size.

- **Material simulation on the graphics card (GPU engine).** Each tool move
  is drawn by the graphics card into a height texture as the exact shape of
  the pass (flat, ball, bull-nose and V cutters), keeping the lowest point,
  and the material surface is shown straight from that texture — no meshes
  are built by the processor. A frame only adds the new moves, so playback
  and seeking stay smooth for programs of any size, also through OpenGL.
  Accuracy from 1024 to 8192 pixels along the stock. On by default; View ›
  „Material simulation on the graphics card (GPU)” turns it off for weak
  graphics cards (the processor then computes the solid).

- **Simulation view: „Solid” and „Toolpaths”.** „Solid” shows the material
  being removed — by the graphics card when the GPU setting is on (the
  default, „Solid (GPU)”), otherwise by the processor („Solid (CPU)”, with a
  note that it is slow for large programs). Rapid moves into the material are
  checked in the background for every view. The „Fast” height-map engine is
  no longer offered — its surface was too coarse for 3D work.

### Fixed

- **Crash in the simulation of very detailed material** („Buffer size … is
  greater than the maximum buffer size”). The material surface now goes to
  the graphics card in parts that fit its limits, and a graphics error no
  longer closes the program.
- **Stripes on finished surfaces** when the 3D model is shown in the
  simulation: where the material touches the model (no stock left), the
  see-through model no longer shows through it.

## 0.5.0 (2026-11-15)

### Added

- **Corners (dog-bones) for parts that fit inside each other:** in inside
  profiles and pockets the cutter can go into every sharp inner corner along
  its diagonal until its edge touches the corner, leaving a round relief, so
  a square part goes all the way in. Switch it on in the operation settings
  („Uszka w narożnikach”) or with the new tool Profil → Narożniki: click a
  curve of an inside profile or a pocket to switch the corners on or off.
  Rounded corners and corners of the part itself are left as they are.
- **Ramp entry on open lines** (profile on a line, engraving): the cutter goes
  down along the line at the set angle instead of plunging — back and forth
  on a short line — then returns to the start at full depth so no wedge of
  material is left, and cuts the whole line.
- **Automation** (Narzędzia → Automatyzacja…, or the button in the operations
  panel): one window that can create operations for all curves that have
  none — chosen per layer (drilling, pocket, outside or inside profile,
  engraving, skip, or „from the shape”; suggested from the layer name, e.g.
  OTWORY → drilling, and remembered) — switch corners on in all inside
  profiles and pockets, set the number of tabs by the part's perimeter and
  order the operations by tool with the cut-out last. A summary shows what
  will happen; everything is one step in the history. Holes without a drill
  of their size are milled along a helix when they are larger than the
  cutter.
- **Two-sided machining** (box on the right): switch it on and choose the
  flip axis (left/right or front/back; the axis is drawn on the drawing).
  „Dodaj otwory na kołki” adds two dowel holes on the axis, outside the part,
  through the stock and into the spoilboard, as the first operation of side A
  (with a drill of that size, or milled along a helix). Each operation can be
  set to side A or B; side B is mirrored about the axis for the flipped board
  — you draw everything as seen from the top, and the dowels land in the
  same holes after flipping. The G-code is saved as two files, „strona_A”
  and „strona_B”; the simulation has a side switch.
- **HSM pocket** (Kieszeń → Adaptacyjna, or „HSM” in the pocket settings): a
  small, even step-over (15 % of the diameter to start with) and loops with
  corners rounded by at least the cutter radius, so the cutter never turns
  sharply or goes full width into a corner; loops are linked without lifting
  where possible, and at every depth a pass along the walls takes the
  corners out. In narrow places the loops are not rounded, so nothing is
  left. This is a simplified high-speed strategy, not full constant
  engagement (trochoidal) milling.
- **V-carving** (Grawer → V-carve): a V or engraving cutter follows the
  middle of each shape with a changing depth — deep where the shape is wide,
  shallow in narrow places, coming up to nothing in sharp corners — so the
  cone always touches both walls. The set depth is a limit (wider areas get a
  flat bottom, with a note); deep carving goes down in steps. Letter holes are
  left standing. Straight stretches of the path are merged, so the G-code is
  short and the simulation quick.
- **Text** (Grawer → Tekst): type a text (several lines with Enter), set the
  height of capital letters, the letter spacing and the font — Ubuntu or the
  fixed-width Hack built in, or any .ttf / .otf file — and click the drawing
  to place it; a preview follows the pointer. The letters become ordinary
  curves on a TEKST layer, ready for engraving or a pocket, and appear as an
  element in the list on the right.
- **Measuring** (Zaznacz → Zmierz): click two points — they snap to corners
  and circle centres — to see the distance on the drawing and the length,
  ΔX, ΔY and angle in the tool box; a third click starts a new measurement.

## 0.4.0 (2026-11-14)

### Added

- **Feed of the finishing pass:** set as a percent of the roughing feed
  (70 % to start with, the value in mm/min is shown next to it), for profiles
  and pockets.
- **Fewer tool changes:** under the list of operations Millwise shows how many
  tool changes the program has and, when it can be fewer, a button that puts
  operations with the same tool next to each other (tools in the order they
  are first used). Outside profiles — cutting the part out — stay at the end,
  so the part is not freed before the rest is machined. One step in the
  history.

### Fixed

- **The window did not open on Linux with NVIDIA cards** (only with OpenGL
  forced by hand). Millwise now draws through OpenGL when it finds the NVIDIA
  driver, and if the default graphics (Vulkan) cannot open the window on
  another computer, it starts itself again with OpenGL and remembers that for
  the next starts. `--gl` and `--vulkan` choose by hand; Pomoc → O programie
  shows which graphics are used.

### Changed

- **An opened DXF drawing always lands at the program zero:** the bottom left
  corner of the stock is at X0 Y0, whatever coordinates the drawing has in
  the file (before, a drawing made around its centre opened with the zero in
  the middle). Saved projects open where they were saved.
- **Added files are laid out in rows:** next to the existing elements, to the
  right while they fit in the machine's work area, then in a new row above,
  never on top of another element.

## 0.3.0 (2026-11-14)

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
