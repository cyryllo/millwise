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
program), `bottom`, `tools`, `machines`, `about`, `keys` (keyboard shortcuts),
`op=2` (select the third operation) — several separated by commas.
`MILLWISE_SCREENSHOT=file.ppm` saves a picture of the program's own window
after a moment and closes it (convert with `convert file.ppm file.png`). `CAMSIM_DEBUG=1` prints the cells the fast (height map) engine could
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
| `assets` | icon (SVG, PNG, ICO) and logo |
| `packaging/linux` | AppImage build (Dockerfile, scripts), `.desktop` file, AppStream metadata, local install script |

Millwise targets Linux only for now; Windows is not built or tested.

User data: `~/.config/millwise/` (`tools.json`, `machines.json`). On the first start after the rename they are
copied from the old `camsoft` folder; projects saved as `.camsoft` still open.

## Linux AppImage

Built on this computer with Docker:

```sh
packaging/linux/build-appimage-docker.sh            # the last commit (HEAD)
packaging/linux/build-appimage-docker.sh v0.1.0     # a tag or another commit
```

The program is compiled in an Ubuntu 22.04 container (`packaging/linux/Dockerfile`,
Rust pinned to the version used here) so it needs only glibc 2.35: it runs on
Debian 12 and 13 and Ubuntu 22.04 or newer. Built straight on a newer system it
would not start on older ones. The script builds from a clean copy of the commit
(`git archive`), so commit first. Crates and build results are kept in
`target/docker` for the next build; `appimagetool` is downloaded once to
`target/tools`. The result goes to `dist/Millwise-<version>-x86_64.AppImage` and is
checked with `--version`. `packaging/linux/build-appimage.sh` alone packs an
already built program.

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
