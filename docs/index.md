---
layout: default
lang: en
other_lang: pl/
wide: true
description: "Millwise is free CAM software for 3-axis CNC routers and mills. From DXF drawings and STL models to G-code for GRBL, FluidNC and LinuxCNC, with 2D and 3D machining and a simulation on the graphics card. For Linux."
---
{% assign img = '/img/en/' | relative_url %}

<div class="hero">
  <img src="{{ '/assets/logo.svg' | relative_url }}" alt="Millwise logo">
  <div>
    <h1>Millwise</h1>
    <p>Free CAM for 3-axis CNC routers and mills. From a drawing or a 3D model to G-code in one window.</p>
    <p class="badges"><span>Linux</span><span>Free &amp; open source</span><span>GRBL · FluidNC · LinuxCNC</span><span>2D and 3D machining</span><span>English · Polski</span></p>
  </div>
</div>

<div class="buttons">
  <a class="button primary" href="#download">Download</a>
  <a class="button" href="{{ '/guide.html' | relative_url }}">User guide</a>
  <a class="button" href="{{ site.repo }}">Source code</a>
</div>

<div class="notice"><strong>Early development.</strong> Millwise changes every day. The first G-code runs on a real router (FluidNC) went well, but the dimensions have not been measured yet and the 3D machining has not been checked on a machine. Always check the G-code before you run it, and use the program at your own risk.</div>

<div class="shot"><a href="{{ img }}main.png"><img src="{{ img }}main.png" alt="Millwise main window: operations on the left, the drawing with toolpaths in the middle, the machine and stock on the right"></a></div>

<p class="lead">Millwise turns <strong>DXF drawings</strong> and <strong>STL models</strong> into toolpaths and writes G-code for <strong>GRBL / FluidNC</strong> and <strong>LinuxCNC</strong>. Pick a tool and an operation, click the curves, watch the whole job in the simulation and export the program: profiles, pockets, holes, engraving and V-carving in 2D, and roughing and finishing of 3D models.</p>

<div class="stats">
  <div><strong>5</strong><span>2D operations: profile, pocket, drilling, engraving, V-carving</span></div>
  <div><strong>4</strong><span>3D operations: roughing, finishing, rest, cut-out</span></div>
  <div><strong>7</strong><span>tool types, 16 tools in the basic set</span></div>
  <div><strong>GPU</strong><span>material simulation on the graphics card</span></div>
  <div><strong>3</strong><span>controllers: GRBL, FluidNC, LinuxCNC</span></div>
</div>

## What Millwise can do

<div class="showcase">
  <div class="shot-col small"><a href="{{ img }}left-panels.png"><img loading="lazy" src="{{ img }}left-panels.png" alt="The toolbar and the operations panel with the settings of a profile"></a></div>
  <div class="text-col">
    <h3>2D machining from DXF drawings</h3>
    <ul>
      <li><strong>Profile</strong> outside, inside or on the line, with tabs, a ramp or arc lead-in, stock to leave and a finishing pass with its own feed.</li>
      <li><strong>Pocket</strong> offset, zigzag or adaptive (HSM: small even step-over, rounded loops), with islands and a finishing pass.</li>
      <li><strong>Drilling</strong> single, peck or helical hole milling; <strong>engraving</strong> on, left or right of the line; <strong>V-carving</strong> with the depth following the width of the shape.</li>
      <li><strong>Corners (dog-bones)</strong> so square parts fit, <strong>text</strong> from any font turned into curves.</li>
      <li>Drag operations to change the order; one click puts operations with the same tool together.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col"><a href="{{ img }}model3d.png"><img loading="lazy" src="{{ img }}model3d.png" alt="The 3D model tab: a model in the stock with roughing and finishing toolpaths"></a></div>
  <div class="text-col">
    <h3>3D machining of STL models</h3>
    <ul>
      <li>Open an STL file — or a <strong>photo</strong>, which becomes a relief — the model sits in the stock (units, scale, 90° rotations, stock left above it) and has its own <strong>3D model</strong> tab.</li>
      <li><strong>Roughing</strong> in Z levels around the model, with extra levels just above flat areas.</li>
      <li><strong>Finishing</strong> with parallel passes or <strong>waterlines</strong> for steep walls; <strong>rest finishing</strong> only where a bigger cutter could not reach.</li>
      <li><strong>Cut-out</strong> along the model silhouette with tabs; the silhouette and the outline at any height also as curves for 2D operations.</li>
      <li>Undercut warning; 3D toolpaths are computed in the background, the window keeps working.</li>
    </ul>
  </div>
</div>

