# M0 Woche 3 — Weichen-Routing, Fehlleitung, Kettensignale

**Teilabgabe W3 der [M0-Angabe](M0-angabe.md) · 10 Punkte**
Spezifikation: [Plan §4.3, §4.4](M0-sim-kern.md) · GDD §7.3/§7.4/§7.6

## Ziel

„Die Weiche ist das Programm" wird diese Woche wahr: Züge folgen den
Weichenregeln des Spielers, Fehlleitungen werden als sauberer Fehlschlag
gemeldet (inklusive Schuldzuweisung an die richtige Weiche), und das
Kettensignal — das Deadlock-Werkzeug des ganzen Spiels — reserviert
Fahrstraßen durch Kreuzungsbereiche. Am Ende sind die Szenarien 7–12, 14
und 15 grün.

**Achtung, Bug-Nest:** Die Fahrstraßen-Reservierung (3.3) ist laut Plan §7
die fehleranfälligste Stelle von ganz M0. Deshalb gilt dort verpflichtend
Test-first: Fixtures schreiben, rot committen, dann implementieren.

## Worauf es ankommt

- **Pure Funktionen zuerst.** `resolve` (Weichenentscheid) hängt nur von
  Weichendaten + Zugeigenschaften ab — keine `&mut self`-Methode, sondern
  eine freie Funktion. Die testest du in Minuten erschöpfend; im Tick-Loop
  wird sie dann nur noch eingestöpselt.
- **Ein Pfad, kein Suchbaum.** Weil Routing deterministisch ist, ist die
  „Route" eines Zugs ein simpler Spaziergang über `Next` + `resolve` —
  Erreichbarkeit prüfen heißt: laufen, bis Sink, Sackgasse oder
  Schleifen-Limit. Kein Dijkstra, kein Backtracking.
- **Reservierungen sind Zustand.** Anders als die pro Tick neu berechnete
  Blockbelegung (W2) leben Kettensignal-Reservierungen über Ticks hinweg —
  sie gehören in die `Sim`-Struct und ab W4 in den Replay-Hash. Halte sie
  in einer `BTreeMap<BlockId, TrainId>`.

## Projektstruktur nach dieser Woche

```
crates/stellwerk_sim/
├── src/
│   ├── lib.rs        # erweitert: pub mod routing; pub use routing::check_reachability;
│   ├── routing.rs    # NEU: resolve, Routen-Spaziergang, check_reachability
│   ├── sim.rs        # erweitert: Outcome::Misrouting, Reservierungen, Kettenlogik
│   └── train.rs      # ggf. erweitert (gefahrene Weichen fürs Blame-Tracking)
└── tests/
    ├── common/mod.rs # erweitert: Expect::Misrouting
    └── scenarios/    # NEU: s07…s12, s14, s15
```

## Konzepte im Mittelpunkt

- Regelauswertung in definierter Reihenfolge (first match wins)
- Pfad-Spaziergang mit Schleifen-Schutz (besuchte Kanten zählen)
- Reservierungs-Lebenszyklus: anfordern → halten → blockweise freigeben
- Fehlerdiagnose als Feature: *welche* Weiche ist schuld?

---

## Aufgabe 3.1 — Weichen-Routing (3 Punkte)

**Was du baust:** `src/routing.rs` mit `resolve` als pure Funktion; die
`SwitchChoice`-Stelle aus W2 ruft sie auf. Szenarien 7–9 grün.

