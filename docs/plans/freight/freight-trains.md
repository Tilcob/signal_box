# Güterzüge & Frachtbahnsteige — Implementierungsplan

Status: Implementiert (M1–M5). Level `k4_01_erste_fracht` geblesst.

## Ziel

Neben den normalen (Personen-)Zügen gibt es **Güterzüge (GZ)**. Ein GZ muss auf
dem Weg zu seinem Ziel an einem **zugewiesenen Frachtbahnsteig** anhalten, dort
eine feste Zeit **abladen** (Verweildauer), und dann weiterfahren — alles ohne
Kollision und ohne Deadlock. Der haltende GZ belegt seinen Block und wird damit
zum zeitlich begrenzten Hindernis: genau das ist der Puzzle-Reiz, und er
komponiert mit Block-/Kettensignalen und Ausweichgleisen.

Realisiert das geplante Kapitel **„Sortierwerk"** (chapter.4).

## Abgestimmte Entscheidungen

1. **Abladen = fester Halt** (Verweildauer in Ticks), keine Quote.
2. **Bestimmter Bahnsteig pro GZ** — jeder GZ trägt Fracht für *einen*
   zugewiesenen Bahnsteig. Echtes Routing-Puzzle.
3. **Nur Kampagne (authored)** in v1 — keine Sandbox-Werkzeuge für GZ/Bahnsteige.
4. **Abwärtskompatible Codes** — alte Codes (ohne Fracht) bleiben importierbar.

## Angenommene Grundregeln (bei Widerspruch anpassen)

- Der **Spieler routet** den GZ per Weichen über den Bahnsteig. Kein Auto-Pathing
  — Routing bleibt der deterministische Walk, der Bahnsteig ist eine Zelle *auf*
  dem Walk.
- Ein Level **mischt** Personen- und Güterzüge.
- v1: **ein** Pflicht-Halt pro GZ (keine Mehrfach-Abladung).
- GZ = eine `TrainClass` (Reuse) plus der Pflicht-Halt am ScheduleEntry.

## Harte Design-Constraint (aus dem Sim-Kern, nicht verhandelbar in v1)

Züge fahren **nur vorwärts** (kein Reverse), und Routing ist ein
**gedächtnisloser** Walk: eine Weiche löst per `(class, sink)` **jedes Mal
gleich** auf. Konsequenz:

> **Der Bahnsteig muss auf durchgehender Strecke liegen, die der GZ geradewegs
> überfährt.** Ein Bahnsteig auf einem Stichgleis (einfahren, halten, wieder
> raus) ist **unmöglich** — der Zug kann nicht rückwärts, und an der Einfahrt-
> Weiche würde er beim „Rausfahren" denselben Ast nehmen wie beim Reinfahren.

Das ist keine Umsetzungsfrage, sondern eine **Grenze des Puzzle-Raums**: „GZ
fährt zur Rampe und hält" wie im echten Sortierwerk geht in v1 geometrisch
nicht. Level-Design muss Bahnsteige als **Durchfahrt-Halte** setzen. Eine
Stichgleis-Rampe wäre ein *anderes, viel teureres* Feature (Reverse/Kopfmachen
im Sim-Kern) — bewusst nicht v1.

---

## Architektur: betroffene Schichten

| Schicht | Datei(en) | Art des Eingriffs |
|---|---|---|
| Sim-Datenmodell (frozen) | `crates/stellwerk_sim/src/level/core.rs`, `units.rs` | **Breaking** — neue Typen |
| Sharing-Codes | `crates/stellwerk_codes/src/lib.rs` | VERSION-Bump + v3-Migration |
| Routing/Validierung | `crates/stellwerk_sim/src/routing.rs`, `graph.rs` | Walk muss Bahnsteig passieren |
| Simulation | `crates/stellwerk_sim/src/sim.rs`, `train.rs` | Dwell-Halt, Ankunft-Gating |
| Rendering | `src/board/draw.rs`, `run_board.rs`, `palette.rs` | GZ-Optik, Bahnsteig, Dwell-Anzeige |
| Validierungs-UI | `src/editor/validation.rs`, `src/ui/valerr.rs`, `assets/i18n/*` | neue Fehlermeldung |
| Onboarding | `assets/i18n/*` (`hint.*`) | Ersthilfe im Sortierwerk-Level |
| Par/Due-Tooling | `tools/due_suggest.rs`, `tools/par_suggest.rs` | Dwell in Basis-Lösung |
| Content | `assets/levels/…` | authored Sortierwerk-Level |

---

