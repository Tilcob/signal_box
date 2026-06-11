# M0 Woche 2 — Tick-Loop, Bewegung, Blocksignale, Kollision

**Teilabgabe W2 der [M0-Angabe](M0-angabe.md) · 10 Punkte**
Spezifikation: [Plan §3.5, §4.1, §4.4](M0-sim-kern.md) · GDD §7.2/§7.4/§7.6

## Ziel

Diese Woche fahren zum ersten Mal Züge. Du baust den Tick-Loop mit fester
Phasenfolge, die Bewegung als Intervall auf Kantenpfaden, Blocksignal-Logik
und die Kollisionserkennung. Am Ende sind die Szenarien 1–6 und 17 grün —
die beiden `#[ignore]` in `tests/scenarios.rs` fliegen raus, und fünf neue
Szenario-Fixtures kommen dazu.

Alles, was du brauchst, liegt bereit: Der `TrackGraph` aus W1 kennt für jede
gerichtete Kante ihre Fortsetzung (`Next`), Signale gaten konkrete Kanten
(`EdgeData::signal`), und `BlockSet::block_of` liefert den Block jeder Kante.
Bewegung ist dadurch reine Buchhaltung — keinerlei Geometrie.

## Worauf es ankommt (Rust-Stolpersteine dieser Woche)

- **Borrow-Splitting im Tick-Loop.** Du willst über Züge iterieren *und*
  dabei Graph + Blockbelegung lesen. Der Borrow-Checker erlaubt kein
  `for train in &mut self.trains { self.irgendwas() }`. Bewährte Muster:
  Indizes statt Iteratoren (`for i in 0..self.trains.len()`), benötigte
  Daten vor der Mutation in lokale Variablen ziehen, oder Hilfsfunktionen
  als *freie* Funktionen schreiben, die nur die Teile nehmen, die sie
  brauchen (`fn move_train(train: &mut Train, graph: &TrackGraph, …)`).
- **Budget-Schleife mit Integern.** Bewegung pro Tick = „verbrauche
  `speed` LE, bis aufgebraucht oder blockiert". Kein Restbruch, kein f32 —
  ein Zug, der am Signal steht, steht exakt bei `head_dist == edge.len`.
- **`BTreeMap`, niemals `HashMap`** (Determinismus-Vertrag in `lib.rs`).
  Die Blockbelegung baust du pro Tick neu auf — bei Puzzle-Größen billig
  und garantiert konsistent.

## Projektstruktur nach dieser Woche

```
crates/stellwerk_sim/
├── src/
│   ├── lib.rs        # erweitert: pub mod sim; pub mod train; pub use sim::{Sim, Outcome};
│   ├── train.rs      # NEU: Train (Pfad-Intervall), Belegungs-Berechnung
│   ├── sim.rs        # NEU: Sim, Tick-Loop, Outcome, SimEvent
│   └── …             # W1-Module unverändert
└── tests/
    ├── common/mod.rs # erweitert: Expect::Collision
    ├── scenarios.rs  # erweitert: s03–s06, s17; `_runs`-Tests scharf
    └── scenarios/    # NEU: s03…s06, s17 als RON
```

Keine neuen Dependencies.

## Konzepte im Mittelpunkt

- Zustandsmaschine mit fester Phasenfolge als API-Vertrag
- Intervall-Arithmetik auf einem Graphen (Zug = belegter Bereich)
- `VecDeque` als Pfad-Fenster (vorne wächst, hinten schrumpft)
- Events nach außen geben, ohne Ownership zu verlieren (`&[SimEvent]`)

---

## Aufgabe 2.1 — Tick-Loop & Bewegung (4 Punkte)

**Was du baust:** `Train` in `src/train.rs`, `Sim` mit `new`/`step`/`run` in
`src/sim.rs`. Szenarien 1–2 grün (Ignore-Marker entfernen).

