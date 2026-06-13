# Aufgabenblatt — Die 3 FPS-Killer beheben

> Du implementierst selbst. Code-Skelette mit `todo!()`, Reihenfolge nach
> Wirkung. **Immer im Release messen:** `cargo run --release --no-default-features`
> + F3-Overlay (das `dev`-Feature mit egui-Inspector verfälscht die FPS).

Reihenfolge bewusst: #1 ist ~90 % des Gewinns, #2/#3 sind Feinschliff.

---

## Killer 1 — `draw_run_board` baut JEDEN Frame ALLE Sprites neu

**Datei:** `src/board/run_board.rs` (+ `src/board/draw.rs`, `src/board/mod.rs`)

**Problem:** `draw_run_board` läuft im `Update` während `Run` und macht zuerst
`for e in &existing { despawn }`, dann spawnt es alles neu — Gitterzellen,
Stationen, **ein Band pro Graph-Kante** (auf großen Layouts hunderte), alle
Signal-Lampen, alle Zug-Sprites. Pro Frame. Das ist Entity-Churn +
Command-Buffer-Flut + Archetyp-Bewegungen → der eigentliche Frame-Drop.

**Einsicht:** Die **Geometrie ändert sich während eines Runs nie** — nur
- Block-Bänder: **Farbe + Breite** (belegt/reserviert/frei),
- Signal-Lampen: **Farbe** (grün/rot),
- Züge: **Position + Anzahl** (die einzigen, die wirklich pro Frame neu müssen).

### Plan: statisch einmal spawnen, dynamisch in-place ändern

**1. Neue Marker/Komponenten** (`draw.rs`):
```rust
/// Persistentes Band einer Graph-Kante; trägt seinen Block für das
/// Per-Frame-Recoloring.
#[derive(Component)]
pub(super) struct BlockBand(pub BlockId);

/// Persistente Signal-Lampe; trägt die gated Folge-Kante (oder den Block),
/// damit das Recoloring den Zustand nachschlagen kann.
#[derive(Component)]
pub(super) struct SignalLamp { pub block: BlockId }

/// Per-Frame neu erzeugte Zug-Sprites (Körper-Bänder, Kopf-Lampe, Nummer).
/// Nur DIESE werden noch despawnt/respawnt — sind wenige.
#[derive(Component)]
pub(super) struct TrainGfx;
```
`LiveGfx` bleibt als Sammel-Marker fürs Cleanup (alle drei Gruppen tragen ihn
zusätzlich, damit `despawn_all::<LiveGfx>` beim Verlassen weiter alles trifft).

**2. `OnEnter(Run)` — statisches Board einmal bauen** (neues System
`spawn_run_board_static`):
```rust
fn spawn_run_board_static(mut commands: Commands, ui_font: Res<UiFont>,
                          active: Option<Res<ActiveLevel>>, ctl: Option<Res<RunCtl>>) {
    let (Some(active), Some(ctl)) = (active, ctl) else { return };
    // todo!(): Gitterzellen + draw_stations EINMAL (Tag::Live).
    // todo!(): pro kanonischer Kante ein Band spawnen, dabei
    //          BlockBand(block) + LiveGfx anhängen. Farbe/Breite hier neutral.
    // todo!(): pro Signal eine Lampe spawnen mit SignalLamp{block} + LiveGfx.
}
```

**3. `Update(Run)` — nur Zustand mutieren** (statt respawnen):
```rust
fn update_block_bands(
    ctl: Option<Res<RunCtl>>,
    mut bands: Query<(&BlockBand, &mut Sprite)>,
) {
    let Some(ctl) = ctl else { return };
    // todo!(): occupied/reserved-Sets EINMAL bilden (wie bisher), dann
    //          für jedes Band Sprite.color + Sprite.custom_size.y (Breite)
    //          nach Block-Zustand setzen. KEIN despawn/spawn.
}

fn update_signal_lamps(
    ctl: Option<Res<RunCtl>>,
    /* gecachte edge_at-Map aus Killer 2 */
    mut lamps: Query<(&SignalLamp, &mut Sprite)>,
) {
    // todo!(): pro Lampe Farbe grün/rot setzen (signal_display_state-Logik,
    //          aber auf den vorab gespeicherten Block angewandt).
}

fn redraw_trains(
    mut commands: Commands, ui_font: Res<UiFont>,
    existing: Query<Entity, With<TrainGfx>>,
    ctl: Option<Res<RunCtl>>,
) {
    // todo!(): NUR TrainGfx despawnen + neu spawnen (Körper-Bänder,
    //          interpolierte Kopf-Lampe, Nummer). Das ist billig (wenige Züge).
}
```