```rust
// src/routing.rs
use crate::graph::SwitchData;
use crate::layout::RuleWhen;
use crate::units::{EdgeId, SinkId, TrainClass};

/// Weichenentscheid für einen Zug: erste passende Regel gewinnt
/// (Listenreihenfolge = Spieler-Priorität), sonst die Grundstellung.
/// Liefert die Ausfahrkante Richtung Zweig (GDD §7.3).
pub fn resolve(switch: &SwitchData, class: TrainClass, sink: SinkId) -> EdgeId {
    for rule in &switch.rules {
        let matches = match rule.when {
            RuleWhen::DestIs(s) => todo!("Aufgabe 3.1"),
            RuleWhen::ClassIs(c) => todo!("Aufgabe 3.1"),
        };
        // TODO: bei Treffer branch_out[rule.branch as usize] zurückgeben
    }
    todo!("Aufgabe 3.1: Grundstellung (default_branch)")
}
```

Unit-Tests **ohne Sim** (direkt im Modul, `SwitchData` von Hand bauen):
Regel nach Ziel, Regel nach Zugtyp, zwei konkurrierende Regeln in beiden
Reihenfolgen (beweist first-match-wins), leere Regelliste → Grundstellung.

**Die drei Fixtures** — alle auf demselben Grundlayout (eine Quelle, eine
Weiche, zwei Sinks „OST"/„NORD" wie im `switch_routing_hooks`-Test aus W1):

| Datei | Weichen-Konfiguration | Erwartung |
|---|---|---|
| `s07_switch_default.ron` | keine Regeln, `default_branch: 1` | Zug landet bei NORD |
| `s08_switch_dest_rule.ron` | Grundstellung OST, Regel `DestIs(NORD) → 1`; Zugziel NORD | Regel schlägt Grundstellung |
| `s09_switch_rule_order.ron` | zwei Züge (Klasse 0/1), Regeln `ClassIs(1) → 1` **vor** `DestIs(…) → 0` | erster Treffer zählt — Zug der Klasse 1 fährt Zweig 1, der andere fällt durch zur zweiten Regel |

> In s09 brauchst du Züge, deren Ziel-Regel und Klassen-Regel
> *widersprechen* — sonst testest du die Reihenfolge nicht. Beide Züge
> müssen trotzdem **korrekt** ankommen (Erwartung `Success`), sonst ist es
> ein Fehlleitung-Szenario; richte die Sinks entsprechend ein.

---

## Aufgabe 3.2 — Fehlleitung & Erreichbarkeit (3 Punkte)

**Was du baust:** `Outcome::Misrouting` mit Schuld-Weiche und
`check_reachability` als Editor-Vorabprüfung. Szenarien 10–12 grün.

```rust
// src/sim.rs — Outcome erweitern:
pub enum Outcome {
    // … W2-Varianten …
    /// Zug erreichte einen falschen Sink oder eine Sackgasse (GDD §7.6).
    Misrouting {
        train: TrainId,
        /// Was tatsächlich erreicht wurde (None = Sackgasse).
        reached: Option<SinkId>,
        /// Die letzte Weiche, deren *anderer* Zweig zum Soll-Sink geführt
        /// hätte — None, wenn keine Weiche das Ziel je erreichbar machte.
        blame: Option<Cell>,
    },
}
```

Auslöser in der Bewegung (ersetzt die W2-Panics): Kopf erreicht eine
Ankunftskante mit falschem Sink, oder `Next::DeadEnd` auf einer
Nicht-Ankunftskante.

**Blame-Rekonstruktion** (Plan §4.4): Du kennst den gefahrenen Pfad. Laufe
ihn rückwärts; an jeder passierten Weiche prüfst du: erreicht man vom
*nicht genommenen* Zweig aus den Soll-Sink, wenn ab dort wieder normal
geroutet wird (`walk_route` unten)? Die erste Weiche (von hinten), bei der
das klappt, ist schuld. Dafür musst du dir beim Fahren merken, welche
Weichen der Zug passiert hat — ein `Vec<(Cell, EdgeId /* genommen */)>` am
`Train` reicht.

