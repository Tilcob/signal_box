# M0 — Implementierungsplan Sim-Kern (`stellwerk_sim`)

> Abgeleitet aus [GDD.md](../../GDD.md) §6, §7, §12, §13. Das GDD bleibt Single
> Source of Truth: Stößt die Implementierung auf einen Design-Konflikt, wird
> **zuerst** das GDD geändert (inkl. Historie), dann dieser Plan, dann Code.
>
> **Ziel (GDD §13):** Deterministischer Tick-Kern — Gleisgraph, Blöcke, beide
> Signaltypen, Weichen-Routing, Deadlock-/Kollisionserkennung, headless.
> **Exit-Kriterium:** 20 Sim-Szenarien als Tests grün; Replays bit-identisch
> (CI auf Windows + Linux).
> **Zeitrahmen:** 4 Wochen.

## 1. Scope

**In M0:**
- Crate `stellwerk_sim` (kein Bevy, kein f32 im Zustand, kein Zufall, §12.1)
- Datenmodell: Level-Definition, Anlage (Layout), Fahrplan
- Tick-Simulation: Bewegung, Blöcke, Block-/Kettensignale, Weichen-Routing,
  Einfahrts-FIFO, Ankunft
- Fehlschlag-Erkennung: Kollision, Deadlock, Fehlleitung (GDD §7.6)
- Bewertung: Durchsatz, Material, Pünktlichkeit (GDD §7.7)
- Erreichbarkeits-Check (Editor-Warnung, GDD §7.3)
- Replay-Hash + Determinismus-Tests, CI-Workflow

**Nicht in M0** (kommt in M1+): Rendering/Interpolation, Editor-UX, Undo-Stack
(nur das Operationen-Datenmodell wird vorbereitet), `stellwerk_codes`
(Sharing), Par-Werte-Tuning, Soundevents-Feinschliff. Der Live-Dispatcher-
Prototyp in `src/` bleibt unangetastet liegen, bis M1 ihn ersetzt.

## 2. Workspace-Umbau (Tag 1)

```
signal_box/
├── Cargo.toml            # wird [workspace] + Root-Bin (bestehender Prototyp)
├── crates/
│   └── stellwerk_sim/    # neu, M0-Gegenstand
│       ├── Cargo.toml    # deps: serde (derive). Sonst nichts.
│       └── src/…
└── src/                  # Prototyp läuft unverändert weiter
```

- Root-`Cargo.toml` bekommt `[workspace] members = ["crates/*"]`; der Prototyp
  bleibt als Root-Package lauffähig (`cargo run` unverändert).
- `stellwerk_sim` Dependencies: **nur `serde`** (GDD §12.4 Pkt. 3). `ron` nur
  als Dev-Dependency für Szenario-Dateien in Tests.

## 3. Datenmodell

### 3.1 Einheiten (`units.rs`)

Newtypes, alle Arithmetik explizit (GDD §12.1):

| Typ | Inhalt | Bedeutung |
|---|---|---|
| `Len(i64)` | Längeneinheiten (LE) | 1 Zelle Kantenlänge = 1000 LE |
| `Tick(u64)` | Sim-Zeitschritt | nominal 10 Ticks/s (nur fürs Frontend relevant) |
| `Speed(i64)` | LE pro Tick | Zuggeschwindigkeit |
| `TrainId(u32)`, `NodeId(u32)`, `EdgeId(u32)`, `BlockId(u32)`, `SignalId(u32)`, `SwitchId(u32)` | Indizes | niemals Pointer/Refs im Zustand |

Segmentlängen als Konstanten-Tabelle (nur Konsistenz zählt, nicht Geometrie-
Exaktheit): Gerade = 1000, 45°-Diagonale = 1414, Kurve = Tabellenwert. Werte
beim Implementieren final fixiert und dann **nie wieder geändert** (ändert
sonst jede Bestzeit/jeden Replay-Hash).