<div class="showcase">
  <div class="shot-col"><a href="{{ img }}sim3d.png"><img loading="lazy" src="{{ img }}sim3d.png" alt="Simulation on the graphics card: finishing half way over the roughed model"></a></div>
  <div class="text-col">
    <h3>See the job before the machine does</h3>
    <ul>
      <li><strong>Solid (GPU)</strong>: the graphics card removes the material move by move, smooth for programs of any size.</li>
      <li><strong>Solid (CPU)</strong> on weak graphics: an exact solid computed by the processor.</li>
      <li><strong>Toolpaths</strong>: the target model with the layer being cut in yellow, instant at any size.</li>
      <li>Play, pause, seek, jump to an operation; <strong>rapid moves into the material</strong> are checked for the whole program.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col"><a href="{{ img }}tools.png"><img loading="lazy" src="{{ img }}tools.png" alt="The tool library with tools grouped by type and the tapered ball cutter selected"></a></div>
  <div class="text-col">
    <h3>A tool library shared by all projects</h3>
    <ul>
      <li>Flat, ball, tapered ball, bull-nose, V, engraving cutters and drills, each with a drawing and its sizes.</li>
      <li>Cutting data per material (wood, MDF, aluminium, acrylic): pick the material of the stock and the feeds follow.</li>
      <li>A basic set of flat, ball and V cutters and drills in 3, 4 and 6 mm to start with.</li>
      <li>Projects keep copies of the tools they use, so they open the same on another computer.</li>
    </ul>
  </div>
</div>

<div class="showcase">
  <div class="shot-col"><a href="{{ img }}machines.png"><img loading="lazy" src="{{ img }}machines.png" alt="The machines window: work area, homing corner, controller and limits"></a></div>
  <div class="text-col">
    <h3>Your machines, your G-code</h3>
    <ul>
      <li>Work area, homing corner, feed and rapid limits, spindle range and spin-up time, tool change (pause, M6 or one file per tool), coolant, probe.</li>
      <li>Your own program start and end, line numbers, decimals, G2/G3 arcs or lines.</li>
      <li>Every operation is checked against the machine: work area, depth, feeds.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col small"><a href="{{ img }}automation.png"><img loading="lazy" src="{{ img }}automation.png" alt="The automation window: an operation for each layer, corners, tabs and order by tool"></a></div>
  <div class="text-col">
    <h3>Less clicking</h3>
    <ul>
      <li><strong>Automation</strong>: operations for all free curves by layer, corners, tabs by perimeter and order by tool in one step.</li>
      <li><strong>Two-sided machining</strong> with dowel pin holes and a mirrored side B in its own G-code file.</li>
      <li>Move, rotate, mirror, scale, align, array, cut, copy and paste; <strong>undo and redo</strong> with a history bar.</li>
      <li>Several DXF files, models and projects in one project; everything is kept in one <code>.millwise</code> file.</li>
    </ul>
  </div>
</div>

<h2 id="download">Download</h2>

<p>Free, no account, nothing to install. The <a href="{{ site.repo }}/releases/latest">latest release</a> has an AppImage for Linux:</p>

<div class="platforms">
  <div class="platform">
    <h3>Linux <span class="tag">x86_64</span></h3>
    <p><code>Millwise-…-x86_64.AppImage</code></p>
    <p>Runs on Debian 12 / 13, Ubuntu 22.04 or newer and other distributions of that age. Make it executable and run it.</p>
  </div>
  <div class="platform">
    <h3>Windows, macOS</h3>
    <p>Not planned for now: Millwise is made for Linux on purpose.</p>
  </div>
</div>

```sh
chmod +x Millwise-*-x86_64.AppImage
./Millwise-*-x86_64.AppImage
```

<div class="buttons">
  <a class="button primary" href="{{ site.repo }}/releases/latest">Go to the latest release</a>
  <a class="button" href="{{ '/guide.html' | relative_url }}">Read the user guide</a>
</div>

## Questions

<div class="faq">
<details><summary>Which machines does Millwise work with?</summary><p>3-axis CNC routers and mills with GRBL, FluidNC (GRBL-compatible G-code) or LinuxCNC. The G-code was tried first on a router with a FluidNC controller.</p></details>
<details><summary>Which files can it open?</summary><p>DXF drawings (lines, arcs, circles, polylines, splines, ellipses, blocks), STL models (binary and text) and its own <code>.millwise</code> projects. Several files can be added to one project.</p></details>
<details><summary>Is it really free?</summary><p>Yes: free software under the GPL-3.0 licence, no account, no ads, no limits. The source code is on <a href="{{ site.repo }}">GitHub</a>.</p></details>
<details><summary>Does it send anything over the internet?</summary><p>No. Millwise works offline and sends nothing anywhere.</p></details>
<details><summary>The window does not open, or it is very slow.</summary><p>Millwise picks OpenGL by itself on NVIDIA cards and when the default graphics fail. If the window still does not open, start it with <code>--gl</code>. On a weak graphics card turn off View → Material simulation on the graphics card (GPU).</p></details>
<details><summary>Is Millwise in my language?</summary><p>English and Polish. It follows the language of your system; change it in View → Język / Language.</p></details>
</div>

## Status

**Millwise 0.7 is an early test version.** It already does 2D and 3D
machining, but it changes every day and has not been tested enough on real
machines. Planned next: combined steep and shallow
finishing, spiral finishing, engraving on 3D surfaces. See the
[changes]({{ site.repo }}/blob/main/CHANGELOG.md) and the
[to-do list]({{ site.repo }}/blob/main/TODO.md) (in Polish).

## Help and feedback

Found a problem or have an idea? Open an [issue]({{ site.repo }}/issues),
in English or Polish. Testing with your machine and telling how it went is
the best help at this stage.
