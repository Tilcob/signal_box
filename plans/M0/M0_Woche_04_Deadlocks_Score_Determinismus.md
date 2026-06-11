# M0 Woche 4 — Deadlocks, Einfahrts-FIFO, Bewertung, Replay-Hash, CI

**Teilabgabe W4 der [M0-Angabe](M0-angabe.md) · 10 Punkte**
Spezifikation: [Plan §4.4, §4.5, §8](M0-sim-kern.md) · GDD §7.5/§7.6/§7.7

## Ziel

Die Abschlusswoche macht aus der laufenden Sim einen *beweisbar*
deterministischen Kern: Deadlocks werden erkannt und als Zyklus gemeldet
(der interessante Fehlschlag, um den das ganze Spiel gebaut ist), belegte
Quellen puffern Züge in einer FIFO, die drei Bewertungsachsen werden
gemessen, und der Replay-Hash plus CI auf zwei Plattformen besiegeln den
Determinismus-Vertrag. Am Ende: alle 20 Szenarien grün, Definition of Done
abgehakt — M0 ist fertig.

## Worauf es ankommt

- **Zyklensuche im Wait-for-Graph** ist Lehrbuch-DFS — die Arbeit steckt
  darin, die Kanten („wer wartet auf wen?") *richtig* zu definieren. Ein
  Zug wartet auf den Halter **oder Reservierer** des Blocks, den er als
  nächstes braucht; vergiss die Reservierungen aus W3 nicht, sonst findest
  du Deadlocks zwischen Fahrstraßen nicht.
- **Hash heißt: Bytes festnageln.** `#[derive(Hash)]` und
  `DefaultHasher` sind tabu (prozess-seeded, layout-abhängig). Du
  schreibst eine explizite `canonical_bytes`-Funktion — was nicht
  hineinfließt, ist per Definition kein Sim-Zustand. Das zwingt dich, ein
  letztes Mal sauber zu entscheiden, was Zustand ist.
- **Goldwerte sind ein Werkzeug, kein Ritual:** Ändert sich ein Hash,
  hat sich Verhalten geändert. Entweder ist das ein Bug — oder eine
  bewusste Entscheidung, und dann gehören neuer Goldwert und Begründung
  in *denselben* Commit.

## Projektstruktur nach dieser Woche

```
crates/stellwerk_sim/
├── src/
│   ├── lib.rs        # erweitert: pub mod hash; pub mod score;
│   ├── hash.rs       # NEU: Fnv1a64, canonical_bytes, replay_hash
│   ├── score.rs      # NEU: Score + score(), Materialkosten
│   ├── sim.rs        # erweitert: Deadlock/Stalled, FIFO-Queues, Hash-Fortschreibung
│   └── failure.rs    # NEU (optional als eigenes Modul): Wait-for-Graph + DFS
├── tests/
│   ├── scenarios.rs  # erweitert: s13, s16, s18–s20 + Goldwert-Tabelle
│   └── scenarios/    # NEU: s13, s16, s18, s19 (s20 ist reiner Code-Test)
└── .github/workflows/ci.yml   # NEU (Repo-Wurzel!)
```

## Konzepte im Mittelpunkt

- Zyklenerkennung per DFS mit deterministischer Startreihenfolge
- FIFO-Semantik mit `VecDeque` + Tick-genauer Verspätungsmessung
- FNV-1a von Hand; Little-Endian-Byteserialisierung als Kanon
- CI als Determinismus-Beweis (gleiche Goldwerte auf Windows *und* Linux)

---

## Aufgabe 4.1 — Deadlock & Stillstand (3 Punkte)

**Was du baust:** Wait-for-Graph + Zyklensuche in Phase 5; Fallback für
zyklenfreien Stillstand. Szenarien 13 und 18 grün.

```rust
// src/sim.rs — Outcome komplettieren:
pub enum Outcome {
    // … W2/W3-Varianten …
    /// Züge warten zyklisch aufeinander (GDD §7.6) — der Zyklus beginnt
    /// bei der kleinsten beteiligten TrainId (Determinismus!).
    Deadlock { cycle: Vec<TrainId> },
    /// Kein Zug bewegt sich seit STALL_TICKS, Fahrplan unfertig, aber kein
    /// Zyklus — z. B. Warten auf einen Block, der nie frei wird.
    Stalled { waiting: Vec<TrainId> },
}
```

```rust
// src/failure.rs
use crate::units::TrainId;
use std::collections::BTreeMap;

/// Kanten des Wait-for-Graphen: blockierter Zug → Zug, der den benötigten
/// Block hält (Belegung) oder reserviert hat (W3). Ein Zug, der fahren
/// kann, taucht nicht auf.
pub fn wait_for_edges(/* Züge, Graph, Belegung, Reservierungen */)
-> BTreeMap<TrainId, TrainId> {
    todo!("Aufgabe 4.1")
}

/// Findet den ersten Zyklus, Startsuche in aufsteigender TrainId-Folge;
/// der zurückgegebene Zyklus ist so rotiert, dass die kleinste Id vorn
/// steht (sonst hinge die Fehlermeldung von der Suchreihenfolge ab).
pub fn find_cycle(waits: &BTreeMap<TrainId, TrainId>) -> Option<Vec<TrainId>> {
    todo!("Aufgabe 4.1")
}
```

> Da jeder Zug auf höchstens *einen* Block wartet, ist der Wait-for-Graph
> ein „funktionaler" Graph (max. eine ausgehende Kante pro Knoten) — die
> Zyklensuche ist damit ein simples Pointer-Jagen mit Besucht-Markierung,
> kein allgemeines Tarjan. Schreibe `find_cycle`-Unit-Tests ohne Sim:
> leere Map, Kette ohne Zyklus, Zweier-Zyklus, Zyklus mit „Schwanz"
> (A→B→C→B).
>
> **Stillstand:** Zähle Ticks ohne jede Bewegung (kein `head_dist`-
> Fortschritt, kein Spawn, keine Ankunft); bei `STALL_TICKS` (Konstante in
> `units.rs`, z. B. 600) und unfertigem Fahrplan → `Stalled` mit allen
> wartenden Zügen. Prüfe die Zyklensuche *vor* dem Stillstands-Fallback —
> ein Deadlock ist auch ein Stillstand, aber die präzise Diagnose gewinnt.

**Fixtures:** `s13_block_only_crossing.ron` ist s14 aus W3 mit
Blocksignalen statt Kettensignalen — Erwartung `Deadlock` mit genau den
zwei Kreuzungszügen im Zyklus. `s18_stall_no_cycle.ron`: ein Zug wartet
vor einem Blocksignal auf einen Block, in dem ein zweiter Zug in einer
Sackgasse verhungert… Moment — der löst `Misrouting` aus. Baue stattdessen:
Zug A wartet auf einen Block, den Zug B hält, der seinerseits *nie*
fahren kann, weil sein Weg über eine besetzte Quelle… Am einfachsten:
zwei Züge, die aufeinander zufahren und vor ihren Signalen verhungern,
ohne dass ein *Zyklus über Blöcke* entsteht (B wartet auf A's Block, aber
A wartet auf einen Block, den **niemand** hält und der trotzdem nie
freigegeben wird — z. B. dauerhaft fremd-reserviert durch einen dritten,
bereits angekommenen… nein: Reservierungen verfallen). Der ehrlichste
zyklenfreie Stillstand: **A und B warten beide auf denselben von C
gehaltenen Block, und C steht an einem roten Signal, dessen Folgeblock A
hält — aber A→C, B→C, C→A ist ein Zyklus über A.** Du merkst: Echte
zyklenfreie Stillstände sind rar. Konstruiere s18 deshalb über die
**Einfahrts-FIFO aus 4.2**: Zug A parkt dauerhaft auf der Quelle von B
(Sackgassen-Stumpf hinter der Quelle, A's Sink dort bewusst nicht
erreichbar wäre Misrouting — also: A hält planmäßig vor einem roten
Signal, das nie grün wird, weil dahinter ein angekommener Zug…). Kurz:
**s18 ist absichtlich die Denksportaufgabe dieser Woche.** Finde ein
Layout, das `Stalled` (nicht `Deadlock`, nicht `Misrouting`) produziert,
und dokumentiere es im Fixture-Kommentar. Tipp: Ein Zug, der vor einem
Kettensignal wartet, dessen Fahrstraße durch einen *stehenden, aber
zyklusfreien* Vordermann nie frei wird, während dieser Vordermann auf die
FIFO-blockierte Einfahrt eines dritten wartet, führt ans Ziel.

---

## Aufgabe 4.2 — Einfahrts-FIFO & Bewertung (3 Punkte)

**Was du baust:** Warteschlangen je Quelle (GDD §7.5) und `score()`
(GDD §7.7). Szenarien 16 und 19 grün.

```rust
// src/sim.rs — Phase 1 ersetzt den W2-Direkt-Spawn:
/// Fällige Einträge wandern ans Ende der Queue ihrer Quelle; gespawnt wird
/// vom Kopf jeder Queue (aufsteigende SourceId-Reihenfolge), sobald der
/// Block der Einfahrkante frei und unreserviert ist. Ein wartender Zug
/// existiert noch nicht in der Welt — er belegt nichts.
queues: BTreeMap<SourceId, VecDeque<usize /* Index in schedule */>>,
```

```rust
// src/score.rs
use crate::layout::{Layout, SignalKind};
use crate::units::Tick;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    /// Tick der letzten Ankunft (GDD §7.7: Durchsatz).
    pub throughput: Tick,
    /// Baukosten NUR des Spieler-Layouts: Gleis 1/Stück, Weiche 4,
    /// Blocksignal 2, Kettensignal 3 (GDD §6).
    pub material: u32,
    /// Summe der Verspätungs-Ticks über alle Züge (Ankunft − due, min 0).
    pub lateness: u64,
}

pub fn material_cost(player: &Layout) -> u32 {
    todo!("Aufgabe 4.2")
}
```

`Outcome::Success` wächst von `{ last_arrival }` zu `{ score: Score }` —
zieh die W2/W3-Fixtures nach (`expect: Success(...)` prüft jetzt gegen
Score-Felder; mindestens `throughput` in jedem Success-Szenario).

> Verspätung braucht die **Ankunftstabelle** aus W2 (`arrivals`) plus das
> `due` jedes Zugs — und sie läuft auch für Züge, die in der FIFO warten:
> genau das macht Stau vor der Einfahrt zur legitimen, aber teuren
> Strategie (GDD §7.5).

**Fixtures:** `s16_source_fifo.ron`: dritte Abfahrt fällig, während die
erste die Quelle noch blockiert (kurzer Block hinter der Quelle + rotes
Signal) — Erwartung: `Success`, Reihenfolge der Ankünfte = Fahrplan-
Reihenfolge, Verspätung > 0. `s19_full_scoring.ron`: kleines Layout, zwei
Züge, exakt vorausberechnete Werte für alle drei Achsen im `expect`
(rechne sie von Hand auf Kästchenpapier vor — das ist der Punkt der
Aufgabe; die Rechnung kommt als Kommentar ins Fixture).

---

## Aufgabe 4.3 — Replay-Hash (2 Punkte)

**Was du baust:** `src/hash.rs` mit handgerolltem FNV-1a-64 und
`canonical_bytes`; Hash-Fortschreibung pro Tick. Szenario 20 grün.

```rust
// src/hash.rs
/// FNV-1a, 64 bit — von Hand, weil std-Hasher prozess-seeded sind und
/// #[derive(Hash)] an Feldreihenfolge/Layout hängt (Determinismus-Vertrag
/// in lib.rs, Regel 3).
pub struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub fn new() -> Self {
        Fnv1a64(Self::OFFSET_BASIS)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes()); // Little-Endian ist der Kanon
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}
```

```rust
// src/sim.rs
impl Sim {
    /// Kanonische Bytes des kompletten Sim-Zustands, in dokumentierter
    /// Reihenfolge: now, dann Züge aufsteigend nach Id (id, class, length,
    /// speed, sink, due, path-Kanten, head_dist), dann Reservierungen
    /// (BTreeMap-Ordnung), dann Queues, dann next_departure, dann arrivals.
    /// Was hier fehlt, ist KEIN Zustand — diese Funktion ist die Antwort
    /// auf die Frage „was muss ein Savegame speichern?".
    fn canonical_bytes(&self, out: &mut Vec<u8>) {
        todo!("Aufgabe 4.3")
    }

    /// Fortgeschrieben am Ende jedes step(): hash = fnv(hash_bytes ‖ state).
    pub fn replay_hash(&self) -> u64 { self.hash }
}
```

**Szenario 20** ist ein reiner Code-Test (kein eigenes RON): nimm s14,
fahre es **zweimal** von frischen `Sim`-Instanzen und sammle pro Tick den
Hash — beide Folgen müssen identisch sein. Dann der Roundtrip: Level +
Layout durch `ron::to_string` → `ron::from_str` schicken, erneut fahren —
wieder dieselbe Folge. (Damit fängst du Serialisierungs-Lecks: Felder, die
beim Roundtrip verloren gehen, ändern den Hash.)

**Fragen (Notizen):** (1) Warum ist `DefaultHasher` ungeeignet?
(2) Warum eine explizite `canonical_bytes` statt den Hash aus der
Feld-Reihenfolge der Structs abzuleiten? (3) `wrapping_mul` steht im
FNV-Code — warum ist das hier *kein* Verstoß gegen die
overflow-checks-Entscheidung aus W1?

---

## Aufgabe 4.4 — CI & Abschluss (2 Punkte)

**Was du baust:** `.github/workflows/ci.yml` in der Repo-Wurzel + die
Goldwert-Tabelle.

```yaml
# .github/workflows/ci.yml
name: ci
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [windows-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      # Der Sim-Kern allein — schnell, ohne Bevy-Abhängigkeiten:
      - run: cargo test -p stellwerk_sim
      - run: cargo clippy -p stellwerk_sim --all-targets -- -D warnings
      - run: cargo fmt -p stellwerk_sim -- --check
```

> Bewusst nur `-p stellwerk_sim`: Der Bevy-Prototyp im Root-Package würde
> auf CI Minuten kosten und gehört nicht zur M0-Definition-of-Done. Der
> Workspace-Build kommt mit M1 in die Pipeline.

**Goldwerte:** In `tests/scenarios.rs` eine Tabelle
`const GOLD: &[(&str, u64)]` — pro Szenario der finale `replay_hash()`.
Ein Test fährt alle und vergleicht. Beim ersten Lauf „blessen": Hashes aus
der Testausgabe in die Tabelle übernehmen (Hinweis-Abschnitt der
[M0-Angabe](M0-angabe.md)). Wenn Windows- und Linux-Job dieselbe Tabelle
grün kriegen, ist der Determinismus-Beweis erbracht — das war der Sinn
der ganzen Integer-Übung.

**Abschluss-Checkliste** (= Definition of Done, Plan §8):

- [ ] Alle 20 Szenarien grün, Goldwerte committed, CI grün auf beiden OS
- [ ] `stellwerk_sim` baut clean in < 10 s auf CI (`cargo build -p … --timings` hilft beim Nachweis)
- [ ] Kern-API rustdoc-dokumentiert (`cargo doc -p stellwerk_sim --no-deps` ohne Warnungen)
- [ ] Plan §8 abgehakt; GDD-Historie ergänzt, falls W2–W4 Design-Entscheidungen zurückgeflossen sind (Kandidat: deine Despawn-Entscheidung aus W2!)
- [ ] Notizen-Frage: Welche W1–W4-Entscheidung hätte das GDD vorgeben müssen?

---

## So prüfst du deine Lösung

- `cargo test -p stellwerk_sim` lokal grün, dann CI auf beiden Plattformen
- `find_cycle`-Unit-Tests decken Kette/Zyklus/Zyklus-mit-Schwanz ab
- Sabotage-Probe: Vertausche testweise zwei Phasen im Tick-Loop — die
  Goldwert-Tabelle muss sofort rot werden. (Wenn nicht, hashst du zu wenig
  Zustand.) Danach zurückdrehen.
- s19-Handrechnung steht als Kommentar im Fixture

## Optionale Erweiterung

Die beiden Bonus-Aufgaben der [M0-Angabe](M0-angabe.md) passen exakt
hierher: der **Fuzz-Smoke** (100 Seed-generierte Layouts, kein Panic,
Hash-Gleichheit bei Doppellauf — eigener LCG, kein `rand`!) und der
**ASCII-Replay-Viewer** (`examples/viewer.rs`), der dir bei s13/s18
vermutlich schon gefehlt hat.