```rust
// src/train.rs
use crate::graph::TrackGraph;
use crate::units::{EdgeId, Len, SinkId, Speed, Tick, TrainClass, TrainId};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Train {
    pub id: TrainId,
    pub class: TrainClass,
    pub length: Len,
    pub speed: Speed,
    pub sink: SinkId,
    pub due: Tick,
    /// Kanten, die der Zug berührt: hinten = Schwanz, vorne (last) = Kopf.
    /// Verlassene Kanten hinten entfernen — sonst wächst der Pfad unbegrenzt
    /// (und ab W4 der Replay-Hash gleich mit).
    pub path: VecDeque<EdgeId>,
    /// Position des Kopfes auf `path.back()`, gemessen vom Kantenanfang.
    pub head_dist: Len,
}

impl Train {
    /// Belegte Intervalle, vom Kopf rückwärts: (Kante, von, bis), gemessen
    /// vom jeweiligen Kantenanfang. Der Schwanz endet spätestens am Anfang
    /// von `path.front()` — ein frisch gespawnter Zug ist also automatisch
    /// erst teilweise „in der Welt" (Plan §3.5: er wächst herein).
    pub fn occupied(&self, graph: &TrackGraph) -> Vec<(EdgeId, Len, Len)> {
        let mut rest = self.length.0;
        let mut out = Vec::new();
        // Kopfkante: [max(0, head_dist - rest), head_dist], dann rest
        // verringern und rückwärts durch `path` weiterlaufen.
        todo!("Aufgabe 2.1")
    }
}
```

```rust
// src/sim.rs
use crate::graph::{self, TrackGraph};
use crate::layout::{Layout, ValidationError};
use crate::level::{Level, ScheduleEntry};
use crate::train::Train;
use crate::units::{BlockId, EdgeId, SinkId, Tick, TrainId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Alle Fahrplan-Züge korrekt angekommen.
    Success { last_arrival: Tick },
    /// Zwei Züge berühren sich (Kante in kanonischer Richtung).
    Collision { trains: (TrainId, TrainId), edge: EdgeId },
    // W3: Misrouting { … }   W4: Deadlock { … }, Stalled { … }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimEvent {
    TrainSpawned(TrainId),
    TrainArrived { train: TrainId, at: Tick },
    RunEnded(Outcome),
}

pub struct Sim {
    graph: TrackGraph,
    /// Fahrplan, sortiert nach (depart, train) — Reihenfolge ist Vertrag.
    schedule: Vec<ScheduleEntry>,
    next_departure: usize,
    trains: Vec<Train>, // stets aufsteigend nach id sortiert
    now: Tick,
    arrivals: Vec<(TrainId, Tick)>,
    outcome: Option<Outcome>,
    events: Vec<SimEvent>,
    /// Ankunftskante → Sink, einmalig aus `graph.sinks` gebaut.
    sink_by_arrival: BTreeMap<EdgeId, SinkId>,
}

impl Sim {
    pub fn new(level: &Level, layout: &Layout) -> Result<Sim, Vec<ValidationError>> {
        let graph = graph::build(level, layout)?;
        // TODO: schedule kopieren + sortieren (depart, dann train),
        //       sink_by_arrival aus graph.sinks füllen, Rest initialisieren.
        todo!("Aufgabe 2.1")
    }

    pub fn now(&self) -> Tick { self.now }
    pub fn outcome(&self) -> Option<&Outcome> { self.outcome.as_ref() }
    pub fn trains(&self) -> &[Train] { &self.trains }

    /// Ein Tick. Die Phasenfolge ist Teil des API-Vertrags (Plan §4.1) —
    /// sie zu ändern ändert Spielverhalten und (ab W4) jeden Replay-Hash.
    pub fn step(&mut self) -> &[SimEvent] {
        self.events.clear();
        if self.outcome.is_some() {
            return &self.events;
        }
        // Phase 1: Spawn — fällige Fahrplan-Einträge einsetzen.
        // Phase 2: Signalauswertung — in W2 nur Blocksignale; Kettensignale
        //          verhalten sich bis W3 wie Blocksignale (dokumentierte
        //          Vereinfachung, Szenarien 1–6/17 enthalten keine).
        // Phase 3: Bewegung — Züge in aufsteigender TrainId-Reihenfolge.
        // Phase 4: Ankunft/Abräumen.
        // Phase 5: Checks — W2: Kollision (Aufgabe 2.3) + Erfolg.
        self.now = Tick(self.now.0 + 1);
        todo!("Aufgabe 2.1")
    }

    /// Headless bis Ende oder `max` Ticks (Treiber für die Szenario-Tests).
    pub fn run(&mut self, max: Tick) -> Outcome {
        while self.outcome.is_none() && self.now < max {
            self.step();
        }
        // TODO: outcome zurückgeben; ohne Ende bis max → das ist ab W4 der
        //       Stillstand-Fall, bis dahin: panic mit klarer Meldung.
        todo!("Aufgabe 2.1")
    }
}
```

