# Developing Millwise

## Build and run

```sh
cargo run --release -p millwise -- samples/test.dxf        # a drawing only
cargo run --release -p millwise -- samples/demo.millwise   # a project with operations
cargo test --workspace                                     # all tests
```

`samples/test.dxf` is made by `cargo run -p cam-io --example make_sample`;
`samples/demo.millwise` by the test `cargo test -p millwise project_roundtrip`.

Simulation timing (release build, prints the time of each stage):
`cargo test --release -p millwise sim_perf -- --ignored --nocapture`.

`MILLWISE_OPEN` opens a view or a window at start, for screenshots and
manual checks: `3d`, `simend`, `simat=0.25` (simulation at a fraction of the
program), `bottom`, `tools`, `machines`, `about` — several separated by
commas. `CAMSIM_DEBUG=1` prints the cells the fast (height map) engine could
not build exactly.

## Project layout

| Crate | Contents |
|---|---|
| `crates/cam-geom` | contours with arcs (bulge), joining pieces into contours, transforms, offset (cavalier_contours) |
| `crates/cam-io` | DXF import |
| `crates/cam-ops` | tools, operations (profile, pocket, drilling, engraving) → toolpaths |
| `crates/cam-post` | G-code for GRBL/FluidNC and LinuxCNC, machine profiles |
| `crates/cam-sim` | material removal simulation: solid engine (manifold-rust: tool sweeps as convex hulls, Boolean difference) and the fast height map engine; rapid-move collision checks |
| `app` | the window (egui + wgpu): toolbar, 2D and 3D views, operations, tool library, machines, projects |
| `assets` | icon (SVG, PNG, ICO), logo, Linux `.desktop` file and local install script |

User data: `~/.config/millwise/` on Linux, `%APPDATA%\millwise\` on Windows
(`tools.json`, `machines.json`). On the first start after the rename they are
copied from the old `camsoft` folder; projects saved as `.camsoft` still open.

## Versions and releases

Version numbers (as in Kerfly):

- **1.0.0** will be the first stable release; until then versions are `0.x.y`;
- the **second number** goes up with every release that adds features (by 10
  when a new module is added);
- the **third number** goes up for releases that only fix bugs.

Every code release raises `version` in the workspace `Cargo.toml` and gets a
section in [CHANGELOG.md](CHANGELOG.md) (written for users: what changed for
them). Changes to the documentation alone get no new version. A release is a
tag `vX.Y.Z`.

Before a release, refresh the list of bundled libraries:
`python3 scripts/third_party.py` (writes `THIRD-PARTY.md`).

## License

GPL-3.0-or-later. New dependencies must have a licence compatible with
GPL-3.0 (MIT, Apache-2.0, BSD, Zlib, ISC, MPL-2.0 are fine).