## 1. Datenmodell (frozen sim core)

`level/core.rs` ist byte-stabil (postcard, positional). Jede Änderung hier
erzwingt einen `stellwerk_codes::VERSION`-Bump + Migration — dafür gibt es ein
etabliertes Muster (siehe §2).

**Neuer Unit-Typ** (`units.rs`): `PlatformId(u32)` analog zu `SinkId`/`SourceId`.

**Neues Element** — der Frachtbahnsteig, analog zu `SinkDef`:

```rust
pub struct PlatformDef {
    pub id: PlatformId,
    pub cell: Cell,
    pub dir: Dir8,   // Anschluss, an dem der GZ hält (wie Sink-Anker)
    pub label: String,
}
```

**`Level`** bekommt ein Feld:

```rust
pub platforms: Vec<PlatformDef>,   // #[serde(default)] für alte RON-Levels
```

**`ScheduleEntry`** bekommt den optionalen Pflicht-Halt (Personenzug = `None`):

```rust
pub stop: Option<PlatformStop>,    // #[serde(default)]

pub struct PlatformStop {
    pub platform: PlatformId,
    pub dwell: Tick,
}
```

`Option`/`Vec` kodieren in postcard sauber positional. `#[serde(default)]` hält
bestehende **RON-Level-Dateien** parsebar (gleicher Kniff wie das nachgerüstete
`SourceDef.label`).

