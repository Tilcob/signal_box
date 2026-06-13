# signal_box — "Stellwerk"

**Stellwerk** is a Zachlike about railway signaling, built with [Bevy](https://bevyengine.org/) 0.18.
You lay track, place switches and signals, and wire up the routing so that an
entire **timetable runs on its own** — every train reaches the station of its
destination collision-free, deadlock-free, and on time, **without a single live
intervention**. Build → simulate → watch where it jams → debug → optimize.

> **Design:** the single source of truth is [GDD.md](GDD.md) (in German).
> Where code and README disagree with the GDD, the GDD wins.

## Getting started

```sh
cargo run                                      # with dev tools (default)
cargo build --release --no-default-features    # release without dev bloat
cargo test                                     # workspace tests (sim, codes, i18n)
```

`STELLWERK_WINDOWED=1 cargo run` starts in a 1600×900 window instead of
borderless fullscreen — handy for screenshots and a second monitor.

## The loop

Each level cycles through four strictly separated phases (the Zachlike
contract — the built layout *is* the program, nothing is timed by hand):

1. **Edit** — lay track by dragging, drop switches and signals, configure
   switch routing. Unlimited time, unlimited undo/redo. Live validation: faulty
   pieces glow, the start button stays locked while errors exist.
2. **Run** — start the simulation; the layout is frozen. Speed is pause / 1× /
   4× / 16× with single-tick stepping. The run ends in success, collision,
   deadlock, misrouting, or stall.
3. **Debug** — the result screen names the place, the trains involved, and the
   responsible switch. Failure is information, not a dead end.
4. **Optimize** — after the first success the level is solved; better scores
   stay possible forever. Three medal axes turn every level into a puzzle.

## Building blocks

| Block | Function | Material |
|-------|----------|----------|
| **Track** | a connection on the grid (straight, 45° diagonal, curve) | 1 / segment |
| **Switch** | a branch carrying its routing config: default branch + optional per-destination rules | 4 |
| **Block signal** | holds while the following block section is occupied | 2 |
| **Chain signal** | holds until every block up to the next block signal is free (no stopping inside a junction) | 3 |

## Scoring

Three axes, each with a par value the level awards a medal for beating:

- **Throughput** — tick of the last arrival (smaller is better).
- **Material** — build cost of *your* layout (designer-fixed track is free).
- **Punctuality** — total lateness across all trains versus their due ticks.

Best values are tracked per level across multiple solution slots.

## Controls

### Edit mode

| Input | Action |
|-------|--------|
| `1` | Track tool (drag to draw a run) |
| `2` | Switch tool |
| `3` | Block signal |
| `4` | Chain signal |
| `5` / `X` | Demolish |
| `Q` | Select (click a switch to open its routing panel) |
| `R` | Rotate the placement variant |
| `6` / `7` | Source / sink (sandbox only) |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo (layout-aware, QWERTZ-safe) |
| `Space` / `Enter` | Start simulation |
| `Esc` | Back to route select (build is autosaved) |

### Run mode

| Input | Action |
|-------|--------|
| `Space` | Pause / resume |
| `1` / `2` / `3` | Speed 1× / 4× / 16× |
| `T` | Single tick while paused |
| Left click on a train | Show its destination and waiting state |
| `Esc` | Back to the desk (Edit) |

## Content

- **Campaign:** 15 hand-built levels across four chapters (`assets/levels/*.ron`),
  each introducing a new *situation* rather than a new part. Reference
  solutions live under `assets/levels/solutions/`.
- **Sandbox:** place your own sources, sinks and schedule and build a puzzle
  from scratch.
- **Sharing codes** (`stellwerk_codes`): solutions and custom levels travel as
  short `SW1-…` text codes — no server, no workshop. Export/import lives in the
  level select and result screens.
- **Languages:** English and German, switchable in the level select; the choice
  persists with your progress. Strings live in `assets/i18n/{en,de}.ron`.

Progress saves to the platform config directory (via `directories`); a corrupt
or locked save is preserved as `.bak` rather than clobbered.

## Architecture

A Cargo workspace: the simulation is engine-agnostic, the Bevy app is pure
frontend on top of its public API.

```
crates/
├── stellwerk_sim/       # deterministic simulation core — no engine, no rendering
│   ├── grid · graph     #   cell grid, track network as a graph
│   ├── layout · level   #   player build + level definition (sources/sinks/schedule)
│   ├── routing          #   reachability + switch routing
│   ├── blocks · sim     #   block sections, the tick loop, outcomes
│   ├── train · units    #   trains; all frozen length/speed/timing constants
│   ├── score · failure  #   the three axes; collision/deadlock/misrouting reports
│   └── hash             #   hand-rolled FNV-1a-64 replay hash
└── stellwerk_codes/     # SW1- share codes (base64 over versioned postcard)

src/                     # the Bevy app (composition root in main.rs)
├── state.rs             # GameState machine (MainMenu→Loading→LevelSelect→Edit→Run→Result)
├── levels.rs            # level catalog, local progress + solution slots, sandbox
├── loading.rs           # asset/catalog load gate
├── i18n.rs              # RON string tables with fallback chain
├── camera · font        # camera, font loading (vendored bevy_text, see below)
├── board/               # grid geometry, palette, edit/run board rendering
├── editor/              # tools, drag drawing, invertible edit ops, validation
├── run.rs               # fixed-tick accumulator driving the sim, head interpolation
├── ui/                  # one plugin per screen (menu, select, HUDs, panels, result)
└── dev_tools.rs         # feature `dev` only
```

### Determinism contract

The sim crate guarantees *same build ⇒ exactly the same run, every time*:

- No `f32`/`f64` and no hash-map iteration in simulation state — `BTreeMap`/`Vec`
  with fixed sort order; all loops iterate in ascending id order.
- No randomness in the sim (frontend juice may use `rand`; the crate never does).
- All length/speed/timing constants live in `units` and are frozen — changing
  one invalidates every replay hash and best score.
- `overflow-checks = true` on every profile: arithmetic bugs panic loudly and
  identically on all platforms instead of wrapping.

## Dev tools (feature `dev`, on by default)

| Key | Tool |
|-----|------|
| `F3` | FPS overlay |
| `F12` | World inspector (entities, components, resources, assets) |

**Hot-reload:** save `assets/config/game.tunables.ron` and train speed & co.
apply immediately in the running game.

`STELLWERK_AUTOCYCLE=1` runs a soak test that cycles the whole catalog
(LevelSelect → Edit → Run → Result → back) forever — the regression harness for
the font-atlas corruption fix below.

### Vendored `bevy_text`

`vendor/bevy_text/` is Bevy 0.18.1's text crate with one patched constant (font
atlas page size 512 → 2048). It works around corrupted glyph rendering when text
overflows onto a second atlas page in Bevy 0.18; remove the patch once upstream
fixes multi-page font atlas rendering.
