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

Der Ergebnis-Screen bietet (im dev-Build, nur bei Erfolg) **vier** Speicher-Knöpfe.
Du entscheidest beim Klick, in welche Datei der Build wandert — **kein Umbenennen
auf der Platte** mehr:

| Knopf | schreibt |
|---|---|
| **DEV: Haupt** | `solutions/<id>.ron` (primäre Lösung) |
| **DEV: +material** | `solutions/<id>__material.ron` |
| **DEV: +durchsatz** | `solutions/<id>__durchsatz.ron` |
| **DEV: +pünktlich** | `solutions/<id>__puenktlich.ron` |

Die drei Varianten-Knöpfe sind nach den drei Par-Achsen benannt — genau der
übliche Fall „eine Lösung je Achsen-Optimum" (§1). **Erneutes Klicken desselben
Knopfes überschreibt gezielt diese eine Datei** — so iterierst du an einer Achse
weiter, ohne Karteileichen anzulegen. Nach jedem Speichern listet die Status-Zeile
die vorhandenen Lösungen des Levels (`Gesichert: … — Lösungen: <id>, <id>__material, …`),
damit ein Überschreiben sichtbar ist.

So legst du mehrere Lösungen an:

1. **Erste Lösung bauen.** Level öffnen, Strecke nach Strategie A bauen,
   **START**. Bei Erfolg den passenden Knopf drücken — z. B. **DEV: Haupt** für
   die schnelle Hauptlösung.
2. **Zweite Lösung bauen.** Dasselbe Level erneut öffnen, Strecke nach
   Strategie B bauen (anderer Kompromiss — z. B. weniger Gleis), **START**, bei
   Erfolg den Knopf der passenden Achse drücken — z. B. **DEV: +material**.
3. **Wiederholen.** Für jede weitere Achse derselbe Ablauf. Die primäre `<id>.ron`
   ist **nicht** zwingend — fürs Tooling ist egal, ob ein Bestwert aus `<id>.ron`
   oder aus einer Variante stammt; eine Variante allein erfüllt `par_proof` schon.

> **Etiketten jenseits der drei Achsen:** Der `__name`-Teil ist auf Datei-Ebene
> frei (§2, „Der Namens-Vertrag"). Brauchst du einen anderen Namen
(`__nachtsprung` o. ä.), benenne
> die Datei nach dem Speichern von Hand um — die drei Knöpfe decken nur den
> Achsen-Standardfall ab.

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

- **Falscher Knopf** → der gewählte Knopf überschreibt **seine** Datei (derselbe
  Achsen-Knopf zweimal = gewolltes Iterieren; **DEV: Haupt** überschreibt immer
  die primäre `<id>.ron`). Zwei verschiedene Strategien also auf **zwei
  verschiedene** Knöpfe legen — die Status-Zeile zeigt nach dem Speichern, welche
  Dateien existieren.
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