```rust
// src/routing.rs
use crate::graph::TrackGraph;
use crate::level::Level;

pub enum RouteEnd {
    Sink(SinkId),
    DeadEnd,
    /// Schleifen-Schutz griff (mehr Kanten besucht als existieren).
    Loops,
}

/// Folgt Next + resolve von einer Startkante bis zum Ende — der
/// „Spaziergang", den ein Zug mit diesen Eigenschaften fahren würde.
pub fn walk_route(
    graph: &TrackGraph,
    start: EdgeId,
    class: TrainClass,
    sink: SinkId,
) -> RouteEnd {
    todo!("Aufgabe 3.2")
}

/// Editor-Vorabprüfung (Plan §4.1): welche Fahrplan-Züge erreichen ihr
/// Ziel mit der aktuellen Weichenkonfiguration nicht?
pub struct Unreachable {
    pub train: TrainId,
    pub end: RouteEnd,
}

pub fn check_reachability(level: &Level, layout: &Layout) -> Vec<Unreachable> {
    // TODO: Graph bauen (validiert), pro Fahrplan-Eintrag walk_route ab der
    //       Quellen-Einfahrkante; alles ≠ richtiger Sink sammeln.
    todo!("Aufgabe 3.2")
}
```

**Die drei Fixtures:**

| Datei | Aufbau | Erwartung |
|---|---|---|
| `s10_misrouting_wrong_sink.ron` | s07-Layout, aber Zugziel OST bei Grundstellung NORD, keine Regeln | `Misrouting`, `reached: Some(NORD)`, `blame: Some(Weichenzelle)` |
| `s11_misrouting_dead_end.ron` | Abzweig endet im Nichts (offenes Gleisende — seit W1 legal!) | `Misrouting`, `reached: None` |
| `s12_reachability_check.ron` | wie s10 | kein Sim-Lauf: Test ruft `check_reachability` und erwartet genau den einen Zug |

In `tests/common/mod.rs`: `Expect::Misrouting { train: u32 }` ergänzen
(plus optional `reached`/`blame`, wenn du sie im RON prüfen willst — je
genauer die Fixtures, desto mehr fangen sie später).

**Frage (Notizen):** Warum darf `check_reachability` *nicht* einfach
„existiert irgendein Pfad zum Sink?" prüfen (klassisches BFS über alle
Zweige)? Was wäre das Spieler-erlebbare Symptom, wenn Editor-Warnung und
Sim-Verhalten hier auseinanderliefen?

---

## Aufgabe 3.3 — Kettensignale & Fahrstraßen (4 Punkte)

**Test-first, verpflichtend:** Schreibe die Fixtures `s14` und `s15` und
committe sie **rot** (Commit-Historie zeigt das), bevor du eine Zeile
Reservierungslogik implementierst.