**Bewegung im Detail** (freie Funktion empfohlen, der Borrow-Splitting wegen):

```rust
/// Bewegt einen Zug um sein Tick-Budget. `may_enter` sagt, ob der Zug die
/// nächste Kante betreten darf (Signal-/Blocklogik — kommt aus Phase 2).
fn move_train(
    train: &mut Train,
    graph: &TrackGraph,
    may_enter: impl Fn(&Train, EdgeId) -> bool,
) -> MoveResult {
    let mut budget = train.speed.0;
    while budget > 0 {
        let edge = graph.edge(*train.path.back().expect("train has a path"));
        let to_end = edge.len.0 - train.head_dist.0;
        if to_end > 0 {
            // TODO: step = min(budget, to_end); head_dist += step; budget -= step;
            todo!("Aufgabe 2.1");
        }
        // Kopf steht exakt am Kantenende: Übergang versuchen.
        // TODO: über edge.next die Folgekante bestimmen:
        //   Next::Fixed(e2)     → may_enter? → path.push_back(e2), head_dist = 0
        //                          sonst → return MoveResult::Blocked
        //   Next::SwitchChoice  → W3! Bis dahin: unreachable!() mit Hinweis —
        //                          W2-Szenarien enthalten keine Weichen.
        //   Next::DeadEnd       → Ankunftskante? (sink_by_arrival) → Arrived;
        //                          sonst W3-Fehlleitung, bis dahin panic.
        todo!("Aufgabe 2.1");
    }
    // TODO: hinten verlassene Kanten abwerfen (occupied() hilft).
    MoveResult::Moved
}

enum MoveResult { Moved, Blocked, Arrived }
```

> **Ankunft = Kopf erreicht das Ende der Ankunftskante** (`graph.sinks[i].arrival`).
> Für W2 genügt: Zug despawnt sofort, `TrainArrived` mit `self.now` in die
> Events, Tick in `arrivals` festhalten. Das „Hinausschrumpfen" aus Plan §3.5
> ist die optionale Erweiterung unten — despawne erst sofort, miss die
> Szenarien, und entscheide dann, ob du sie brauchst.
>
> **Erfolgs-Check (Phase 5):** alle Fahrplan-Einträge gespawnt, keine Züge
> mehr unterwegs → `Outcome::Success { last_arrival }` + `RunEnded`-Event.
> Ob ein Zug am *richtigen* Sink ankam, prüfst du ab W3 — W2-Szenarien
> haben nur einen Sink; ein `debug_assert_eq!(sink, train.sink)` reicht.

**Spawn (Phase 1):** Solange `schedule[next_departure].depart <= now`: Zug
mit `path = [entry]` und `head_dist = 0` einsetzen (`entry` aus
`graph.sources`, das Mapping `SourceId → EdgeId` baust du dir in `new`).
Eine belegte Quelle kann in den W2-Szenarien nicht vorkommen — die
FIFO-Warteschlange kommt in W4 (Aufgabe 4.2).

