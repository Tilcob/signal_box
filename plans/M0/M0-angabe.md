# Angabe M0 — Der Sim-Kern `stellwerk_sim`

**Projekt:** Stellwerk · **Spezifikation:** [M0-sim-kern.md](M0-sim-kern.md) + [GDD](../../GDD.md) §6/§7/§12
**Abgabe:** 4 Teilabgaben (Ende Woche 1–4) · **Punkte:** 40 (+5 Bonus)

## Lernziele

- Eine deterministische Fixed-Tick-Simulation ohne Engine entwerfen:
  Integer-Arithmetik mit Newtypes, kanonische Iterationsreihenfolge,
  Replay-Hash als beweisbares Determinismus-Kriterium.
- Zustand über Id-Indizes statt Referenzen modellieren und begründen können,
  warum das in Rust der natürliche Schnitt ist.
- Graph-Algorithmen im Anwendungskontext: Flood-Fill (Blockableitung),
  BFS (Erreichbarkeit), Zyklensuche im Wait-for-Graph (Deadlocks).
- Test-first gegen eine Spezifikation arbeiten: die 20 Szenarien aus dem Plan
  sind das Exit-Kriterium, nicht „es sieht richtig aus".

## Voraussetzungen & Spielregeln

- GDD ist Single Source of Truth, der Plan konkretisiert es. Wo Sie beim
  Implementieren vom Plan abweichen müssen: zuerst GDD/Plan ändern (mit
  Historie-Eintrag), dann Code.
- Der `[workspace]`-Eintrag in der Root-`Cargo.toml` existiert bereits; der
  Live-Dispatcher-Prototyp in `src/` bleibt unangetastet und lauffähig.
- Harte Regeln für `stellwerk_sim` (Plan §4.5 — Verstöße kosten die Punkte
  der jeweiligen Aufgabe): kein Bevy, kein `f32`/`f64`, kein
  `HashMap`/`HashSet` im Sim-Zustand, einzige Dependency `serde`
  (`ron` nur als Dev-Dependency).
- Jede Teilabgabe: `cargo test --workspace` grün, `cargo clippy -- -D
  warnings` sauber, `cargo fmt --check` sauber.
- Begründungsfragen beantworten Sie in `plans/M0-notizen.md` (nummeriert,
  2–5 Sätze je Frage) — sie sind Teil der Punkte.

---

## Teilabgabe W1 — Fundament (10 Punkte)

### Aufgabe 1.1 — Crate & Einheiten (3 Punkte)

- Legen Sie `crates/stellwerk_sim` an. Die `lib.rs`-Moduldoku enthält die
  Determinismus-Regeln aus Plan §4.5 — sie sind ab jetzt Vertrag.
- Implementieren Sie die Newtypes aus Plan §3.1 (`Len`, `Tick`, `Speed`,
  Id-Typen). Arithmetik nur dort, wo sie fachlich Sinn ergibt
  (`Len + Len` ja, `Len * Len` nein; `Speed * Tick-Differenz → Len` ja).
  Kein `Deref`-Durchgriff auf die Innenwerte.
- Die Segmentlängen-Tabelle (Gerade/Diagonale/Kurve) ist eine benannte
  Konstantengruppe in `units.rs` mit Doku-Kommentar, *warum* sie nach M0
  eingefroren ist.
- **Fragen (Notizen):** (1) Warum Id-Indizes statt Referenzen/`Rc` im
  Sim-Zustand? (2) Warum `u64`-Ticks statt `f32`-Sekunden — was genau geht
  bei Floats plattformübergreifend schief? (3) Was bricht alles, wenn die
  Längentabelle nach Release geändert wird?

### Aufgabe 1.2 — Gitter, Layout, Level, Validierung (4 Punkte)

- Datenmodell nach Plan §3.2/§3.4: Zellen mit 8 Anschlüssen, Gleisstück,
  Weiche (Grundstellung + geordnete Regelliste), Signal (gerichtet,
  Block/Kette), `Layout`, `Level` inkl. Fahrplan. Alles `serde`-fähig.