### 3.2 Gitter & Anlage (`grid.rs`, `layout.rs`)

- Quadratgitter; Zellanschlüsse an den 8 Kantenmitten/Ecken (N, NO, O, …).
- Ein **Gleisstück** = Zelle + Anschlusspaar (z. B. W↔O gerade, W↔NO Kurve).
- Eine **Weiche** = Zelle mit drei Anschlüssen (Stamm + 2 Zweige) +
  Konfiguration: `Grundstellung` (Zweig-Index) + geordnete Liste von
  `Weichenregeln` (Bedingung: Zielbahnhof oder Zugtyp → Zweig; erste
  passende Regel gewinnt, sonst Grundstellung). GDD §7.3.
- Ein **Signal** = gerichteter Anker an einem Gleisstück-Ende +
  `SignalKind { Block, Chain }`. GDD §6.
- `Layout` = Liste dieser Elemente. **Validierung** beim Bau des Sim-Graphen
  liefert die vollständige Fehlerliste; die Sim startet nie mit invalidem
  Layout. Regeln (umgesetzt in W1):
  - **Offene Gleisenden sind legal** — Sackgassen sind Laufzeit-Fehlleitung
    (Szenario 11), und Quell-/Senken-Anschlüsse sind bauartbedingt offen.
  - Knicke (< 90°-Anschlusspaare) und doppelte Stücke sind illegal.
  - **Verzweigung nur per Weiche:** max. 2 Gleisenden je Anschlusspunkt;
    eine Zelle darf jeden Anschluss nur einmal belegen (keine Haarnadeln).
  - Signal nur auf vorhandenem Gleisanschluss; Weichen-Zellen sind exklusiv;
    Quelle/Senke müssen auf Gleis ankern; Fahrplan-Referenzen müssen
    existieren; `speed < 500 LE/Tick` (Anti-Tunneling, kürzester Stub).

### 3.3 Abgeleiteter Spurgraph (`graph.rs`)

Aus Level + Layout wird einmalig (bei Sim-Start) ein gerichteter Graph
gebaut — Zellen sind danach irrelevant:

- **Stub-Modell** (umgesetzt in W1): Knoten sind Anschlusspunkte *und*
  Zellmittelpunkte; jedes Gleisstück = zwei „Stubs" (Anschluss↔Mitte), jeder
  Stub = zwei gerichtete Kanten. Längen komponieren aus den Halblängen
  (kardinal 500, diagonal 707) — Weichen (3 Stubs) und Stücke addieren sich
  dadurch exakt konsistent. Jede Kante kennt ihre Fortsetzung vorab
  (`Next::Fixed`/`SwitchChoice`/`DeadEnd`); Züge fahren nur vorwärts
  (GDD §7.2), die Richtungskante kodiert das.
- Weichen werden zu Mittelknoten mit Verzweigungs-Metadaten: Stamm-Einfahrt →
  `SwitchChoice` (Routing-Entscheid §4.3), Zweig-Einfahrt → fix zum Stamm.
- Bekannte M0-Grenze (bewusst): Kreuzen sich zwei Routen in einer Zelle,
  teilen sie Mittelknoten und damit den Block — Schutz via Blocksignale;
  eine geometrische Kollision *am* Kreuzungspunkt selbst wird nicht separat
  erkannt.
- **Blockableitung** (`blocks.rs`): Signale schneiden den Graphen in Blöcke
  (Flood-Fill zwischen Signalankern). Signallose Teilnetze = ein großer Block
  ohne Schutz — dort sind Kollisionen möglich (gewollt, GDD §7.6).

### 3.4 Level & Fahrplan (`level.rs`)

```
Level {
  buildable: Zellmenge,
  fixed_tracks: vorgegebene Gleise (nicht abreißbar),
  sources: [{ id, zelle, richtung }],
  sinks:   [{ id, zelle, label }],
  schedule: [{ train_id, typ, länge: Len, speed: Speed,
               source, sink, depart: Tick, due: Tick }],
  par: { durchsatz: Tick, material: u32, verspätung: u64 },
}
```