**Compile-Fanout (nicht unterschätzen):** `#[serde(default)]` rettet nur die
*De*serialisierung, **nicht** die literale Konstruktion. Ein neues `Level`-Feld
bricht **jedes** literale `Level { … }` ohne `..default()` — belegt in
`editor/ops.rs`, `editor/placement.rs` (3×), `editor/tools/track.rs`, plus
Sandbox-/Editor-Konstruktion; ein neues `ScheduleEntry.stop` jedes literale
`ScheduleEntry { }` (u.a. der Schedule-Editor „+ ZUG"). Alles mechanisch (`stop:
None`, `platforms: vec![]`), aber es rippelt durch den halben Frontend-Code —
der Compiler zeigt jede Stelle, einplanen.

---

## 2. Sharing-Codes: VERSION 3 → 4 + Migration

`stellwerk_codes/src/lib.rs`: `VERSION` von `3` auf `4`.

**Achtung — Byte-Falle im bestehenden Muster (der eigentliche Aufwandstreiber):**
`LevelV1` und `LevelV2` haben `schedule: Vec<ScheduleEntry>` — den **LIVE**
`ScheduleEntry` (importiert aus `stellwerk_sim::level`), *keine* eingefrorene
Kopie; ebenso reusen sie live `SinkDef`, `Par`, `SourceDef` (V2). Das hält nur,
solange diese Typen sich nie ändern. **Hänge ich dem live `ScheduleEntry` ein
`stop`-Feld an, ändert sich das Byte-Layout, auf das die v1- UND v2-Dekoder
zugreifen → jeder bestehende v1/v2-Golden-Code bricht.** Das „Muster" ist also
nicht Vorlage, sondern Falle.

**Richtiger Umbau:**
- Eine **eingefrorene `ScheduleEntryV3`-Kopie** (heutige 8 Felder) einführen und
  `LevelV1`/`LevelV2`/`LevelV3` **darauf** umstellen — sie dürfen den live
  `ScheduleEntry` nicht mehr referenzieren. (Gleiches Prinzip prüfen für
  `SinkDef`/`Par`/`SourceDef`: solange die live unverändert bleiben, dürfen v1/v2
  sie weiter reusen — aber das ist genau die Annahme, die hier gerade gekippt
  ist. Sicherer: die vom Fracht-Umbau berührten Typen einfrieren.)
- Neues Modul `v3` kapselt die **heutige** (Prä-Fracht-)`Level`-Form als
  `LevelV3` (mit `ScheduleEntryV3`), `v3::migrate` hebt an: `platforms = vec![]`,
  jeder Eintrag `stop = None`.
- `decode()` bekommt den Arm `3 => v3::migrate(...)`; v1/v2/v3 bleiben lesbar
  (Ziel: **abwärtskompatibel**).
- Golden-Codes: **erst** eine Regression schreiben, die die bestehenden v1/v2/v3
  Golden-Codes *nach* dem Feld-Umbau noch dekodiert (sonst merkt man den Bruch
  nicht), dann einen v4-Golden ergänzen.

> **Aufwandstreiber Nr. 1** — und *nicht* „gut vorgezeichnet": das vorhandene
> Muster muss erst entschärft werden, bevor es Vorlage sein kann.

---

## 3. Routing & Validierung

Routing ist ein deterministischer **Walk** (`walk_route`), Weichen lösen per
`resolve(switch, class, sink)`. Der Bahnsteig ist eine **Zelle auf dem Walk**,
kein zweites Routing-Ziel: der GZ fährt seinen normalen Walk Quelle→Ziel, der
zufällig (weil der Spieler so geroutet hat) den Bahnsteig-Anschluss kreuzt.

**`walk_route` erweitern**, sodass es meldet, *welche* Bahnsteig-Anker der Walk
kreuzt (und in welcher Reihenfolge relativ zum Sink). Konkret: neben `RouteEnd`
einen Satz „passierte Bahnsteig-Kanten" führen.

**`check_reachability`** (Editor-Vorabprüfung) zusätzlich prüfen: für jeden GZ
muss der Walk von der Quelle den **zugewiesenen** Bahnsteig-Anker **vor** dem
Sink kreuzen. Sonst neuer Validierungsfehler „GZ erreicht seinen Bahnsteig
nicht" (bzw. „nur nach dem Ziel"). Personenzüge unverändert.

---

## 4. Simulation (der heikelste Teil)

`sim.rs`, `train.rs`.

**`Train`** bekommt Dwell-Zustand:

```rust
pub stop: Option<PendingStop>,   // aus ScheduleEntry.stop übernommen
// PendingStop { arrival_edge: EdgeId, dwell_remaining: Tick, done: bool }
```

**`platform_by_arrival: BTreeMap<EdgeId, PlatformId>`** in `Sim` (analog
`sink_by_arrival`), damit Bahnsteig-Übergänge beim Kopf-Fortschritt erkannt
werden.

**Ablauf pro Zug:**
1. Kopf erreicht die Bahnsteig-Kante des zugewiesenen Bahnsteigs und `!done`
   → **Dwell beginnt**: der Zug hält, `dwell_remaining` zählt pro Tick runter,
   der Zug belegt weiter seinen Block.
   **Nicht** der Signal-Halt: ein Zug wartet heute, weil er für die nächste
   Kante *keine `clearance`/`grant`* bekommt (Block voraus nicht frei). Der Dwell
   hält auf **freier** Strecke — das ist eine **neue Halte-Ursache**, die in die
   Bewegungs-/Clearance-Schleife eingewoben werden muss (vor dem Grant-Versuch:
   „hat dieser Zug einen offenen Dwell? → diesen Tick nicht fortschreiten").
2. `dwell_remaining == 0` → `done = true`, Zug fährt weiter (Ziel = Sink).
3. Kopf erreicht den **Sink** → Ankunft zählt nur als Erfolg, wenn `stop` `None`
   oder `done`. Sink erreicht **ohne** erledigten Halt → **Misrouting**
   (Detail: „Fracht nicht abgeliefert"; entweder neuer `Outcome`-Detailwert oder
   Wiederverwendung von `Misrouting`).

**Kritische Subtilität 1 — Dwell vs. Stall/Deadlock-Erkennung:**
Ein dwellender Zug bewegt sich absichtlich `dwell` Ticks nicht. Die
Stall-Erkennung (`STALL_TICKS`, keine Bewegung ⇒ `Stalled`) und die
Deadlock-Zyklus-Erkennung dürfen einen **legitimen Dwell nicht** als Stillstand
werten. → Beim Zählen von „keine Bewegung" dwellende Züge ausnehmen, solange
`dwell_remaining > 0`. Bestehende Stall/Deadlock-Logik erst lesen, dann
minimal-invasiv erweitern.

**Kritische Subtilität 2 — Dwell darf die Signal-Priorität nicht vergiften:**
Die Zug-Reihenfolge läuft über `(waiting_since, id)`/`effective_priority`
(First-come an roten Signalen). Der Dwell **darf `waiting_since` nicht setzen** —
sonst gewänne ein gerade abladender GZ künstlich Vorrang an Signalen, oder die
Wait-for-Graph/Deadlock-Kanten verfälschen sich. Dwell ist ein **eigener**
Zustand neben `waiting_since`, nicht dasselbe.

**Determinismus/Hash:** der Dwell-Zustand (`dwell_remaining`, `done`) muss in den
Sim-Hash (`Fnv1a64`) einfließen, sonst driften Replays/Codes.

---

## 5. App-Schicht (Bevy)

- **Rendering** (`board/draw.rs`, `run_board.rs`, `palette.rs`):
  - GZ visuell klar unterscheidbar (eigene Farbe/Form — kein Text).
  - Bahnsteig als Element rendern (Stil an `draw_stations` anlehnen), eigener
    Palette-Eintrag.
  - **Dwell-Anzeige** am haltenden Zug: schrumpfender Ring / Restsekunden —
    visuell, damit der Spieler den Halt versteht (Anti-Text-Prinzip).
- **Validierungs-UI** (`editor/validation.rs`, `ui/valerr.rs`, `assets/i18n`):
  neue lokalisierte Meldung (de/en) für „GZ passiert Bahnsteig nicht". i18n-
  Coverage-Test grün halten.
- **Onboarding**: ein `hint.<sortierwerk-level-id>` (Mechanik erklärt sich sonst
  nicht). Recall-Button-System existiert bereits.

---

## 6. Par/Due-Tooling

`tools/due_suggest.rs`, `tools/par_suggest.rs`: die Verweildauer verlängert die
Ankunftszeit des GZ. Die Basis-Lösung, aus der `due`/`par` gemessen werden, muss
den Dwell **mitrechnen**, sonst stimmen Medaillen/Lateness nicht. Blessing-Fluss
unverändert (`due_suggest --write` → `par_suggest --write`).

---

## 7. Content

Ein bis mehrere authored **Sortierwerk-Level** (`assets/levels/…`): Bahnsteige +
GZ-Schedule-Einträge, dazu ein Ersthilfe-Hint. Fängt klein an (ein GZ + ein
Personenzug, ein Ausweichgleis-artiges Layout), steigert sich.

---

## Reihenfolge / Meilensteine

1. **Datenmodell + Codes** (§1, §2): **zuerst** die berührten Typen in
   `stellwerk_codes` einfrieren + Golden-Code-Regression schreiben, *dann* die
   neuen Felder + VERSION 4 + v3-Migration. Plus den Compile-Fanout abarbeiten
   (`platforms: vec![]` / `stop: None` an allen literalen Konstruktionen). Reiner
   Kern, keine Spielbarkeit — aber das Fundament mit dem Byte-Vertrag, und der
   Teil, der am leisesten kaputtgeht.
2. **Routing/Validierung** (§3): Walk erkennt Bahnsteig-Durchfahrt,
   `check_reachability` prüft GZ. Testbar ohne UI.
3. **Sim** (§4): Dwell-Halt + Ankunft-Gating + Stall/Deadlock-Ausnahme + Hash.
   Der inhaltliche Kern; szenariogetestet in `crates/stellwerk_sim/tests`.
4. **Rendering + Validierungs-UI** (§5): sichtbar/spielbar.
5. **Par/Due + Content** (§6, §7): ein echtes Sortierwerk-Level, geblesst.

Jeder Meilenstein clippy-grün (`--workspace --all-targets -D warnings`) und mit
Sim-Szenariotests, wo Kern-Logik dranhängt.

---

## Risiken / offene Punkte

- **Migrations-Byte-Falle** (§2) — der größte unterschätzte Posten: v1/v2 reusen
  den live `ScheduleEntry`; das Fracht-Feld bricht sie. Erst einfrieren, dann
  erweitern. Regressionstest über die alten Golden-Codes *zuerst*.
- **Dwell-Halt in die Clearance-Schleife weben** (§4) — neue Halte-Ursache, kein
  Signal-Reuse; und `waiting_since`/Priorität nicht vergiften.
- **Stall/Deadlock vs. legitimer Dwell** (§4) — dwellende Züge beim
  „keine Bewegung"-Zählen ausnehmen.
- **Design-Grenze: Bahnsteig = Durchfahrt** (oben) — Level-Design darauf
  festnageln; keine Stichgleis-Rampen in v1.
- **`walk_route`-Erweiterung**: sauber melden, dass der Bahnsteig *vor* dem Sink
  liegt (nicht nur „irgendwo im Netz").
- **Misrouting-Outcome**: neuer Detailwert vs. Reuse — beim Bauen entscheiden,
  je nachdem wie gut die bestehende `Misrouting`-Meldung passt.

## Bewusst NICHT in v1 (YAGNI)

- Sandbox-Werkzeuge für GZ/Bahnsteige (Entscheidung 3).
- Quote/Durchsatz am Bahnsteig (Entscheidung 1).
- Mehrere Abladestellen pro GZ, Ladekapazität, sichtbares Frachtgut.
- Bahnsteig-Kapazität (mehrere GZ gleichzeitig) — v1: der Block regelt das eh.
