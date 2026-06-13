# Ein Level bauen

Praxisleitfaden für Kampagnen-Level in Stellwerk. Beschreibt das aktuelle
Dateiformat (mit Metadaten-Schicht, M2 §2.3), den Autorenworkflow und die
Tests, die grün sein müssen, bevor ein Level „fertig" ist. Am Ende ein
[Ausblick](#ausblick-kampagnen-level-künftig-leichter-bauen), wie sich das
Bauen künftig beschleunigen lässt.

> Hintergrund-Entscheidungen zum Format stehen in
> [plans/optimierung/05-level-metadaten-format.md](../plans/optimierung/05-level-metadaten-format.md).

---

## 1. Wo Level leben und wie sie heißen

```
assets/levels/
  k1_01_erste_fahrt.ron          ← das Level
  k1_02_blocktakt.ron
  …
  solutions/
    k1_01_erste_fahrt.ron        ← Designer-Lösung (nur Layout) für die Par-Härtung
    k1_01_erste_fahrt__material.ron   ← optionale Zweitlösung (z. B. material-optimal)
```

**Der Dateiname (ohne `.ron`) ist die Level-`id`** — z. B.
`k1_02_blocktakt`. Diese `id` ist der **stabile Schlüssel** für

- den Spielfortschritt (`progress.ron`),
- die Lösungs-Sharing-Codes (`level_id` im Code),
- die i18n-Keys (`level.<id>.name`, `level.<id>.briefing`),
- die Zuordnung der Designer-Lösung in `solutions/`.

> ⚠️ **Eine veröffentlichte `id` nie umbenennen.** Das bricht Saves und alle
> geteilten Codes. Die Anzeige-Reihenfolge steuert man über `meta.order`
> (siehe unten), **nicht** über den Dateinamen. Konvention `kN_MM_kurzname`
> ist nur Lesehilfe — die echte Ordnung kommt aus den Metadaten.

---

## 2. Das Dateiformat

Eine Level-Datei hat zwei Blöcke: **`meta`** (Kampagnen-Organisation) und
**`sim`** (das spielbare Puzzle). Nur `sim` ist der Simulationskern; nur er
wandert in Sharing-Codes.

```ron
#![enable(unwrap_newtypes, implicit_some)]
// Kapitel 1, Level 1 — kurze Notiz an dich selbst, was das Level lehrt.
(
    meta: (
        schema_version: 1,          // Format-Version der Datei (s. §6)
        chapter: 1,                 // Kapitel (1-basiert)
        order: 10,                  // Reihenfolge IM Kapitel (Schritte von 10)
        optional_hard: false,       // eines der letzten „optional-schweren" Level?
        briefing: "Ziehen Sie das Gleis durch und bringen Sie den Zug nach OST.",
    ),
    sim: (
        name: "1.1 Erste Fahrt",    // authored Anzeigename (i18n-Fallback)
        buildable: [(x: 0, y: 0), (x: 1, y: 0), (x: 2, y: 0), (x: 3, y: 0), (x: 4, y: 0), (x: 5, y: 0)],
        fixed: (
            pieces: [(cell: (x: 0, y: 0), a: W, b: E), (cell: (x: 5, y: 0), a: W, b: E)],
            switches: [],
            signals: [],
        ),
        sources: [(id: 0, cell: (x: 0, y: 0), dir: W)],
        sinks: [(id: 0, cell: (x: 5, y: 0), dir: E, label: "OST")],
        schedule: [
            (train: 0, class: 0, length: 800, speed: 100, source: 0, sink: 0, depart: 0, due: 70),
        ],
        par: (throughput: 60, material: 4, lateness: 0),
    ),
)
```

### `meta` — Kampagnen-Metadaten (wandert NIE in einen Code)

| Feld | Typ | Bedeutung |
|---|---|---|
| `schema_version` | `u16` | Format-Version der Datei. Aktuell `1`. Siehe [§6](#6-versionierung-zwei-getrennte-nummern). |
| `chapter` | `u8` | Kapitel, 1-basiert (GDD §8.1). Steuert Gruppierung in der Streckenwahl. |
| `order` | `u16` | Reihenfolge **innerhalb** des Kapitels. In Schritten von 10 vergeben → Platz zum Einschieben. |
| `optional_hard` | `bool` | Markiert die letzten 2–3 „optional-schweren" Level eines Kapitels. Werden in der Streckenwahl mit `(schwer)` gekennzeichnet, blockieren aber nie den Fortschritt. |
| `briefing` | `String` | Betriebsauftrag, 1–2 Sätze (GDD §8.1). Deutscher authored Text = i18n-Fallback für `level.<id>.briefing`. Wird im Edit-HUD angezeigt. |

### `sim` — der Simulationskern (das, was ein Sharing-Code trägt)

| Feld | Bedeutung |
|---|---|
| `name` | Authored Anzeigename, zugleich i18n-Fallback für `level.<id>.name`. |
| `buildable` | Liste der Zellen, auf denen der **Spieler** bauen darf. Fest verlegtes Gleis (`fixed`) ist ausgenommen. |
| `fixed` | Vom Designer vorgegebene, **unveränderliche** Infrastruktur: `pieces` (Gleise), `switches` (Weichen), `signals` (Signale). Gleiche Struktur wie ein Spieler-`Layout`. |
| `sources` | Quellen: `(id, cell, dir)` — Züge erscheinen, indem sie über den `dir`-Anschluss in `cell` einfahren. Die Zelle muss dort Gleis haben. |
| `sinks` | Senken: `(id, cell, dir, label)` — Ankunft = Zugspitze erreicht den `dir`-Anschluss von `cell`. `label` ist der Bahnhofsname (→ i18n `station.<label>`). |
| `schedule` | Fahrplan, je Zeile ein Zug (s. unten). |
| `par` | Designer-Referenzwerte je Achse: `throughput` (Tick der letzten Ankunft), `material` (Bau-Kosten), `lateness` (Summe Verspätung). Bestwert ≤ Par → Medaille. |

### Eine Fahrplan-Zeile

```ron
(train: 0, class: 0, length: 800, speed: 100, source: 0, sink: 0, depart: 0, due: 70)
```

| Feld | Einheit / Bedeutung |
|---|---|
| `train` | eindeutige Zug-Id |
| `class` | Zugklasse (offene Zahl, z. B. 0 = S-Bahn, 1 = Güter) — Kriterium für Weichen-Klassenregeln |
| `length` | Länge in **LE** (1 Zellkante = 1000 LE; `800` = 0,8 Zellen, `1800` = langer Zug über ~2 Zellen) |
| `speed` | **LE pro Tick**. Muss `< 500` sein (Tunneling-Schutz). `100` ≈ langsam, `240` ≈ schnell. |
| `source` / `sink` | Quell-/Ziel-Id aus `sources`/`sinks` |
| `depart` | Abfahrt-Tick (frühestens; die Quelle puffert FIFO) |
| `due` | Soll-Ankunft-Tick. Ankunft danach erzeugt Verspätung (Pünktlichkeitsachse). |

### Koordinaten & Anschlüsse

- **Zelle** `(x, y)`: **+x = Osten, +y = Norden** (mathematische Orientierung,
  siehe `Dir8::offset` in `grid.rs`: `N → (0, +1)`, `S → (0, -1)`,
  `E → (+1, 0)`, `W → (-1, 0)`). Eine NORD-Senke liegt also bei größerem `y`,
  eine SUED-Senke bei kleinerem.
- **Richtungen (`Dir8`)**: `N, NE, E, SE, S, SW, W, NW`.
- Ein **Gleisstück** `(cell, a, b)` verbindet zwei Anschlüsse einer Zelle,
  z. B. `(a: W, b: E)` = gerade durch, `(a: N, b: S)` = senkrecht,
  `(a: E, b: S)` = 90°-Bogen. Eine Zelle kann mehrere Stücke tragen (Kreuzung:
  `(W,E)` **und** `(N,S)`).
- Eine **Weiche** sitzt allein auf ihrer Zelle (keine losen Gleise dort):
  `(cell, stem, branches: [b0, b1], default_branch, rules)`. `stem` ist die
  Einfahrt, `branches` die zwei Ausgänge, `default_branch` die Grundstellung
  (0/1), `rules` die Zielregeln (`DestIs(SinkId)` / `ClassIs(TrainClass)`).
- Ein **Signal** `(cell, at, kind)` hängt am Anschluss `at` eines Gleises;
  `kind` ist `Block` oder `Chain` (Kettensignal).

---

## 3. Der Autorenworkflow (Schritt für Schritt)

1. **In der Sandbox entwerfen.** Sandbox = Editor + Sim wie die Kampagne, nur
   ohne Bewertung. Quellen, Senken und Fahrplan setzen, Gleisidee skizzieren.
   *Die Sandbox ist das Autorenwerkzeug.*
2. **Als Level-Code exportieren** (`LEVEL-CODE EXPORTIEREN` im Sandbox-HUD →
   `stellwerk_code.txt`). Der Code enthält das volle `sim`-Level.
3. **Datei anlegen** `assets/levels/kN_MM_kurzname.ron`. Den `sim`-Teil aus dem
   Sandbox-Export übernehmen (bzw. die `sandbox.ron` aus dem Konfigverzeichnis
   als Vorlage), in das `(meta:, sim:)`-Gerüst aus [§2](#2-das-dateiformat)
   einsetzen.
4. **Metadaten füllen:** `chapter`, `order` (nächster freier 10er-Schritt im
   Kapitel), `optional_hard`, `briefing`. `schema_version: 1`.
5. **`buildable` festlegen:** genau die Zellen, die der Spieler bebauen darf.
   Designer-Gleis gehört in `fixed`, nicht in `buildable`.
6. **`par` zunächst grob** setzen (wird in Schritt 8 hart bewiesen).
7. **i18n-Keys ergänzen** in **beiden** Tabellen `assets/i18n/de.ron` und
   `assets/i18n/en.ron`:
   - `level.<id>.name`
   - `level.<id>.briefing`
   - jedes neue `station.<LABEL>`
   Deutsche Werte = die authored Strings; Englisch = Übersetzung.
8. **Designer-Lösung bauen & hinterlegen:** das Level spielen, lösen, die
   Lösung als **Lösungs-Code** exportieren und als
   `assets/levels/solutions/<id>.ron` (nur das `Layout`) ablegen. Für jede
   Achse, die eine eigene Lösung braucht, eine Variante `…__name.ron`.
9. **Par scharfstellen:** `cargo test --test par_proof -- --nocapture` zeigt
   die je Achse **erreichten** Werte. `par` so setzen, dass jede Achse von
   einer hinterlegten Lösung erreicht wird (nie strenger).
10. **Alles grün?** Siehe [§4](#4-was-grün-sein-muss). Dann ist das Level fertig.
11. **Tempo notieren** in [plans/M2/content-log.md](../plans/M2/content-log.md)
    (Exit-Kriterium: < 1 Tag/Level).

---

## 4. Was grün sein muss

```bash
cargo test
```

| Test | prüft |
|---|---|
| `tests/levels.rs · all_levels_parse_and_validate_empty` | Datei parst als `LevelDef`; mit leerem Spieler-Layout valide; Fahrplan nicht leer; `material`-Par > 0; **Metadaten-Lint**: `schema_version` aktuell, `chapter > 0`, `order > 0`, `briefing` nicht leer. |
| `tests/par_proof.rs · every_level_par_is_proven` | Zu jedem Level liegt ≥ 1 Lösung in `solutions/`, jede liefert `Success`, und **jede Par-Achse** wird von einer Lösung erreicht. Unerreichbare Pars sind damit unmöglich. |
| `tests/i18n.rs · every_level_name_has_a_key_in_both_languages` | `level.<id>.name` in de **und** en. |
| `tests/i18n.rs · every_level_briefing_has_a_key_in_both_languages` | `level.<id>.briefing` in de **und** en. |
| `tests/i18n.rs · every_station_label_has_a_key` | jedes `sink.label` hat `station.<LABEL>` in beiden Tabellen. |
| `tests/i18n.rs · language_tables_cover_identical_keys` | beide Tabellen haben exakt denselben Key-Satz. |

> Wenn `all_levels_parse_and_validate_empty` die Anzahl prüft (`== 15`),
> diese Konstante beim Hinzufügen eines Levels hochzählen.

---

## 5. Häufige Stolperfallen

- **Quelle/Senke ohne Gleis am `dir`-Anschluss** → Validierungsfehler. Quelle
  `dir: W` in `(0,0)` braucht dort ein Gleis mit `W`-Anschluss.
- **`speed >= 500`** → abgelehnt (Tunneling-Schutz). Schnellzüge `240` ist die
  Bestands-Obergrenze.
- **Weichenzelle mit losem Gleis** → eine Weiche belegt ihre Zelle exklusiv.
- **`due` vor `depart`** → Fehler. Soll immer nach Abfahrt.
- **Neues `station.<LABEL>` vergessen** → i18n-Test rot. Dynamische
  Sandbox-Labels `Z<n>` brauchen keinen Key (fallen auf den Rohwert zurück).
- **`id` zum Umsortieren umbenannt** → bricht Saves/Codes. Stattdessen
  `meta.order` ändern.

---

## 6. Versionierung: zwei getrennte Nummern

Nicht verwechseln:

- **`stellwerk_codes::VERSION`** (postcard-Wire-Format der Sharing-Codes).
  Wird **nur** vom eingefrorenen `sim`-Kern (`Level`) gespeist. Solange du
  am `Level`-Struct nichts änderst, bleibt sie unberührt — der Golden-Code
  bleibt dekodierbar. Ein neues Feld in `Level` ist ein **Bruch**: VERSION
  bumpen **und** Migration schreiben.
- **`meta.schema_version`** (On-Disk-Format der Level-Datei). Rein additive
  Felder bleiben dank `#[serde(default)]` kompatibel und brauchen **keinen**
  Bump. Nur bei echten Brüchen (Feld umbenannt/entfernt/Bedeutung geändert)
  hochzählen — dann auch die Bestandsdateien anpassen.

Merksatz: **Metadaten dürfen frei wachsen** (kein Code hängt dran), **der
`sim`-Kern ist eingefroren** (alle Codes hängen dran).

---

## Ausblick: Kampagnen-Level künftig leichter bauen

Der heutige Workflow ist „in Sandbox bauen → Code exportieren → von Hand in
eine `.ron` gießen → Metadaten + i18n + Lösung nachpflegen". Das funktioniert,
hat aber mehrere manuelle Nähte. Vorschläge, grob nach Aufwand/Nutzen:

### Kurzfristig (kleine Werkzeuge)

1. **„Als Kampagnen-Level speichern" aus der Sandbox.** Statt Code →
   Zwischenablage → Datei: ein Editor-Knopf, der direkt
   `assets/levels/<id>.ron` mit gefülltem `meta`-Block schreibt (kleines
   Eingabefeld für `chapter`/`order`/`briefing`). Schreibt zugleich
   Platzhalter-i18n-Keys in beide Tabellen.
2. **Lösung automatisch ablegen.** Wenn ein Run im Editor `Success` liefert,
   einen Knopf „als Designer-Lösung sichern" anbieten → schreibt
   `solutions/<id>.ron` und (optional) achsenbenannte Varianten. Spart den
   Export/Import-Umweg in Schritt 8.
3. **Par-Vorschlag statt Raten.** `par_proof` kennt die je Achse erreichten
   Bestwerte schon. Ein `cargo run --bin par_suggest` (oder ein Flag) könnte
   die `par`-Zeile direkt in die Level-Datei zurückschreiben — „bless"-Flow
   statt manuellem Abtippen.
4. **i18n-Lückenfüller.** Ein kleines Tool, das über `assets/levels/*.ron`
   läuft und fehlende `level.*`/`station.*`-Keys mit dem authored Wert als
   Platzhalter in beide Tabellen einträgt (rot markiert, bis übersetzt).

### Mittelfristig (Pipeline & UX)

5. **Briefing-/Auftrags-Screen.** Heute steht das Briefing klein im Edit-HUD.
   Ein eigener Auftrags-Screen beim Level-Start (mit „Verstanden"-Knopf) macht
   den Betriebsauftrag zur echten Puzzle-Ansage — und ist der natürliche Ort,
   später Lernziele/Tipps unterzubringen.
6. **Kapitel-Freischaltung.** `meta.chapter` liegt jetzt als Datum vor. Damit
   lässt sich „N gelöste Level öffnen das nächste Kapitel" (GDD §8.1) bauen:
   Streckenwahl nach Kapiteln gruppieren, gesperrte Kapitel ausgrauen.
7. **Level-Lint erweitern.** Über die heutigen Checks hinaus: Quelle/Senke je
   plausibel erreichbar, keine doppelten `order` im selben Kapitel, `briefing`
   in Länge plausibel (1–2 Sätze), `optional_hard` nur auf den letzten Leveln
   eines Kapitels.
8. **Content-Pipeline als CLI.** Ein `xtask level new kN`/`level check`
   bündelt Gerüst-Anlegen, i18n-Stub, Lösungs-Slot und Lint in einem Befehl —
   das „ein Level < 1 Tag"-Ziel wird so messbar statt gefühlt.

### Langfristig (Skalierung auf 30+ Level)

9. **Reihenfolge-Refactor-Sicherheit.** Wenn Einschieben zwischen zwei Level
   den 10er-Abstand sprengt, ein Tool zum Neu-Nummerieren der `order` (nie der
   `id`!) innerhalb eines Kapitels.
10. **Schema-Migration vorbereiten.** Sobald `schema_version` erstmals bumpt
    (z. B. Kapitel 5 „Gebirge" bringt Geländedaten), eine
    `migrate_level(version, value)`-Funktion analog zu `parse_progress` — alte
    Dateien lesen, in das neue Format heben, Test mit eingefrorenem
    Alt-Fixture.
11. **Telemetrie-gestützte Balance.** Pars und `optional_hard`-Einstufung
    bewusst erst mit echten Spielzeiten feinjustieren (GDD: Feinbalance in M4).
    Bis dahin sind die CI-bewiesenen Pars die harte Untergrenze.
12. **Solver für Erreichbarkeits-Smoketest.** Ein einfacher Auto-Solver (BFS
    über erlaubte Bauaktionen auf kleinen `buildable`-Flächen) könnte vor jedem
    Commit beweisen, dass ein Level *überhaupt* lösbar ist — noch bevor eine
    Designer-Lösung existiert.

> Reihenfolge-Empfehlung: zuerst **1, 2, 3** (sie entfernen die größten
> manuellen Nähte im täglichen Bauen), dann **6** (Kapitel-Freischaltung nutzt
> die neuen Metadaten sofort sichtbar aus), der Rest nach Bedarf.