- Eine RON-Beispieldatei (Mini-Level + Mini-Layout) als Test-Fixture —
  sie ist zugleich die erste Dokumentation des Dateiformats.
- Validierung liefert eine **Liste** aller Fehler (nicht nur den ersten):
  mindestens Knick (< 90°-Paar), Verzweigung ohne Weiche (> 2 Gleisenden an
  einem Punkt / Anschluss doppelt belegt), Signal auf Nicht-Gleis, Weiche
  mit fehlendem Zweig, Fahrplan-Zug mit unbekannter Quelle/Senke, Speed ≥
  kürzeste Segmentlänge (Anti-Tunneling, Plan §4.4). **Offene Gleisenden
  sind legal** — Sackgassen sind Laufzeit-Fehlleitung (Szenario 11).
- **Frage:** Warum gehört der Anti-Tunneling-Check in die Validierung statt
  in die Bewegungslogik?

### Aufgabe 1.3 — Spurgraph & Blockableitung (3 Punkte)

- Ableitung Layout → gerichteter Graph (Plan §3.3): jedes Gleisstück zwei
  Richtungskanten, Weichen als Verzweigungsknoten.
- Blockableitung per Flood-Fill zwischen Signalankern; signallose Teilnetze
  werden genau ein Block.
- Tests mit handkonstruierten Mini-Layouts: erwartete Anzahl Knoten, Kanten,
  Blöcke; ein Layout mit 0 Signalen, eines mit 2, eines mit Weiche.
- Bauen Sie den **Szenario-Treiber**: `tests/scenarios.rs` lädt RON-Paare
  (Level + Layout + erwartetes Outcome) aus `tests/scenarios/`. Legen Sie
  die Fixtures für Szenario 1–2 bereits an (`#[ignore]`, solange die Sim
  fehlt — W2 schaltet sie scharf).

---

## Teilabgabe W2 — Züge rollen (10 Punkte)

> **Detail-Angabe mit Code-Skeletten:**
> [M0_Woche_02_Tick_Loop_und_Bewegung.md](M0_Woche_02_Tick_Loop_und_Bewegung.md)
> — die Punkteverteilung unten gilt, die Detail-Angabe sagt dir, *wie*.

### Aufgabe 2.1 — Tick-Loop & Bewegung (4 Punkte)

- Implementieren Sie `Sim::new` (Validierung → Graph → Anfangszustand) und
  `sim.step()` mit der **festen Phasenfolge** aus Plan §4.1. Die Reihenfolge
  steht als Doku-Kommentar über der Loop.
- Züge als Intervall auf Kantenpfad (Plan §3.5): Spawn wächst aus der Quelle
  herein, Ankunft = Spitze erreicht Sink-Anker (Tick festhalten), danach
  schrumpft der Zug hinaus.
- `sim.run(max)` und `sim.snapshot()` nach Plan §4.1; `SimEvent`s mindestens
  `TrainSpawned`, `TrainArrived`, `RunEnded`.
- **Szenarien 1–2 grün** (Ignore-Marker entfernen).
- **Frage:** Warum ist die Phasenreihenfolge Teil des API-Vertrags und nicht
  bloß ein Implementierungsdetail? Konstruieren Sie ein konkretes Beispiel,
  bei dem „Bewegung vor Signalauswertung" ein anderes Ergebnis liefert.

### Aufgabe 2.2 — Blocksignale & Belegung (3 Punkte)

- Blockbelegung aus Zug-Intervallen; Blocksignal hält am Signalanker, solange
  der Folgeblock belegt ist. Haltepunkt exakt am Anker (kein Überschießen).
- Konkurrenz um einen Block: first-come, Tie-Break niedrigere `TrainId`
  (Plan §4.1 Phase 2) — schreiben Sie dafür einen gezielten Unit-Test, der
  beide Fälle (verschiedene Anspruchsticks / gleicher Tick) abdeckt.