**Was du baust:** Fahrstraßen-Reservierung in der `Sim`; das Kettensignal
ersetzt die W2-Vereinfachung („Kette = Block").

Semantik (GDD §7.4, Referenz: Factorio-Wiki „Railway signaling" — notiere
bewusste Abweichungen in den Notizen):

1. Ein Zug steht vor einem **Kettensignal**. Laufe seine effektive Route
   (`Next` + `resolve`) ab dem Signal vorwärts und sammle die Blöcke:
   - an jedem weiteren **Kettensignal**: weitersammeln;
   - am ersten **Blocksignal**: dessen Folgeblock kommt noch dazu, dann
     stoppt die Sammlung;
   - endet die Route vorher (Sink): Sammlung stoppt dort.
2. Grün ⇔ **alle** gesammelten Blöcke sind frei oder gehören dem Zug
   selbst (weder belegt noch fremd-reserviert).
3. Bei Grün werden alle gesammelten Blöcke für den Zug **reserviert**
   (`BTreeMap<BlockId, TrainId>` in der `Sim`).
4. Reserviert zählt für *andere* Züge wie belegt — auch fürs simple
   Blocksignal aus W2 (Belegungs-Check um Reservierungen erweitern!).
5. Freigabe blockweise: verlässt das Zugende einen Block (er taucht nicht
   mehr in `occupied` auf), verfällt seine Reservierung.

```rust
// src/sim.rs — Skizze der Signalauswertung (Phase 2):
fn signal_clearance(
    train: &Train,
    gated_edge: EdgeId,
    graph: &TrackGraph,
    occupancy: &BTreeMap<BlockId, Vec<TrainId>>,
    reservations: &BTreeMap<BlockId, TrainId>,
) -> Clearance {
    match graph.signals[…].kind {
        SignalKind::Block => todo!("W2-Logik + Reservierungen beachten"),
        SignalKind::Chain => {
            // TODO: Blöcke entlang der Route sammeln (Schritt 1),
            //       prüfen (Schritt 2), Ergebnis melden — die Reservierung
            //       selbst trägt der Aufrufer ein (Schritt 3), damit diese
            //       Funktion pur und testbar bleibt.
            todo!("Aufgabe 3.3")
        }
    }
}

enum Clearance {
    Go,
    /// Bei Chain: die zu reservierenden Blöcke.
    GoAndReserve(Vec<BlockId>),
    Stop,
}
```

> **Konkurrenz unverändert:** fordern zwei Züge im selben Tick
> überschneidende Fahrstraßen an, gewinnt die niedrigere `TrainId` (Phase-2-
> Reihenfolge). Der zweite sieht die frische Reservierung und wartet.
>
> **Schleifen-Schutz** auch hier: die Sammel-Route läuft maximal über so
> viele Kanten, wie der Graph hat — sonst hängt ein kreisförmiges Layout
> ohne Blocksignal die Sim auf.

**Die zwei Fixtures (test-first):**

| Datei | Aufbau | Erwartung |
|---|---|---|
| `s14_chain_signal_crossing.ron` | Kreuzungszelle (zwei Stücke N–S + W–E in einer Zelle teilen den Block); Querverkehr aus zwei Quellen; **Kettensignal** vor der Kreuzung, **Blocksignal** dahinter | `Success` — kein Zug bleibt in der Kreuzung stehen |
| `s15_chain_reservation_timing.ron` | wie s14, aber Abfahrtszeiten so, dass Zug B sein Kettensignal exakt in dem Tick erreicht, in dem Zug A die Fahrstraße schon hält | `Success`, und B's Ankunft beweist, dass er gewartet hat (Mindest-Ankunftstick in `expect` festhalten) |

> Szenario 13 (dieselbe Kreuzung **nur mit Blocksignalen** → Deadlock)
> bleibt liegen: Es braucht die Deadlock-*Erkennung* aus W4. Wenn du s14
> vorher versehentlich mit Block- statt Kettensignalen fährst, bekommst du
> einen Vorgeschmack — die Sim steht dann einfach für immer. Genau deshalb
> gibt es W4.

---

## So prüfst du deine Lösung

- `cargo test -p stellwerk_sim`: W1+W2-Tests plus Szenarien 7–12, 14, 15
- Commit-Historie zeigt: s14/s15-Fixtures **vor** der Kettenlogik, rot
- `resolve`-Unit-Tests decken first-match-wins in beiden Reihenfolgen ab
- Clippy/fmt sauber; weiterhin kein `f32`/`HashMap` in `src/`
- Notizen: Frage aus 3.2 + die Kettensignal-Frage aus der
  [M0-Angabe](M0-angabe.md) (warum darf die Reservierung die
  Weichenentscheidung nicht raten?) + bewusste Factorio-Abweichungen

## Optionale Erweiterung

`walk_route` gibt es jetzt — damit ist ein `examples/route_dump.rs`
billig: Szenario laden, pro Fahrplan-Zug die Route als Kantenliste mit
Blockgrenzen und Signalen ausgeben. Zehn Zeilen, und du debuggst s15 nicht
mehr blind. (Der volle ASCII-Replay-Viewer bleibt Bonus in der
[M0-Angabe](M0-angabe.md).)