**Treiber scharfschalten:** In `tests/scenarios.rs` die beiden `#[ignore]`
entfernen und die `_runs`-Tests implementieren:

```rust
fn run_scenario(name: &str) -> (stellwerk_sim::sim::Outcome, common::Expect) {
    let s = load(name);
    let mut sim = stellwerk_sim::Sim::new(&s.level, &s.layout)
        .expect("scenario validates");
    (sim.run(stellwerk_sim::units::Tick(10_000)), s.expect)
}
// Dann pro Szenario gegen `expect` matchen — mit sprechender Fehlermeldung
// („s03: erwartet Success bis Tick 80, bekam Collision bei Tick 41 …").
```

**Frage (Notizen):** Warum ist die Phasenreihenfolge Teil des API-Vertrags
und nicht bloß ein Implementierungsdetail? Konstruiere ein konkretes
Beispiel, bei dem „Bewegung vor Signalauswertung" ein anderes Ergebnis
liefert als die spezifizierte Reihenfolge.

---

## Aufgabe 2.2 — Blocksignale & Belegung (3 Punkte)

**Was du baust:** Die `may_enter`-Logik aus Phase 2/3. Szenario 3 grün.

Pro Tick baust du die Blockbelegung deterministisch neu auf:

```rust
/// Block → Züge, die ihn berühren (über Train::occupied aller Züge).
fn block_occupancy(
    trains: &[Train],
    graph: &TrackGraph,
) -> BTreeMap<BlockId, Vec<TrainId>> {
    todo!("Aufgabe 2.2")
}
```

Die Regel (GDD §7.4, in Graph-Begriffen):

- Ein Zug darf Kante `e2` betreten, wenn deren Block frei ist oder ihm
  selbst gehört. **Ohne Signal am Übergang gibt es keine Prüfung** — der
  Übergang bleibt im selben Block, oder es kracht eben (Aufgabe 2.3, das
  ist das Spiel).
- Trägt die aktuelle Kante ein Signal (`edge.signal.is_some()`), wird am
  Kantenende gehalten, solange der Block der Folgekante einem *anderen*
  Zug gehört. `BlockSet::block_of` liefert den Block.

> **Konkurrenz** (Plan §4.1 Phase 2): Begehren zwei Züge im selben Tick
> denselben Block, gewinnt der mit dem früheren Anspruch; beim selben Tick
> die niedrigere `TrainId`. Da Phase 3 die Züge in aufsteigender Id-Folge
> bewegt, bekommst du den Tie-Break fast geschenkt — aber **schreibe den
> gezielten Unit-Test** (zwei Züge, gleicher Tick, beide wollen denselben
> Block), der beweist, dass nicht zufällig beide hineinfahren.

**Szenario 3** (`s03_two_trains_block_signal.ron`) baust du als Fixture:
gerade Strecke wie s01, ein Blocksignal in der Mitte, zwei Züge mit 2 Ticks
Abstand. Erwartung: `Success`, und der zweite Zug muss am Signal gewartet
haben (prüfbar über den Ankunftsabstand: deutlich größer als der
Abfahrtsabstand).

In `tests/common/mod.rs` erweiterst du dafür:

```rust
#[derive(Debug, Deserialize)]
pub enum Expect {
    Success { last_arrival_by: Tick },
    Collision { trains: (u32, u32) },   // TrainIds
}
```

---

## Aufgabe 2.3 — Kollisionserkennung (3 Punkte)

**Was du baust:** Phase-5-Check über Intervall-Überlappung. Szenarien 4–6
und 17 grün.

Zwei Züge kollidieren, wenn sich ihre belegten Intervalle auf demselben
**ungerichteten** Stub überlappen. Gegenrichtungs-Kanten musst du dafür auf
eine kanonische Richtung spiegeln:

```rust
/// Intervall [a, b] auf Kante e, ausgedrückt auf der kanonischen Kante
/// (kleinere Id von e und e.opposite). Liegt e selbst kanonisch, bleibt
/// alles; sonst wird gespiegelt: [len - b, len - a].
fn canonical_interval(
    graph: &TrackGraph,
    edge: EdgeId,
    from: Len,
    to: Len,
) -> (EdgeId, Len, Len) {
    todo!("Aufgabe 2.3")
}
```