- **Szenario 3 grün.**

### Aufgabe 2.3 — Kollisionserkennung (3 Punkte)

- Intervall-Überlappung auf derselben Kante **und** auf Gegenrichtungs-
  Kantenpaaren (Frontalkollision!); Check nach der Bewegungsphase.
- `Outcome::Collision` enthält beide Züge und die Kante — der Frontend-Report
  (GDD §3 Säule 4) braucht das später.
- **Szenarien 4–6 und 17 grün.** Szenario 6 (Ausweiche mit Signalen) ist der
  erste echte Integrationstest — planen Sie hier Debugzeit ein.

---

## Teilabgabe W3 — Routing & Kettensignale (10 Punkte)

> **Detail-Angabe mit Code-Skeletten:**
> [M0_Woche_03_Routing_und_Kettensignale.md](M0_Woche_03_Routing_und_Kettensignale.md)

### Aufgabe 3.1 — Weichen-Routing (3 Punkte)

- `resolve(switch, train) -> Zweig` als pure Funktion (Plan §4.3): erste
  passende Regel gewinnt (Listenreihenfolge!), sonst Grundstellung.
- Unit-Tests ohne Sim: Regel nach Ziel, nach Zugtyp, Reihenfolge zweier
  konkurrierender Regeln, leere Regelliste.
- **Szenarien 7–9 grün.**

### Aufgabe 3.2 — Fehlleitung & Erreichbarkeit (3 Punkte)

- Fehlleitung: falscher Sink oder Kante ohne Fortsetzung → `Outcome` mit
  gefahrenem Weg und der Weiche, an der die Route vom Soll-Sink wegführte
  (BFS-Vergleich, Plan §4.4).
- `check_reachability(&Level, &Layout)` als freie Funktion für den Editor.
- **Szenarien 10–12 grün.**

### Aufgabe 3.3 — Kettensignale & Fahrstraßen (4 Punkte)

- **Test-first, verpflichtend:** Formulieren Sie die Fixtures für 14 und 15
  *bevor* Sie implementieren, und committen Sie sie rot (Commit-Historie
  zeigt das). Das ist das Bug-Nest von M0 — die Tests zuerst zu denken ist
  der Punkt dieser Aufgabe.
- Kettensignal nach GDD §7.4: grün ⇔ alle Blöcke bis einschließlich des
  nächsten blocksignal-geschützten frei; bei Freigabe wird die Fahrstraße
  reserviert; Freigabe Block für Block, sobald das Zugende ihn verlässt.
- Die Fahrstraße folgt der Weichenkonfiguration des konkreten Zugs.
- **Szenarien 14–15 grün.**
- **Frage:** Ein Kettensignal, dessen Fahrstraße über eine Weiche führt —
  warum darf die Reservierung die Weichenentscheidung nicht „raten", und
  woher kennt sie sie?

---

## Teilabgabe W4 — Fehlschläge, Bewertung, Determinismus (10 Punkte)

> **Detail-Angabe mit Code-Skeletten:**
> [M0_Woche_04_Deadlocks_Score_Determinismus.md](M0_Woche_04_Deadlocks_Score_Determinismus.md)

### Aufgabe 4.1 — Deadlock & Stillstand (3 Punkte)

- Wait-for-Graph (Zug → Halter/Reservierer des benötigten Blocks), Zyklus
  per DFS; der Zyklus steht im `Outcome` (Reihenfolge: aufsteigend ab
  kleinster `TrainId` — deterministisch!).
- Stillstand-Fallback: N Ticks ohne jede Bewegung bei unfertigem Fahrplan
  (N als Konstante in `units.rs`).
- **Szenarien 13 und 18 grün.**

### Aufgabe 4.2 — Einfahrts-FIFO & Bewertung (3 Punkte)

- FIFO-Warteschlange je Quelle (GDD §7.5): belegte Quelle verzögert, die
  Soll-Ankunftszeit läuft weiter.