Level-Dateien als RON (GDD §12.2), in M0 nur von Tests gelesen.

### 3.5 Zug-Zustand (`train.rs`)

Ein Zug ist ein **Intervall auf einem Kantenpfad**: Liste der zuletzt
befahrenen Kanten + Position der Spitze auf der aktuellen Kante (`Len`) +
Zuglänge. Belegung = alle Kanten, die das Intervall schneidet ⇒ lange Züge
belegen mehrere Blöcke gleichzeitig (GDD §7.2). Spawn: Zug „wächst" aus der
Quelle herein; Ankunft: Spitze erreicht Sink-Anker = Ankunftstick, danach
„schrumpft" er hinaus.

## 4. Kernalgorithmen

### 4.1 Tick-Loop (`sim.rs`)

Feste Phasenfolge pro Tick — Reihenfolge ist Teil des Determinismus-Vertrags
und wird im Code dokumentiert:

1. **Spawn:** fällige Fahrplan-Einträge; Quelle belegt → FIFO-Queue je Quelle
   (GDD §7.5).
2. **Signalauswertung:** für jeden wartenden/heranfahrenden Zug in
   aufsteigender `TrainId`-Reihenfolge: Blocksignal (Folgeblock frei?) bzw.
   Kettensignal (alle Blöcke bis einschließlich des nächsten
   blocksignal-geschützten frei? → reserviert sie als Fahrstraße, GDD §7.4).
   Konkurrenz: first-come, Tie-Break niedrigere `TrainId`.
3. **Bewegung:** jeder Zug fährt `min(speed, Distanz bis Haltepunkt)`;
   Haltepunkt = rotes Signal oder Blockgrenze ohne Freigabe. An Weichen
   entscheidet die Konfiguration (§4.3) — keine Laufzeitwahl.
4. **Ankunft/Abräumen:** Züge am Sink despawnen, Blöcke/Reservierungen frei.
5. **Fehlschlag-Checks** (§4.4) und **Erfolgs-Check** (alle Fahrplan-Züge
   korrekt angekommen → `Outcome::Success(Score)`).

API-Oberfläche (alles, was M1 je braucht — bewusst klein):

```rust
Sim::new(&Level, &Layout) -> Result<Sim, Vec<ValidationError>>
sim.step() -> &[SimEvent]          // ein Tick; Events u. a. für Audio/UI
sim.run(max: Tick) -> Outcome      // headless bis Ende/Abbruch
sim.snapshot() -> &SimState        // reine Daten, Frontend interpoliert
sim.replay_hash() -> u64           // FNV-1a über den kanonischen Zustand
check_reachability(&Level, &Layout) -> Vec<Unreachable>  // Editor-Warnung
score(&Sim) -> Score               // Durchsatz/Material/Pünktlichkeit
```

`SimEvent`: `TrainSpawned`, `TrainArrived`, `SignalChanged`, `SwitchPassed`,
`RunEnded(Outcome)` — bewusst grob; Feinheiten zieht sich das Frontend aus
Snapshot-Diffs.

### 4.2 Block-/Kettensignal-Logik (`signals.rs`)

- Blockbelegung = Menge `BlockId → TrainId` (Belegung durch Zug-Intervall)
  **plus** Reservierungen (Kettensignal-Fahrstraßen).
- Kettensignal-Reservierung folgt der Weichenkonfiguration des konkreten Zugs
  (die Fahrstraße ist eindeutig, weil Routing deterministisch ist).
- Reservierung wird Block für Block freigegeben, sobald das Zugende ihn
  verlassen hat.

### 4.3 Weichen-Routing (`routing.rs`)