Dann: alle Intervalle aller Züge einsammeln (nach kanonischer Kante
gruppiert, `BTreeMap<EdgeId, Vec<(TrainId, Len, Len)>>`), pro Kante jedes
Paar prüfen. Überlappung zweier Intervalle `[a1,b1]`, `[a2,b2]`:
`a1 < b2 && a2 < b1` — **strikt**, damit „Kopf an Schwanz bei exakt 0 Lücke"
(Folgezug wartet direkt am Signal) noch keine Kollision ist.

Beim ersten Treffer: `Outcome::Collision { trains: (kleinere Id zuerst),
edge: kanonische Kante }`, `RunEnded`, Sim eingefroren (`step` tut danach
nichts mehr — das steht schon im Skeleton).

**Die drei Fixtures:**

| Datei | Aufbau | Erwartung |
|---|---|---|
| `s04_rear_end_no_signal.ron` | s03 ohne das Signal | `Collision` der beiden Züge |
| `s05_head_on_single_track.ron` | eine Strecke, Quelle links *und* rechts, je ein Zug aufeinander zu | `Collision` |
| `s06_passing_loop.ron` | Gegenverkehr wie s05, aber mit Ausweiche: zwei parallele Gleise zwischen zwei Weichen, Signale davor | `Success` |
| `s17_long_train_two_blocks.ron` | ein Zug, dessen Länge zwei Blöcke überspannt; dahinter ein zweiter Zug mit Blocksignal | `Success` — der zweite hält, solange der lange *irgendeinen* Teil im Block hat |

> **s06 ist der erste echte Integrationstest** — Weichen! Falls du 2.1
> streng ohne `SwitchChoice` gebaut hast (`unreachable!()`), ziehst du die
> Minimal-Variante hier vor: an `SwitchChoice` einfach
> `branch_out[default_branch]` nehmen (`graph.switches[i]`). Die Regeln
> (`rules`) bleiben W3. Plane für s06 Debugzeit ein; ein simpler
> ASCII-Dump des Zustands (Bonus W4) zahlt sich ab hier aus.
>
> Für s05 brauchst du **zwei Quellen und zwei Sinks** (jede Richtung hat
> ihr Ziel auf der Gegenseite) — Gegenverkehr auf demselben Gleis nutzt
> die Gegenrichtungs-Kanten, genau das prüft deine Spiegelung.

---

## So prüfst du deine Lösung

- `cargo test -p stellwerk_sim` — alles grün, **nichts mehr ignoriert**:
  W1-Tests + Szenarien 1–6, 17 (`_builds` und `_runs`)
- Der Konkurrenz-Unit-Test aus 2.2 existiert und schlägt fehl, wenn du den
  Tie-Break absichtlich kaputt machst (einmal ausprobieren!)
- `cargo clippy -p stellwerk_sim --all-targets` ohne Warnungen,
  `cargo fmt -p stellwerk_sim -- --check` sauber
- Kein `f32`, kein `HashMap` — einmal `grep` über `src/` laufen lassen
- Notizen-Fragen aus 2.1 in `plans/M0-notizen.md` beantwortet

## Optionale Erweiterung

**Hinausschrumpfen statt Sofort-Despawn:** Nach der Ankunft bleibt der Zug
als „ausfahrend" erhalten; sein Kopf läuft virtuell weiter, die Belegung
schrumpft pro Tick, bis nichts mehr in der Welt ist (Feld
`leaving_since: Option<Tick>` + Anpassung in `occupied`). Vergleiche s03:
Wie ändert sich der Ankunftsabstand des zweiten Zugs? Halte in den Notizen
fest, welche Variante du behältst und warum — beides ist vertretbar, aber
ab W4 friert der Replay-Hash die Entscheidung ein.
