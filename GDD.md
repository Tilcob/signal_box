# Stellwerk — Game Design Document

> **Single Source of Truth.** Bei Widersprüchen zwischen Code, README, Ideen-Doc
> und diesem Dokument gilt dieses Dokument. Änderungen an Design-Entscheidungen
> werden hier eingetragen (siehe Änderungshistorie), bevor sie implementiert
> werden.

| | |
|---|---|
| **Titel** | Stellwerk *(deutsches Wort als internationale Marke; Steam-Untertitel: „a railway signaling puzzle")* |
| **Repo / Arbeitsname** | `signal_box` |
| **Genre** | Open-ended Puzzle („Zachlike") |
| **Plattform** | PC (Steam), Windows zuerst; Linux/macOS nach Determinismus-Prüfung |
| **Preis** | 10–13 € |
| **Engine** | Bevy (Rust) |
| **Sprachen v1** | Englisch (primär), Deutsch |
| **Zeitrahmen** | 5–8 Monate Solo-Entwicklung |
| **Status** | In Design / Prototyp vorhanden (siehe §14) |

---

## 1. Pitch

Das ganze Spiel ist das, was Factorio-Spieler am meisten lieben und fürchten:
Signale, Weichen, Blockabschnitte. **Stellwerk** ist ein Open-ended-Puzzlespiel,
in dem du Gleisanlagen baust und mit Signallogik verdrahtest, sodass alle Züge
eines Fahrplans **ohne einen einzigen Eingriff** kollisionsfrei, deadlockfrei
und pünktlich ihr Ziel erreichen. Bauen → Simulation starten → zusehen, wo es
klemmt → debuggen → optimieren.

## 2. USP

**„Deadlocks debuggen wie Code."** Stellwerk isoliert das Zug-Signal-Problem —
in Factorio/OpenTTD nur Teilsystem — und macht es zum ganzen Spiel:

1. **Die Lösung muss autonom laufen.** Keine Live-Eingriffe; die gebaute
   Anlage *ist* das Programm. Scheitern heißt: Denkfehler in der Logik, nicht
   zu langsam geklickt.
2. **Drei Bewertungsachsen** (Durchsatz, Material, Pünktlichkeit) machen jedes
   Level nach dem ersten Lösen zum Optimierungsproblem — der Zachtronics-Hook.
3. **Stellwerk-Pult-Ästhetik:** Das Spielfeld sieht aus wie ein echtes
   Drucktasten-Stellwerk. Kein anderes Spiel sieht so aus (§10).

**Das GIF:** Ein verknoteter Bahnhof auf schwarzem Pult, 30 Lichtbänder
wandern im perfekten Takt durch rot/grün ausgeleuchtete Fahrstraßen — kein
Stillstand, befriedigend wie ein Uhrwerk.

## 3. Designsäulen

Jede Feature-Idee muss mindestens eine Säule stärken, ohne eine andere zu
verletzen:

1. **Deterministisch & lesbar.** Gleicher Aufbau ⇒ exakt gleicher Ablauf,
   jederzeit. Jeder Zugstopp hat eine sichtbare Ursache (welches Signal, warum
   rot). Kein Zufall in der Simulation.
2. **Der Spieler baut die Lösung, nicht die Reaktion.** Alles, was zur Laufzeit
   Geschick oder Timing verlangt, ist designwidrig.
3. **Tiefe aus wenigen Bausteinen.** Lieber 5 Bausteine mit Kombinatorik als
   20 Spezialteile. Neue Kampagnen-Kapitel führen neue *Situationen* ein, nicht
   zwingend neue Teile.
4. **Scheitern ist Information.** Crash/Deadlock-Reports zeigen präzise Ort,
   Beteiligte und Hergang — Debugging soll Spaß machen, nicht raten.

## 4. Zielgruppe & Markt

- **Kern:** Zachtronics-Käufer (Opus Magnum, SpaceChem, shenzhen I/O — treue
  Nische, ~500k-Pool, kauft jedes gute Zachlike), Factorio/OpenTTD-Spieler mit
  Signal-Faible.
- **Sekundär:** Eisenbahn-Nische (DACH besonders stark — daher Deutsch ab v1),
  Mini-Metro-Publikum als Einstiegs-Zielgruppe der frühen Kapitel.
- **Referenzen:** Zachtronics-Katalog, Mini Metro, Railbound, A-Train-Romantik.
- **Erwartung:** realistisch 50–150k Verkäufe Decke; dafür extrem planbar.
  Dieses Projekt ist bewusst das „Launch-Handwerk lernen"-Spiel.
- **Marketing-Pfad:** Demo + Steam Next Fest, Ziel 7.000+ Wishlists vor Launch.

## 5. Kern-Loop & Phasen

Pro Level wiederholt sich der Zyklus **Bauen → Simulieren → Debuggen →
Optimieren**. Die Phasen sind strikt getrennt:

### Phase 1 — Bauen (Edit Mode)
- Level zeigt: Quellen (Einfahrten) und Ziele (Bahnhöfe/Ausfahrten), den
  Fahrplan (welcher Zug, wann, von wo, wohin), die baubare Fläche, ggf.
  vorgegebene Gleise/Hindernisse.
- Spieler verlegt Gleise und Weichen, setzt Block- und Kettensignale und
  konfiguriert Weichen: Grundstellung plus optionale Regeln (§7.3).
- Unbegrenzt Zeit, unbegrenzt Undo/Redo. Material wird gezählt, nicht begrenzt
  (Budget = Bewertungsachse, kein Hard Limit; Ausnahme: einzelne Level mit
  explizitem Limit als Puzzle-Twist).

### Phase 2 — Simulieren (Run Mode)
- Start per Knopf. Die Anlage ist eingefroren — **keine Bau- oder
  Schalt-Eingriffe**. Geschwindigkeit: Pause / 1× / 4× / 16×, Einzel-Tick-Step.
- Züge fahren den Fahrplan ab. Der Run endet mit **Erfolg** (alle Züge korrekt
  angekommen), **Kollision** oder **Deadlock** (§7.6).

### Phase 3 — Debuggen
- Bei Abbruch: Report mit Ort, beteiligten Zügen und Zeitleiste; bei Deadlock
  wird der Wartezyklus visuell hervorgehoben („Zug 3 wartet auf Block, den
  Zug 5 hält, der auf Zug 3 wartet").
- Ein Klick zurück in den Edit Mode; die Sim-Historie des letzten Runs bleibt
  als Zeitleiste einsehbar.

### Phase 4 — Optimieren
- Nach dem ersten Erfolg zeigt der Ergebnisbildschirm die drei Achsen gegen
  die Par-Werte (§7.7). Level gilt als gelöst; bessere Werte jederzeit möglich.
  Der Loop beginnt freiwillig von vorn — hier lebt die Langzeitmotivation.

## 6. Bausteine (was der Spieler baut)

Bewusst nur **vier** Bausteine (Säule 3):

| Baustein | Funktion | Materialkosten |
|---|---|---|
| **Gleis** | Verbindung auf einem Gitter (gerade, 45°-Diagonale, Kurve) | 1 / Segment |
| **Weiche** | Verzweigung/Zusammenführung zweier Gleise; trägt ihre Routing-Konfiguration: Grundstellung + optionale Regeln je Ziel/Zugtyp (§7.3) | 4 |
| **Blocksignal** | Halt, solange der folgende Blockabschnitt besetzt ist | 2 |
| **Kettensignal** | Halt, solange nicht alle Blöcke bis zum nächsten Blocksignal frei sind (verhindert Stehenbleiben in Kreuzungsbereichen) | 3 |

- Signale sind **gerichtet** (gelten für eine Fahrtrichtung) und teilen das
  Netz in Blockabschnitte — Semantik wie in Factorio, dort millionenfach
  als lernbar und tief bewiesen.
- **Kein** manuell schaltbares Signal, **keine** Live-Eingriffe an Weichen —
  alles Verhalten ergibt sich aus Topologie, Signalplatzierung und der im
  Edit Mode festgelegten Weichenkonfiguration (Säule 2).

## 7. Simulation & Regeln

### 7.1 Zeit
Deterministische Fixed-Tick-Simulation (Render interpoliert). Eine Lösung
spielt auf jedem Rechner exakt gleich ab — Voraussetzung für Sharing (§9),
Bestwerte und Regressionstests.

### 7.2 Züge
- Züge haben Länge (belegen mehrere Segmente), Einfahrtszeit, Start, Ziel.
  Beschleunigung/Bremsweg: v1 vereinfacht (konstante Geschwindigkeit,
  Sofortstopp am Signal) — physikalischere Fahrdynamik nur, falls Playtests
  zeigen, dass sie Puzzles *besser* macht, nicht nur schwerer.
- Zugtypen (später Kapitel): unterschiedliche Länge und Geschwindigkeit
  (Güterzug lang/langsam, S-Bahn kurz/schnell mit Taktanforderung); der Zugtyp
  ist zugleich Kriterium für Weichenregeln (§7.3).
- Züge fahren ausschließlich **vorwärts** — kein Wenden, kein Rückwärtsfahren,
  kein Teilen/Kuppeln in v1 (Kopfbahnhof-Wenden als v1.x-Kandidat, §16).
  Sackgassen enden als Fehlleitung (§7.6).

### 7.3 Routing — „die Weiche ist das Programm"
Es gibt **kein Pathfinding**: Ein Zug folgt an jeder Weiche deren
Konfiguration, die der Spieler im Edit Mode festlegt:

1. **Grundstellung:** die Richtung, die die Weiche standardmäßig stellt.
2. **Weichenregeln (optional):** Ausnahmen je Zug-Eigenschaft — nach Ziel
   („Züge nach Ost → links") oder Zugtyp („Güterzüge → Ausweichgleis").
   Regeln sind eine Eigenschaft der Weiche (§6), kein eigener Baustein.

Damit ist jede Route zu 100 % vorhersagbar (Säule 1): Die Weichen bestimmen,
*wo* Züge fahren, die Signale, *wann*. Überhol- und Ausweichgleise entstehen
aus Regeln + Signalen; „Fehlleitung" (§7.6) ist ein echter, sichtbarer
Konfigurationsfehler. Der Editor warnt vorab per Erreichbarkeits-Check, wenn
ein Fahrplan-Zug sein Ziel mit der aktuellen Konfiguration nicht erreichen
kann. *(Verworfen: kürzester Pfad — macht Überholgleise unmöglich, da nie
kürzeste Route; Factorio-artiges Kosten-Pathfinding — Routen schwer
vorhersagbar, verletzt Säule 1.)*

### 7.4 Blöcke & Reservierung
- Ein Block = Gleisbereich zwischen Signalen. Ein Block gehört höchstens
  einem Zug.
- Blocksignal: grün ⇔ Folgeblock frei. Kettensignal: grün ⇔ alle Blöcke bis
  einschließlich des nächsten Blocksignal-geschützten frei (Durchfahrt ohne
  Halt im Zwischenbereich garantiert).
- Konkurrenz um denselben Block: der Zug, dessen Signal zuerst Anspruch
  erhob, gewinnt; bei Gleichstand entscheidet eine feste, dokumentierte
  Prioritätsregel (z. B. niedrigere Zugnummer). Keine Zufälle (Säule 1).

### 7.5 Fahrplan & Pünktlichkeit
- Jeder Zug hat eine Einfahrtszeit und eine **Soll-Ankunftszeit**. Verspätung
  = Ticks nach Soll; Summe über alle Züge ist die Pünktlichkeits-Achse.
- Frühe Kapitel: großzügige Soll-Zeiten (Pünktlichkeit faktisch = Bonus).
  Späte Kapitel (S-Bahn-Takt): enge Takte als eigentliches Puzzle.
- Ist der Einfahrtsbereich zur Einfahrtszeit belegt, wartet der Zug unsichtbar
  vor der Welt (FIFO-Warteschlange je Quelle) und fährt ein, sobald frei —
  kein Fehlschlag, aber seine Soll-Ankunftszeit läuft weiter. Stau vor der
  Einfahrt ist damit eine legitime (teure) Strategie.

### 7.6 Fehlschläge
- **Kollision:** zwei Züge im selben Segment → Run bricht sofort ab,
  Crash-Marker + Report. (Tritt nur auf, wo der Spieler Blöcke gar nicht
  gesichert hat — korrekt signalisierte Anlagen kollidieren nie.)
- **Deadlock:** Wartezyklus erkannt oder globaler Stillstand → Abbruch mit
  Zyklus-Visualisierung. Das ist der *interessante* Fehlschlag, um den das
  Spiel gebaut ist.
- **Fehlleitung:** Zug erreicht ein falsches Ziel oder eine Sackgasse — Folge
  der Weichenkonfiguration (§7.3) → Abbruch mit Anzeige des gefahrenen Wegs
  und der Weiche, an der die Route vom Ziel wegführte.

### 7.7 Bewertung
Drei Achsen, **lokal** gegen Par-Werte des Designers (keine Server in v1;
Community-Histogramme als Post-Launch-Option, §16):

| Achse | Messung | Optimierungsrichtung |
|---|---|---|
| **Durchsatz** | Tick der letzten Ankunft | kleiner = besser |
| **Material** | Summe Baukosten (§6) | kleiner = besser |
| **Pünktlichkeit** | Summe Verspätungs-Ticks | kleiner = besser |

- Pro Achse eine Medaille bei Par-Erreichen; „Goldene Anlage" = alle drei in
  einer einzigen Lösung (bewusst oft Zielkonflikt — eine Lösung pro Achse ist
  der Normalfall, alle drei auf einmal die Königsdisziplin).
- Bestwerte pro Level werden mit der zugehörigen Lösung gespeichert (mehrere
  Lösungs-Slots pro Level wie bei Zachtronics).

## 8. Struktur & Modi

### 8.1 Kampagne (Herzstück, ~40–60 Level)
Kapitel führen je eine Situation/Mechanik ein; innerhalb eines Kapitels sind
die letzten 2–3 Level optional-schwer (Blocker vermeiden):

| # | Kapitel (Arbeitstitel) | Neu eingeführt |
|---|---|---|
| 1 | **Blockstrecke** | Gleise, Blocksignale, Zugfolge auf einer Strecke |
| 2 | **Ausweiche** | Weichen (Grundstellung + Zielregeln), Ausweich-/Überholgleise, Gegenverkehr auf eingleisiger Strecke |
| 3 | **Der Knoten** | Kettensignale, Kreuzungen, erste echte Deadlock-Gefahr |
| 4 | **Sortierwerk** | Reihenfolge-Puzzles: Züge müssen in bestimmter Folge ankommen; Zuglängen, Zugtyp-Weichenregeln *(kein Rangieren — Züge fahren nur vorwärts, §7.2)* |
| 5 | **Gebirge** | Lange eingleisige Abschnitte, knappes Material, Geländebeschränkung |
| 6 | **S-Bahn-Takt** | Enge Fahrpläne, Zugtypen gemischt, Pünktlichkeit als Hauptachse |

- Freischaltung: N gelöste Level öffnen das nächste Kapitel (kein 100 %-Zwang).
- Jedes Level hat eine Kurzbeschreibung im Stil eines Betriebsauftrags
  (1–2 Sätze, wenig Text — hält Lokalisierung billig).

### 8.2 Sandbox
Leere (wählbar große) Fläche, frei definierbare Quellen/Ziele/Fahrpläne, alle
Bausteine frei. Kein Ziel, keine Bewertung — Spielwiese und Quelle für
Community-Inhalte. Billig, weil Editor und Sim identisch mit der Kampagne sind.

### 8.3 Level- & Lösungs-Sharing per Code
- Export/Import als kompakter Text-Code (Base64-kodiert, versioniert):
  **Lösungs-Codes** (Anlage zu einem Kampagnen-Level) und **Level-Codes**
  (Sandbox-Setups inkl. Fahrplan als spielbares Custom-Puzzle).
- Kein Server, kein Workshop in v1 — Codes laufen über Discord/Reddit/Foren.
  Deterministik (§7.1) garantiert identisches Abspielverhalten.

### 8.4 Nicht in v1 (explizite Nicht-Ziele)
- Keine Wirtschaftssimulation, keine Passagiere mit Meinungen, kein
  Streckennetz-Management — nur Züge, Signale, Takt.
- Kein Multiplayer, keine Server-Features (Histogramme, Leaderboards).
- Keine Tages-Challenge (braucht Puzzle-Generator → v1.x-Kandidat).
- Kein Level-Editor-UI über die Sandbox hinaus (kein Skripting o. Ä.).

## 9. UI / UX

- **Pult-Metapher konsequent:** Edit Mode = Arbeiten am Pult (Bausteine als
  „Tasten/Module"), Run Mode = das Pult leuchtet und lebt. Moduswechsel ist
  ein einziger großer, unübersehbarer Schalter.
- Bau-UX: Klick-Drag-Gleisziehen auf Gitter, R = rotieren, Baustein-Hotkeys
  1–4, Flächen-Abriss, vollständiges Undo/Redo (auch über Runs hinweg).
- Sim-UX: Geschwindigkeitsleiste (Pause/1×/4×/16×), Einzel-Tick-Knopf,
  klickbarer Zug zeigt Route + nächsten Haltegrund („wartet auf Signal B3:
  Block belegt durch Zug 7").
- Lesbarkeit vor Realismus: Blockgrenzen, Reservierungen und Signalzustände
  sind im Run Mode permanent sichtbar (Ausleuchtung), nicht erst auf Hover.
- Kamera: stufenloses Pan (Maus-Drag/WASD) und Zoom (Mausrad); weit
  herausgezoomt vereinfachen Symbole zu Lichtpunkten — Lesbarkeit vor Detail.
- Input-Architektur: Hotkeys, Kamera und Editor-Shortcuts als Action-Maps über
  `bevy_enhanced_input` (§12.2), rebindbar (einfaches Listen-UI genügt in v1);
  Klicks auf UI-Elemente laufen über bevy_ui-Picking.
- **Barrierefreiheit:** Farbe ist nie alleiniger Informationsträger — Block-
  und Signalzustände unterscheiden sich zusätzlich über Form/Muster (z. B.
  besetzt = gefüllt wandernd, reserviert = schraffiert). Pflicht, weil der
  Pult-Look (§10) rot/grün-lastig ist; ab M1 mit Farbenblind-Simulation testen.
- Vollständig maus-spielbar; Hotkeys als Beschleuniger. Controller: nein (v1).

## 10. Art Direction

**Drucktasten-Stellwerk-Ästhetik** (SpDrS60/Domino-Pult):

- Nahezu schwarzes Pult als Spielfläche; Gleise als schmale Leuchtbänder
  (frei: dunkel umrandet, reserviert: gelb, besetzt: rot wandernd, Fahrstraße:
  grün), Züge als wandernde Lichtsegmente mit Zugnummern-Label.
- Signale/Weichen als Pult-Symbole mit Leuchtmeldern; UI-Typografie im Stil
  technischer Beschriftungsschilder (DIN-artig).
- Wenig Farben, hoher Kontrast, viel Glow — der Look entsteht aus Shadern und
  Vektorformen, **null gemalte Assets**. Capsule-tauglich, weil sofort als
  „echtes Stellwerk" lesbar und von jedem anderen Puzzle-Spiel unterscheidbar.
- Stretch (nur wenn Zeit): kleines „Außenwelt"-Fenster, das den Pult-Zustand
  als stilisierte Miniatur-Bahnlandschaft spiegelt.

## 11. Audio

- Soundkulisse statt Musik im Run Mode: Relaisklacken beim Umlegen, leises
  Brummen des Pults, Zuglauf als rhythmisches Ticken, sattes „Klack" bei
  Fahrstraßenbildung. ASMR-Qualität ist hier Teil des Produkts (vgl. Opus
  Magnum) — kleines externes Budget (~1–2 k€) einplanen.
- Edit Mode: ruhige, minimale Ambient-Musik (2–3 Tracks).
- Erfolgs-/Crash-/Deadlock-Stinger klar unterscheidbar.

## 12. Tech

### 12.1 Architektur & Workspace

Cargo-Workspace mit harter Trennung zwischen deterministischem Kern und
Bevy-Frontend:

```
signal_box/
├── crates/
│   ├── stellwerk_sim/    # Sim-Kern: KEINE Bevy-Dependency, kein f32 im
│   │                     # Zustand, kein Zufall. Headless kompilier- und
│   │                     # testbar. Enthält: Gleisgraph, Blöcke, Signale,
│   │                     # Routing, Tick-Loop, Deadlock-/Kollisionserkennung,
│   │                     # Level-/Anlagen-Datenmodell, Bewertung.
│   └── stellwerk_codes/  # Sharing-Codes: (De-)Serialisierung + Versionierung
│                         # der Level-/Lösungs-Codes. Hängt nur an
│                         # stellwerk_sim-Datentypen.
└── src/                  # Bevy-App: Rendering, UI, Audio, Editor-UX, Steam.
                          # Spricht mit dem Kern ausschließlich über dessen
                          # öffentliche API (Tick rein, Zustands-Snapshot raus).
```

- **Sim-Arithmetik: handgerollte Integer** (entschieden). Eigene Einheiten als
  Newtypes: Position in mm (`i64`) auf dem Gitter, Zeit in Ticks (`u64`),
  Geschwindigkeit in mm/Tick. Keine Mathe-Dependency im Kern; bit-identisches
  Verhalten auf allen Plattformen ist damit trivial statt erkämpft.
- Das Frontend interpoliert Integer-Snapshots für die Darstellung in f32 —
  Floats existieren nur jenseits der Sim-Grenze.
- Replay = Level-Definition + Anlage (kein Input-Log nötig, da die Sim ohne
  Laufzeit-Eingaben deterministisch abläuft).
- Headless-Sim ermöglicht: jede Kampagnen-Par-Lösung läuft als Regressionstest
  in CI; Balancing-Sweeps über Nacht.

### 12.2 Crates (verbindlich)

| Bereich | Crate | Begründung / Grenzen |
|---|---|---|
| Engine | `bevy` | Gesetzt. Minor-Version wird pro Meilenstein gepinnt; Upgrades nur *zwischen* Meilensteinen, nie währenddessen. |
| Vektor-Tessellation | `lyon` (direkt, **nicht** das Bevy-Plugin) | Entschieden: Pfade/Kurven/Striche → Vertices für Gleisbänder & Pult-Symbole; gerendert wird Bevy-nativ (eigene 2D-Meshes + Material). Engine-unabhängige Dependency ⇒ kein Bevy-Versions-Lock im Renderpfad. Glow über Bevys eingebautes HDR-Bloom. |
| Spiel-UI | `bevy_ui` (eingebaut) | Entschieden: Editor-Panels, Ergebnisbildschirm, Menüs nativ — volle Kontrolle übers Pult-Styling. Die UI ist bei diesem Spiel das Spiel. |
| Input | `bevy_enhanced_input` | Entschieden: Action-Maps für Hotkeys, Kamera, Editor-Shortcuts (§9), rebindbar. Bevy-gekoppelt → Ausnahmeliste §12.4 Pkt. 2; Fallback bei Pflege-Ausfall wäre Bevys eingebautes Input-API. UI-Klicks bleiben bei bevy_ui-Picking. |
| Dev-Tools | `bevy_egui` + `bevy-inspector-egui` | Nur hinter dem `dev`-Feature (wie im Prototyp). Niemals Spieler-UI. |
| Tunables-Hot-Reload | `bevy_common_assets` (RON-Loader) | Aus dem Prototyp übernommen: Balancing-Konstanten zur Laufzeit nachladen. Bevy-gekoppelt, aber klein und leicht ersetzbar — fällt es bei einem Upgrade aus, blockiert nichts (§12.4 Pkt. 2). |
| Audio | `bevy_kira_audio` | Entschieden: viele gleichzeitige Klack-Instanzen, Fades, Loops — der ASMR-Anspruch (§11) braucht die Kontrolle. |
| Serialisierung | `serde` + `ron` | Level-Definitionen, Savegames, Lokalisierungstabellen: menschenlesbar, diffbar, Git-freundlich. |
| Sharing-Codes | `postcard` + `base64` | Kompaktes Binärformat → Text-Code. Erst falls Codes > ~1500 Zeichen werden: `miniz_oxide` dazwischenschalten. Format trägt Versionsbyte ab Tag 1. |
| Zwischenablage | `arboard` | M2-Restfeature 01: Sharing-Codes per Copy/Paste statt über Dateien. Plattformübergreifend (X11/Wayland/Windows/macOS), engine-unabhängig (kein Bevy-Lock); Text-only über die Default-Features (kein `image-data`). Fällt der Clipboard-Zugriff aus, greift der Datei-Fallback `stellwerk_code.txt`/`stellwerk_import.txt` — blockiert also nichts. |
| Save-Pfade | `directories` | Ab M2 (Save v2): Fortschritt/Sandbox ins plattformübliche Konfigverzeichnis statt ins Arbeitsverzeichnis. Engine-unabhängig, winzig. |
| Steam | `steamworks` (steamworks-rs) | Achievements + Cloud Saves, mehr nicht. Kommt erst in M4; bis dahin hinter einem `steam`-Feature, Builds laufen immer auch ohne. |

### 12.3 Bewusst ohne Crate (handgerollt)

| Problem | Lösung | Warum keine Dependency |
|---|---|---|
| Erreichbarkeits-Check (§7.3) | Simple Graph-Traversierung (BFS) | Die Sim selbst hat kein Pathfinding (Weichen-Routing); BFS dient nur der Editor-Warnung „Ziel unerreichbar". Graphen sind winzig — kein `petgraph`. |
| Undo/Redo (§9) | Command-Stack über Editier-Operationen | Trivial, wenn jede Bau-Aktion eh als Operation modelliert ist (braucht auch das Sharing-Format). |
| Zufall | — | Es gibt keinen Zufall in der Sim (Säule 1). Frontend-Juice (Partikel o. Ä.) darf `rand` nutzen, niemals der Kern. |
| Lokalisierung (EN/DE) | RON-Stringtabelle pro Sprache + Key-Lookup | Wenig Text (§8.1), keine Plural-/Genus-Logik nötig. `fluent` erst, falls FIGS (§16) kommt. |
| Tweening/UI-Animation | Kleine eigene Lerp-Helfer | `bevy_tweening` ist Bevy-versionsgekoppelt; unser Bedarf (Blenden, Pulsieren) ist minimal. Neu bewerten in M3, falls Juice-Bedarf wächst. |

### 12.4 Dependency-Politik

1. Jede neue Dependency wird hier in §12.2 eingetragen, *bevor* sie in
   `Cargo.toml` landet — mit Begründung.
2. Bevy-gekoppelte Plugins (Upgrade-Risiko!) nur, wenn sie hinter einem
   Feature-Flag liegen oder ihr Ausfall das Shipping nicht blockiert.
   Aktuell genau drei: egui-Stack (dev-only), `bevy_kira_audio` und
   `bevy_enhanced_input` — die beiden letzten als akzeptiertes,
   shipping-relevantes Risiko (etablierte Pflege; ihr Ausfall verzögert ein
   Bevy-Upgrade, blockiert aber kein Shipping).
3. `stellwerk_sim` und `stellwerk_codes` halten ihre Dependencies minimal
   (`serde` ja; sonst Einzelfallprüfung) — der Kern muss in CI in Sekunden
   bauen.

### 12.5 Tooling & CI

- GitHub Actions: `cargo test --workspace` (inkl. Headless-Par-Lösungen als
  Regressionstests), `clippy -D warnings`, `fmt --check`; Windows + Linux,
  damit Determinismus-Abweichungen sofort auffallen (bit-identische
  Replay-Hashes als Test).
- Save-/Code-Format versioniert ab Tag 1 (Level-Codes überleben Updates).

## 13. Meilensteine (Zielrahmen 5–8 Monate)

> Detaillierte Implementierungspläne pro Meilenstein liegen in `plans/`:
> [M0](plans/M0/M0-sim-kern.md) · [M1](plans/M1/M1-vertical-slice.md) ·
> [M2](plans/M2/M2-content-maschine.md) · [M3](plans/M3/M3-demo.md) ·
> [M4](plans/M4/M4-launch.md). Sie konkretisieren dieses GDD, ändern es aber
> nie — Design-Änderungen laufen immer zuerst hier durch (§18). M1–M4 sind
> Rolling-Wave-Pläne: Sie werden beim jeweiligen Meilenstein-Start gegen den
> realen Stand geschärft; die zugehörigen Wochen-Angaben entstehen erst dann.

| Meilenstein | Inhalt | Exit-Kriterium |
|---|---|---|
| **M0 – Sim-Kern** (4 Wo) | Deterministischer Tick-Kern: Gleisgraph, Blöcke, beide Signaltypen, Weichen-Routing (Grundstellung + Regeln), Deadlock-/Kollisionserkennung; Headless-Tests | 20 Sim-Szenarien als Tests grün; Replays bit-identisch |
| **M1 – Vertical Slice** (6 Wo) | Editor (Bauen/Undo), Run Mode mit Speed-Controls, Debugging-Report, Bewertung, 8 Level aus Kapitel 1–3, Pult-Look v1 | Fremde Testspieler lösen Kapitel 1 ohne Hilfe; „noch ein Versuch"-Sog spürbar — sonst Design nachschärfen, bevor Content entsteht |
| **M2 – Content-Maschine** (6 Wo) | Level-Pipeline, Kapitel 1–4 komplett, Sandbox, Sharing-Codes, Lokalisierungs-Setup EN/DE | 30+ Level spielbar; Level bauen kostet < 1 Tag/Stück |
| **M3 – Demo** (4 Wo) | Kapitel 5–6, Polish-Pass (Audio, Juice, Onboarding), Demo-Build (Kapitel 1–3 — der Deadlock-USP muss in der Demo erlebbar sein), Steam-Page | Demo veröffentlicht; Next-Fest-Anmeldung |
| **M4 – Launch** (4–8 Wo) | Playtest-Feedback, Balancing (Par-Werte!), Achievements, Trailer/GIFs, Bugfixing | 1.0 auf Steam |

**Erst-Validierung (vorgezogen, Teil von M0/M1):** Papier-/Greybox-Check der
Kernfrage „macht Bauen→Scheitern→Debuggen süchtig?" mit hässlichen Test-Leveln —
bevor eine Stunde in den Pult-Look fließt.

## 14. Stand: Prototyp vs. dieses GDD

Der vorhandene Prototyp (siehe `README.md`) war der Tech-/Gefühls-Spike und
ist ein **Live-Dispatcher** — er weicht damit absichtlich vom Zieldesign ab:

| Prototyp heute | GDD-Ziel |
|---|---|
| Signale/Weichen live klicken | Strikte Phasentrennung, Anlage läuft autonom |
| Festes Gleisnetz, nur Togglen | Spieler verlegt Gleise selbst |
| f32-/Frame-basierte Bewegung | Integer-Fixed-Tick-Sim, headless testbar |
| Signale manuell rot/grün | Block-/Kettensignale mit automatischer Blocklogik |
| Crash bei Berührung (Radius) | Blockbasierte Belegung; Kollision nur bei ungesicherten Blöcken |

Der Prototyp bleibt als Referenz für Bewegungsgefühl/Tempo; M0 ersetzt seinen
Sim-Kern vollständig.

## 15. Risiken

| Risiko | Gegenmaßnahme |
|---|---|
| Nische klein (~50–150k Decke) | Bewusst akzeptiert: planbares Erstprojekt; Kostenrahmen klein halten |
| Deadlock-Puzzles frustrieren statt fesseln | Säule 4 ernst nehmen: Debug-Werkzeuge sind Kern-Feature, nicht Beiwerk; M1-Exit-Kriterium |
| Onboarding: Block-/Kettensignal-Semantik ist erklärungsbedürftig | Kapitel 1–3 = sanfte Lernkurve mit Show-don't-tell-Leveln; Factorio beweist Lernbarkeit |
| Determinismus-Bugs (Plattformen, Float) | Integer-Sim ab M0; bit-identische Replay-Tests in CI |
| Par-Werte schlecht balanciert | Headless-Sweeps + Demo-Telemetrie (lokal, opt-in) vor Launch |
| Solo-Scope-Creep | §8.4 Nicht-Ziele; neue Ideen landen in §16 statt im Code |

## 16. Offene Fragen / Später entscheiden

- **Community-Histogramme** (Zachtronics-Style) post-launch: Steam-Leaderboards
  reichen evtl. — prüfen, sobald v1 steht.
- **Tages-Challenge** (v1.x): braucht validierenden Puzzle-Generator.
- **Bedingte Weichenregeln** (belegungsabhängig: „wenn Ausweichgleis besetzt →
  Hauptgleis"): v1.x-Kandidat, falls späte Kapitel mehr Ausdruckskraft
  brauchen. Vorsicht: macht Routen zustandsabhängig — reibt sich mit Säule 1.
- **Kopfbahnhof-Wenden** (Richtungswechsel an Endgleisen): v1.x-Kandidat —
  thematisch stark, kostet aber Sim-Komplexität (entschieden: v1 nur
  vorwärts, §7.2).
- **Fahrdynamik** (Beschleunigung/Bremswege): nach M1-Playtests bewerten.
- **Außenwelt-Fenster** (§10 Stretch): Nice-to-have, frühestens M3.
- Steam-Untertitel final (EN) + Capsule-Konzept: vor Steam-Page (M3).

## 17. Glossar

| Begriff | Bedeutung |
|---|---|
| **Block(abschnitt)** | Gleisbereich zwischen Signalen; gehört max. einem Zug |
| **Blocksignal** | Hält Züge, bis der Folgeblock frei ist |
| **Kettensignal** | Hält Züge, bis alle Blöcke bis einschließlich des nächsten blocksignal-geschützten frei sind (§7.4) |
| **Fahrstraße** | Reservierte Kette von Blöcken für einen Zug |
| **Grundstellung** | Standardrichtung einer Weiche; im Edit Mode festgelegt |
| **Weichenregel** | Ausnahme von der Grundstellung je Ziel/Zugtyp (§7.3) |
| **Deadlock** | Zyklus von Zügen, die gegenseitig auf ihre Blöcke warten |
| **Par** | Designer-Referenzwert je Bewertungsachse; Medaille bei Erreichen |
| **Run** | Ein Simulationsdurchlauf von Start bis Erfolg/Abbruch |
| **Tick** | Kleinste Sim-Zeiteinheit; alle Regeln greifen tick-synchron |
| **Zachlike** | Open-ended-Puzzlegenre nach Zachtronics: bauen, laufen lassen, optimieren, Metriken vergleichen |

## 18. Änderungshistorie

| Datum | Änderung |
|---|---|
| 2026-06-11 | Erstfassung. Grundsatzentscheidungen: echtes Zachlike (strikte Phasentrennung), Spieler verlegt Gleise, Bewertung 3 Achsen lokal, v1 = Kampagne + Sandbox + Code-Sharing, Pult-Ästhetik, Titel „Stellwerk", Sprachen EN+DE. |
| 2026-06-11 | §12 zum Tech-Stack-Kapitel ausgebaut. Entschieden: Workspace mit Bevy-freiem Sim-Kern (`stellwerk_sim`), handgerollte Integer-Arithmetik, `lyon` als Tessellator (Bevy-natives Rendering + Bloom), Spiel-UI in `bevy_ui` (egui nur dev), `bevy_kira_audio`; serde+ron (Levels/Saves), postcard+base64 (Sharing-Codes), handgerollt: Pathfinding, Undo, Lokalisierung, Tweening. Dependency-Politik §12.4. |
| 2026-06-11 | Review-Runde: Routing-Widerspruch aufgelöst — §7.3 ersetzt durch **Weichen-Routing** (Grundstellung + Regeln je Ziel/Zugtyp, kein Pathfinding; „die Weiche ist das Programm"). Züge fahren nur vorwärts (kein Wenden in v1, §7.2); Kapitel 4 „Rangierbahnhof" → „Sortierwerk"; Demo = Kapitel 1–3 (USP muss erlebbar sein); Einfahrts-FIFO bei belegter Quelle definiert (§7.5); §9 ergänzt um Kamera, Input-Architektur und Barrierefreiheit (Farbe nie alleiniger Träger); `bevy_enhanced_input` als Input-Crate aufgenommen (§12.2, Ausnahmeliste §12.4); Kettensignal-Definition Glossar/§7.4 harmonisiert. |
| 2026-06-11 | M0 fertig implementiert (20 Szenarien grün, Goldwert-Hashes committed). Sim-Präzisierungen aus der Implementierung (Details: plans/M0/M0-sim-kern.md §9): strikte Blockregel inkl. Selbstbelegung (Selbstblockade auf Ringen endet als Stillstand, nicht Deadlock); Ankunftstick = Kopf erreicht Anker im selben Tick; Erreichen eines fremden Sink-Ankers zählt als Fehlleitung; Einfahrts-FIFO prüft nur die physische Einfahrkante (Auffahrunfälle im Quellblock bleiben möglich — Spielrisiko). |
| 2026-06-12 | M1-Slice implementiert (Editor, Run-Mode, Reports, Bewertung, Pult-Look v1, 8 Level, Fortschritts-Save). Code-DoD erfüllt; **Playtest-Exit-Kriterium offen**. Abweichungen (plans/M1/M1-vertical-slice.md §8): `bevy_enhanced_input` und `lyon` aus §12.2 noch nicht eingebunden (Bevy-Input genügt vorerst; Stub-Geometrie ist geradlinig — Quads + Bloom statt Tessellation), Fortschritt vorerst als lokale Datei statt `directories` (Save v2 in M2), i18n-Shim mit DE-Strings (EN/DE-Tabellen M2). |
| 2026-06-12 | M2-Systeme implementiert: `stellwerk_codes` (SW1-Codes, Goldcode eingefroren), **Par-Beweis-Harness** (jedes Level trägt CI-geprüfte Designer-Lösungen — §7.7-Versprechen technisch erzwungen), Sandbox mit Quelle/Ziel-Werkzeugen + Fahrplan-Editor (§8.2), Code-Import/-Export über Dateien (Zwischenablage vertagt), EN/DE-RON-Tabellen mit Sprachumschalter, Save v2 (`directories`, 3 Lösungs-Slots, M1-Migration). Content-Stand 15/30+ Level (alle par-bewiesen) — Rest über die Pipeline, Tempo-Log in plans/M2/content-log.md. Levelbestand-Lücken und Abweichungen: plans/M2/M2-content-maschine.md §7/§8. |
| 2026-06-15 | M2-Restfeature 01 (plans/M2/restfeatures/01): Sharing-Codes laufen über die Systemzwischenablage (`arboard`, §12.2) statt über Dateien; `stellwerk_code.txt`/`stellwerk_import.txt` bleiben verlustfreier Fallback bei fehlendem Clipboard (Headless/CI/Linux ohne Clipboard-Manager). Schließt M2-§8-Abweichung 1. |
| 2026-06-15 | M2-Restfeature 02 (plans/M2/restfeatures/02): Sandbox-Level-Edits (Quellen/Senken/Fahrplan) teilen jetzt eine Undo-Zeitachse mit den Layout-Bauaktionen — `EditOp` um Station-/Schedule-Varianten erweitert, `apply`/`invert` über ein modulinternes `EditTarget` (Layout + Level); Sink-Löschung kaskadiert abhängige Fahrplanzeilen als `Group`. Schließt M2-§8-Abweichung 2. |
| 2026-06-15 | M2-Restfeature 03 (plans/M2/restfeatures/03): Fahrplan-Editor bekommt echte Eingabefelder — wiederverwendbares `numeric_field`-Widget (fokussierbar, `ButtonInput<Key>` für QWERTZ, Enter/Blur committet geclampt), depart/due/speed/length tippbar, jeder Commit ein `ScheduleEdit`. Tool-Hotkeys/Run-Start werden via `FocusedField`/`no_field_focused` unterdrückt, solange ein Feld fokussiert ist. Schließt M2-§8-Abweichung 3. |
