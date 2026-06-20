# Alternative Lösungen für ein Level

Wie man einem Level **mehrere** Designer-Lösungen mitgibt — und warum das oft
nötig ist, um einen scharfen, **beweisbar erreichbaren** Par zu setzen.

> Voraussetzung: **dev-Build** (Standard) und Start **aus dem Repo-Verzeichnis**
> — der Speicher-Knopf schreibt nach `assets/`. Im Ship-Build
> (`--no-default-features`) gibt es ihn nicht.
>
> Diese Seite ist die Tiefenversion von
> [level-bauen.md §3, Schritt 4](level-bauen.md#3-der-autorenworkflow-schritt-für-schritt).
> Die CLI-Referenz zu `par_suggest` steht in [autoren-tools.md](autoren-tools.md).

---

## 1. Warum mehrere Lösungen?

Der Par hat **drei Achsen**, und für jede gilt *niedriger ist besser*:

| Achse | Bedeutung |
|---|---|
| `throughput` | Tick der **letzten** Ankunft (wie schnell ist alles durch?) |
| `material` | Bau-Kosten des Layouts (wie sparsam ist die Strecke?) |
| `lateness` | Summe der Verspätung über alle Züge (wie pünktlich?) |

`par_suggest` und der CI-Test `tests/par_proof.rs` fahren **jede** hinterlegte
Lösung headless durch die Sim und nehmen je Achse das **Minimum über alle
Lösungen** — also den Bestwert, *egal aus welcher Datei er stammt*.

Das ist der springende Punkt: Eine einzelne Lösung ist selten auf allen drei
Achsen gleichzeitig optimal. Eine **durchsatzschnelle** Strecke baut oft
großzügig (viel `material`); eine **materialsparsame** Strecke ist langsamer
(höherer `throughput`). Willst du einen Par, der auf jeder Achse *nah am
physikalisch Möglichen* sitzt, brauchst du in der Regel **eine Lösung je
Achsen-Optimum**.

`par_proof` macht das verbindlich: Es verlangt, dass **jede Par-Achse von
irgendeiner Lösung erreicht wird**. Drückst du also den `material`-Par tiefer,
als deine schnelle Hauptlösung schafft, wird der Test rot — bis du eine
materialsparsame Variante ablegst, die diesen Wert beweist. Unerreichbare Pars
sind damit technisch unmöglich (GDD §7.7).

---

## 2. Wie es technisch funktioniert

### Wo die Dateien liegen

```
assets/levels/
  k2_04_ueberholung.ron                  ← das Level (LevelDef: meta + sim)
  solutions/
    k2_04_ueberholung.ron                ← primäre Lösung
    k2_04_ueberholung__material.ron      ← Variante (material-optimal)
    k2_04_ueberholung__puenktlich.ron    ← weitere Variante
```

### Der Namens-Vertrag: doppelter Unterstrich

Eine Datei in `solutions/` zählt zum Level `<id>`, wenn ihr Stamm (Name ohne
`.ron`)

- **genau `<id>`** ist (die primäre Lösung), **oder**
- **mit `<id>__` beginnt** (eine Variante — `<id>`, dann **zwei**
  Unterstriche, dann ein freier Name).

Diese Regel steht identisch in `tools/par_suggest.rs` und `tests/par_proof.rs`.
Der **doppelte** Unterstrich ist Absicht: Level-`id`s enthalten nie `__`, also
kann die Variante eines Levels niemals versehentlich einem anderen Level
zugeschlagen werden.

> ⚠️ **Einfacher Unterstrich zählt nicht.** `k2_04_ueberholung_material.ron`
> (ein `_`) matcht **weder** `== id` **noch** `id__…` → die Datei wird
> **stillschweigend ignoriert**. Kein Fehler, kein Test wird rot — die Lösung
> trägt einfach nichts bei. Immer `…__name.ron`.

Der `<name>`-Teil ist frei; nimm ihn als Etikett für das, was die Variante
optimiert: `__material`, `__durchsatz`, `__puenktlich`.

### Was in einer Lösungsdatei steht

Eine Lösungsdatei ist ein **`Layout`** — nur der Spieler-Build:

```ron
(
    pieces: [ /* Gleisstücke (cell, a, b) */ ],
    switches: [ /* Weichen */ ],
    signals: [ /* Signale */ ],
)
```

Also **kein** `LevelDef`, **kein** `meta`/`sim`-Wrapper, **kein** Fahrplan und
keine Quellen/Senken — die kommen aus der Level-Datei. Beim Beweis wird die
Lösung über `validate(&level, &layout)` gegen das Level geprüft und dann
simuliert.

> Werkzeug-geschriebene Lösungen tragen die **gewickelte** RON-Form (`Tick(60)`,
> `Some(…)`) ohne `#![enable(unwrap_newtypes, …)]`-Header — dieselbe Eigenheit
> wie bei maschinengeschriebenen Level-Dateien (siehe
> [level-bauen.md §5](level-bauen.md#5-häufige-stolperfallen)). Parst identisch.

---

## 3. Schritt für Schritt: eine Variante anlegen

**Der Knackpunkt:** Der Knopf **„DEV: Lösung sichern"** im Ergebnis-Screen
schreibt **immer** die primäre `solutions/<id>.ron` — er überschreibt sie bei
jedem Klick. Es gibt **keine** Oberfläche, um direkt eine `__variante` zu
benennen. Varianten entstehen also durch **Umbenennen auf der Platte** zwischen
zwei Speichervorgängen.

So legst du eine zweite (dritte, …) Lösung an:

1. **Erste Lösung bauen.** Level öffnen, Strecke nach Strategie A bauen,
   **START**. Bei Erfolg im Ergebnis-Screen **„DEV: Lösung sichern"**.
   → schreibt `assets/levels/solutions/<id>.ron`.
2. **Umbenennen.** In `assets/levels/solutions/` die eben geschriebene
   `<id>.ron` umbenennen auf `<id>__<name>.ron`, z. B.
   `k2_04_ueberholung__durchsatz.ron`. Damit ist Strategie A „weggeräumt" und
   der primäre Slot wieder frei.
3. **Zweite Lösung bauen.** Dasselbe Level erneut öffnen, Strecke nach
   Strategie B bauen (anderer Kompromiss — z. B. weniger Gleis), **START**, bei
   Erfolg wieder **„DEV: Lösung sichern"**.
   → schreibt eine **frische** `<id>.ron`.
4. **Wiederholen.** Für jede weitere Variante: Schritt 2 (umbenennen) + Schritt 3
   (neu bauen & sichern). Optional auch die letzte primäre `<id>.ron` noch in
   `…__name.ron` umbenennen — fürs Tooling ist es egal, ob die Bestwerte aus
   `<id>.ron` oder aus Varianten kommen; die primäre Datei ist **nicht**
   zwingend (eine Variante allein erfüllt `par_proof` bereits).

> Vergisst du Schritt 2, **überschreibt** der zweite Speicher-Klick deine erste
> Lösung — sie ist dann weg. Erst umbenennen, dann neu sichern.

---

## 4. Beispiel

Level `k2_04_ueberholung`. Die schnelle Hauptlösung ist durchsatz-optimal, baut
dafür eine großzügige Ausweiche (viel `material`). Du legst daneben eine
materialsparsame Variante ab:

```
solutions/
  k2_04_ueberholung.ron              # durchsatz-optimal
  k2_04_ueberholung__material.ron    # material-optimal
```

`par_suggest` nimmt je Achse das Beste aus beiden:

```sh
$ cargo run --bin par_suggest -- k2_04_ueberholung
k2_04_ueberholung: par (throughput: 188, material: 12, lateness: 0)   [aktuell: 200, 14, 0]

(dry-run — mit `-- --write` in die Level-Dateien zurückschreiben)
```

`throughput 188` stammt aus der schnellen Lösung, `material 12` aus der
sparsamen — kein einzelner Build erreicht beides. Passt der Vorschlag,
zurückschreiben:

```sh
$ cargo run --bin par_suggest -- --write k2_04_ueberholung
```

---

## 5. Prüfen

Zwei Wege, dieselbe Wahrheit (beide nutzen exakt die obige Datei-Matching-Regel):

- **`par_suggest`** (CLI, beim Bauen): zeigt erreichte Bestwerte je Achse,
  schreibt mit `--write` die `par:`-Zeile. Lösung fehlt / validiert nicht /
  scheitert → Zeile auf **stderr**, Level übersprungen. Details:
  [autoren-tools.md §1](autoren-tools.md#1-par_suggest--par-werte-beweisbar-setzen).
- **`tests/par_proof.rs`** (CI, beim `cargo test`): verlangt, dass

  1. zu jedem Level **mindestens eine** Lösung in `solutions/` liegt,
  2. **jede** Lösung gegen das Level validiert und innerhalb von 50 000 Ticks
     `Success` liefert,
  3. **jede Par-Achse** von einer Lösung erreicht wird (Bestwert ≤ Par).

  Bei einem Par-Verfehler druckt der Test die erreichten Bestwerte je Achse
  (`cargo test -- --nocapture` zeigt sie immer) — dann **bewusst** entscheiden:
  Par anheben oder eine bessere/zusätzliche Lösung bauen.

---

## 6. Stolperfallen

- **Zweimal sichern ohne Umbenennen** → der zweite Klick überschreibt die erste
  Lösung. Reihenfolge: bauen → sichern → **umbenennen** → neu bauen → sichern.
- **Einfacher statt doppelter Unterstrich** (`<id>_name.ron`) → still ignoriert,
  trägt nichts bei. Immer `<id>__name.ron`.
- **Variante scheitert** (erreicht keine Senke, Timeout) → `par_proof` wird rot;
  `par_suggest` meldet sie auf stderr und überspringt das Level. Nur
  erfolgreiche Builds als Lösung ablegen (der Knopf erscheint ohnehin **nur bei
  Erfolg**).
- **`LevelDef` statt `Layout` in `solutions/`** → Parse-Fehler beim Beweis. Eine
  Lösungsdatei ist nur das `Layout` (pieces/switches/signals).
- **Level nachträglich geändert** (`sim`-Layout, `buildable`, Fahrplan) → eine
  gespeicherte Lösung kann ungültig werden oder andere Werte liefern. Nach
  Level-Änderungen `cargo test` / `par_suggest` neu laufen lassen und Varianten
  bei Bedarf neu bauen.
- **Variante schlechter auf allen Achsen** → harmlos, aber nutzlos: Sie wird nie
  zum Minimum und verändert den Par nicht. Eine Variante lohnt nur, wenn sie auf
  **mindestens einer** Achse die anderen schlägt.
- **`id` umbenannt** → die Lösungen heißen nach dem alten Stamm und zählen nicht
  mehr zum Level. Stämme in `solutions/` mit umziehen (und nie eine
  veröffentlichte `id` ändern, siehe
  [level-bauen.md §1](level-bauen.md#1-wo-level-leben-und-wie-sie-heißen)).