`resolve(switch, train) -> Zweig`: erste passende Regel (Ziel, dann Zugtyp —
Auswertungsreihenfolge = Listenreihenfolge des Spielers), sonst Grundstellung.
Pure Funktion, trivial testbar. Dazu `check_reachability`: BFS je
Fahrplan-Zug unter Anwendung seiner effektiven Weichenentscheidungen — Wege,
die nie zum Sink führen, melden die erste „wegführende" Weiche (GDD §7.6).

### 4.4 Fehlschlag-Erkennung (`failure.rs`)

- **Kollision:** zwei Zug-Intervalle überlappen auf derselben Kante (oder
  Gegenrichtungs-Kantenpaar). Check nach der Bewegungsphase; Bewegungs-Schritt
  klein genug, dass kein „Durchtunneln" möglich ist (max Speed < kürzeste
  Kante — Validierungsregel).
- **Deadlock:** Wait-for-Graph (Zug → Zug, der seinen benötigten Block
  hält/reserviert); Zyklus via DFS = Deadlock, Zyklus wird im Outcome
  mitgeliefert (GDD §3 Säule 4: der Report braucht ihn). Fallback: kein Zug
  bewegt sich N Ticks (N = Konstante, z. B. 600) und Fahrplan unfertig →
  `Stillstand` mit den vordersten Wartenden als „Verdächtige".
- **Fehlleitung:** Zugspitze erreicht falschen Sink oder Kante ohne
  Fortsetzung → Outcome enthält gefahrenen Weg + letzte Weiche, an der die
  effektive Route vom Soll-Sink wegführte (rekonstruiert per BFS-Vergleich).

### 4.5 Determinismus-Regeln (verbindlich für jeden Commit in `stellwerk_sim`)

1. Kein `f32`/`f64`, kein `HashMap`/`HashSet` im Sim-Zustand oder in
   Iterationen, die den Zustand ändern (`BTreeMap`/`Vec` + feste Sortierung).
2. Alle Schleifen über Züge/Signale/Blöcke in aufsteigender Id-Reihenfolge.
3. Replay-Hash: handgerollter FNV-1a-64 über die kanonische Serialisierung
   des Zustands (std-`Hasher` ist prozess-seeded — ungeeignet). Hash wird pro
   Tick fortgeschrieben; `Outcome` enthält den finalen Hash.
4. Jede Konstante, die Längen/Speeds/Timing betrifft, lebt in `units.rs` und
   gilt nach M0 als eingefroren.

## 5. Die 20 Exit-Szenarien (`tests/scenarios/`)

Je ein RON-Paar (Level + Layout) + erwartetes `Outcome`/Teilzustände; ein
gemeinsamer Test-Treiber lädt und prüft. Nummerierung = Implementierungs-
reihenfolge:

| # | Szenario | Prüft |
|---|---|---|
| 1 | Einzelzug, gerade Strecke, kommt an | Bewegung, Ankunftstick, Durchsatz |
| 2 | Einzelzug über Kurven/Diagonalen | Längentabelle, Pfad-Intervall |
| 3 | Zwei Züge nacheinander, Blocksignal | Folgeblock-Logik, kein Auffahren |
| 4 | Wie 3, ohne Signal | Auffahr-Kollision wird erkannt |
| 5 | Gegenverkehr eingleisig, ungesichert | Frontalkollision (Gegenrichtungs-Kanten) |
| 6 | Gegenverkehr mit Ausweiche + Signalen | beide kommen an (Kapitel-2-Kernfall) |
| 7 | Weiche: Grundstellung | Routing-Default |
| 8 | Weiche: Zielregel überschreibt Grundstellung | Regel-Auswertung |
| 9 | Weiche: Zugtyp-Regel + Regelreihenfolge | erste passende Regel gewinnt |
| 10 | Fehlleitung: falscher Sink | Outcome + verantwortliche Weiche |
| 11 | Fehlleitung: Sackgasse | Kante ohne Fortsetzung |
| 12 | `check_reachability` meldet unerreichbaren Zug vorab | Editor-Warnpfad |
| 13 | Kreuzung nur mit Blocksignalen | Deadlock entsteht, Zyklus korrekt benannt |
| 14 | Gleiche Kreuzung mit Kettensignal | läuft durch (Kapitel-3-Kernfall) |
| 15 | Kettensignal-Reservierung blockiert Querverkehr erst ab Anspruch | Fahrstraßen-Timing, first-come + Tie-Break |
| 16 | Blockierte Quelle | FIFO-Einfahrt, Verspätung läuft (GDD §7.5) |
| 17 | Langer Zug belegt zwei Blöcke gleichzeitig | Intervall-Belegung |
| 18 | Stillstand ohne Zyklus | Fallback-Abbruch nach N Ticks |
| 19 | Bewertung komplett | Durchsatz/Material/Verspätung exakt erwartete Werte |
| 20 | Determinismus: Szenario 14 zweimal + nach Serde-Roundtrip | identische Hash-Folge, bit-identisches Outcome |