**4. `board/mod.rs` umverdrahten:**
```rust
.add_systems(OnEnter(GameState::Run), spawn_run_board_static)
.add_systems(Update, (update_block_bands, update_signal_lamps, redraw_trains)
    .run_if(in_state(GameState::Run)))
// Result: Endbild einfrieren — statisches Board + EIN letzter Train-Redraw.
.add_systems(OnEnter(GameState::Result), (spawn_run_board_static, redraw_trains).chain())
.add_systems(OnExit(GameState::Result), despawn_all::<LiveGfx>)
.add_systems(OnEnter(GameState::Edit), despawn_all::<LiveGfx>)
.add_systems(OnEnter(GameState::LevelSelect), despawn_all::<LiveGfx>)
```

⚠️ **Breite-Änderung:** `band()` setzt `Sprite::from_color(color, Vec2(len, width))`.
Breite ändern = `sprite.custom_size = Some(Vec2(len, neue_breite))`. Länge bleibt
(Geometrie statisch). Das ist eine reine Feld-Mutation, kein Respawn.

**Erwartung:** Entity-Anzahl pro Frame konstant statt hunderte Spawns/Despawns.
Auf großen Sandbox-Layouts der entscheidende Sprung Richtung stabile 60 FPS.

---

## Killer 2 — `signal_display_state` baut die Kanten-Map pro Frame neu

**Datei:** `src/board/run_board.rs`

**Problem:** In `draw_run_board` wird jeden Frame
`edge_at: BTreeMap<(Point,Point), EdgeId>` über **alle** Kanten neu aufgebaut,
nur um Signale ihrer gated Kante zuzuordnen. Der Graph ist während des Runs fix.

**Fix:** Einmal pro Run als Resource cachen.
```rust
#[derive(Resource)]
struct SignalEdgeMap(BTreeMap<(Point, Point), EdgeId>);

// OnEnter(Run): aus sim.graph() bauen, insert_resource.
// (Mit Killer 1 brauchst du die Map ohnehin nur noch in update_signal_lamps.)
fn build_signal_edge_map(/* ctl */ mut commands: Commands) {
    todo!("Map einmal bauen + insert_resource(SignalEdgeMap(..))");
}
// OnExit(Run)/OnEnter(Edit): remove_resource::<SignalEdgeMap>.
```
Noch sauberer: gleich beim `OnEnter(Run)`-Aufbau die gated `EdgeId` **direkt in
`SignalLamp`** speichern (statt der Map) — dann fällt die Map ganz weg.

---

## Killer 3 — `Train::occupied()` alloziert einen `Vec` pro Aufruf

**Datei:** `crates/stellwerk_sim/src/train.rs` (+ Aufrufer in `sim.rs`, `board`)

**Problem:** `occupied(&self, graph) -> Vec<(EdgeId, Len, Len)>` alloziert jedes
Mal einen `Vec`. Im Sim pro Tick mehrfach pro Zug, im Rendering pro Frame pro
Zug — auf der heißen Schleife.

**Fix (rein perf, KEINE Verhaltensänderung):** Buffer-Variante anbieten, die in
einen übergebenen `&mut Vec` schreibt; die alte Methode bleibt als dünner
Wrapper für Tests/Bequemlichkeit.
```rust
impl Train {
    /// Wie `occupied`, aber schreibt in `out` (vorher geleert) statt zu
    /// allozieren. Heiße Pfade nutzen diese Variante mit wiederverwendetem Buffer.
    pub fn occupied_into(&self, graph: &TrackGraph, out: &mut Vec<(EdgeId, Len, Len)>) {
        out.clear();
        // todo!(): bisherige occupied-Logik, aber out.push(...) statt Vec::push.
    }

    pub fn occupied(&self, graph: &TrackGraph) -> Vec<(EdgeId, Len, Len)> {
        let mut out = Vec::new();
        self.occupied_into(graph, &mut out);
        out
    }
}
```
Aufrufer in `sim.rs` (z. B. `phase_checks`, Occupancy-Aufbau) und in
`run_board.rs` halten **einen** `Vec` außerhalb der Zug-Schleife und rufen
`occupied_into` pro Zug.

⚠️ **Determinismus:** Reihenfolge/Inhalt von `occupied_into` muss **bit-identisch**
zu `occupied` sein — sonst kippen die Golden-Replay-Hashes. Nach dem Umbau
`cargo test -p stellwerk_sim` (besonders `golden_replay_hashes`) grün halten.
Das ist eine reine Allokations-Optimierung, **kein** Logik-Eingriff.

---

## Verifikation (alle drei)

- [ ] `cargo clippy --workspace --all-targets` ohne Warnung, alle 63 Tests grün
      (v. a. `golden_replay_hashes`, `s20_determinism`).
- [ ] **Release-FPS:** `cargo run --release --no-default-features`, ins größte
      Sandbox-Layout, viele Züge, ×16 Tempo, F3. Vorher/Nachher vergleichen.
- [ ] `STELLWERK_AUTOCYCLE`-Soak läuft weiter sauber (Render-Pfad geändert →
      kurz gegenchecken, dass Board/Result korrekt aussehen).
- [ ] Optik unverändert: Bänder, Lampen, Züge, Farben/Breiten wie vorher.