- `score(&Sim) -> Score` mit den drei Achsen (GDD §7.7); Material zählt aus
  dem Layout (Kostentabelle GDD §6).
- **Szenarien 16 und 19 grün.**

### Aufgabe 4.3 — Replay-Hash (2 Punkte)

- Handgerollter FNV-1a-64 über eine explizite `canonical_bytes()`-Funktion
  des Zustands (Plan §4.5 Pkt. 3) — **nicht** über `#[derive(Hash)]` und
  nicht über `DefaultHasher`.
- Hash wird pro Tick fortgeschrieben; `Outcome` trägt den finalen Hash.
- **Szenario 20 grün:** zwei Läufe + Serde-Roundtrip ⇒ identische Hash-Folge.
- **Fragen:** (1) Warum ist `std::collections::hash_map::DefaultHasher`
  ungeeignet? (2) Warum eine explizite `canonical_bytes()` statt den Hash aus
  der Feld-Reihenfolge der Structs abzuleiten?

### Aufgabe 4.4 — CI & Abschluss (2 Punkte)

- GitHub-Actions-Workflow: `test`/`clippy -D warnings`/`fmt --check` auf
  Windows **und** Linux; die Hash-Goldwerte aller 20 Szenarien liegen als
  Konstanten im Repo und werden auf beiden Plattformen geprüft.
- Gehen Sie die Definition of Done (Plan §8) Punkt für Punkt durch und haken
  Sie sie im Plan ab; GDD-Historie ergänzen, falls unterwegs Design-
  Entscheidungen zurückgeflossen sind.
- **Frage:** Welche Ihrer W1–W4-Entscheidungen hätte das GDD eigentlich
  vorgeben müssen? (Ehrliche Antwort — „keine" ist zulässig, wenn begründet.)

---

## Bonus (+5 Punkte)

- **Fuzz-Smoke (+3):** Ein Test generiert aus einem festen Seed (eigener
  LCG, kein `rand` im Kern!) 100 zufällige kleine Layouts, lässt jedes
  500 Ticks laufen und prüft nur: kein Panic, und zweiter Lauf ⇒ identischer
  Hash. Dokumentieren Sie den ersten echten Fund.
- **ASCII-Replay-Viewer (+2):** `examples/viewer.rs` rendert ein Szenario
  Tick für Tick als Text (Gleise, Signale, Zugpositionen) in die Konsole.
  Sie werden ihn beim Debuggen von 3.3 lieben.

## Hinweise

- **Reihenfolge schlägt Cleverness:** Fast jeder Determinismus-Bug ist eine
  Iterationsreihenfolge. `BTreeMap`/sortierte `Vec`s von Anfang an; bei
  Sortierungen mit möglichen Gleichständen niemals `sort_unstable_by_key`
  auf dem Schlüssel allein — Tie-Break immer bis zur Id durchziehen.
- **Goldwerte „blessen":** Beim ersten Grünwerden eines Szenarios den Hash
  aus der Testausgabe in die Konstantentabelle übernehmen — ab dann schlägt
  jede Verhaltensänderung als Diff auf. Ein bewusst geändertes Verhalten
  heißt: Goldwert-Update im selben Commit, mit Begründung in der Message.
- **Kettensignal-Referenz:** Das Factorio-Wiki („Railway signaling") ist die
  Verhaltens-Referenz für 3.3; notieren Sie Abweichungen bewusst in den
  Notizen statt sie zufällig einzubauen.
- Der Szenario-Treiber lohnt Sorgfalt: sprechende Fehlermeldungen
  („Szenario 14, Tick 312: erwartet Success, bekam Deadlock[T2→T5→T2]")
  sparen in W3/W4 Stunden.
- `cargo test -p stellwerk_sim` hält die Schleife schnell — der Kern baut
  ohne Bevy in Sekunden; das ist der Sinn des Workspace-Schnitts.