Zusätzlich (kein Szenario, aber CI): Hash-Goldwerte aller 20 Szenarien als
Konstanten committed → Windows- und Linux-Job müssen dieselben produzieren.

## 6. Wochenplan

| Woche | Liefert | Szenarien grün |
|---|---|---|
| **W1** | Workspace-Umbau; `units`, `grid`, `layout` + Validierung, `level`; Graphableitung + Blockableitung; Test-Treiber + RON-Szenarioformat | — (Treiber steht, Szenarien 1–2 als Fixtures angelegt) |
| **W2** | Tick-Loop, Bewegung/Intervalle, Blocksignale, Kollisionserkennung, Spawn/Ankunft | 1–6, 17 |
| **W3** | Weichen-Routing + Regeln, Fehlleitung, `check_reachability`, Kettensignale + Fahrstraßen-Reservierung | 7–12, 14–15 |
| **W4** | Deadlock (Wait-for-Graph) + Stillstand-Fallback, FIFO-Quelle, Score, Replay-Hash, CI-Workflow (Win+Linux, clippy, fmt), API-Doku (`lib.rs`-Rustdoc) | 13, 16, 18–20 |

Puffer: Die Szenarien 13/15 (Reservierungs-Timing) sind erfahrungsgemäß die
Bug-Nester — W4 enthält bewusst wenig Neues.

## 7. Risiken & Stolpersteine

| Risiko | Plan |
|---|---|
| Kettensignal-Semantik subtil falsch (Factorio-Edge-Cases) | Szenarien 13–15 zuerst als *failing tests* formulieren, dann implementieren; Factorio-Verhalten als Referenz dokumentieren |
| Durchtunneln bei hohen Speeds | Validierungsregel max-Speed < kürzeste Kantenlänge; Test mit Maximal-Speed |
| Blockableitung an Weichen mehrdeutig (Signal direkt an Weichenzelle) | Regel festlegen: Signalanker nur an Gleisstück-Enden, Weichenzellen signalfrei — Editor erzwingt das (in GDD §6 nachtragen, falls Playtests es bestätigen) |
| Hash-Drift durch Refactorings | Hash speist sich aus expliziter `canonical_bytes()`-Funktion, nicht aus `#[derive]`-Reihenfolge |
| Scope-Sog Richtung Frontend | M0 endet ohne ein einziges gerendertes Pixel — Erfolg ist ein grüner CI-Lauf |

## 8. Definition of Done (M0)

- [ ] `cargo test --workspace` grün auf Windows + Linux (CI)
- [ ] Alle 20 Szenarien implementiert, Hash-Goldwerte committed
- [ ] `stellwerk_sim` baut ohne Bevy in < 10 s clean auf CI
- [ ] Kern-API (§4.1) rustdoc-dokumentiert; Determinismus-Regeln (§4.5) als
      Modul-Doku in `lib.rs`
- [ ] GDD-Abgleich: alle während M0 getroffenen Design-Abweichungen sind ins
      GDD zurückgeflossen (Historie-Eintrag)
