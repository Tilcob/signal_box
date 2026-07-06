# Zug-Optik: Klassen, Loks & Waggons

Status: Design abgestimmt, noch nicht gebaut. Rein **Frontend + Content** —
**kein** Eingriff in den Sim-Kern, die Sharing-Codes oder den Replay-Hash.

## Ziel

Züge sehen je **Zugklasse** unterschiedlich aus (Farbe **und** Form — Farbe nie
alleiniger Träger, GDD-Accessibility), und der Körper liest sich als **Lok +
Waggons** statt als durchgehender Streifen. Güterwaggons zeigen **Ladung**. Im
Sandbox wählt man statt einer LE-Länge die **Wagenzahl**.

## Modell & Maße

- **Zug = 1 Lok + N Anhänger.** Der Sandbox-Wert = **Anzahl Anhänger** (3 = Lok + 3).
- **Lok** 200 LE, **Wagen** 200 LE, **Kupplung** 20 LE **zwischen allen Einheiten**
  (auch Lok↔Wagen 1). Layout ab Kopf:
  `Lok(200) · Kupplung(20) · Wagen(200) · Kupplung(20) · … · Wagen(200)`.
- **Länge (LE)** = `200 + N·220` → 1 Wagen = 420 · 2 = 640 · 3 = 860 …
  Das ist der Wert, der weiterhin in `ScheduleEntry.length` steht; der Sim sieht
  nur diese Zahl, weiß nichts von „Wagen".

## Klassen-Tabelle

| Klasse | Farbe | Lok-Form |
|---|---|---|
| 0 Nahverkehr | Blau | Quadrat |
| 1 Güter | Grün | Raute |
| 2 Express | Orange | Chevron/Pfeil (in Fahrtrichtung) |
| sonst | Golden-Angle-Hue (wie `col_block`) | Quadrat (Fallback) |

- **Lok** = Klassenfarbe, **etwas heller & einen Tick größer** als die Waggons.
- **Waggons** = Klassenfarbe (etwas dunkler als die Lok).
- Klasse 0 und 2 haben **gleiche Waggons** (nur Lok-Farbe/Form unterscheidet).
- **Güter-Waggons**: Gondel-Look — grüner Wagenkasten mit einem **eingesetzten
  Ladungsblock** in **klarem Dunkelgrau** (Schüttgut-Optik, hebt sich vom Kasten
  ab). Ladung **nur auf Anhängern**, nicht auf der Lok.
- **Dwell-Timer-Scheibe** bleibt unverändert (liegt weiter auf dem Kopf).

---

## Umbau

### 1. Palette (`src/board/palette.rs`)
Neue Farben:
- `col_class_wagon(class) -> Color` — 0 Blau, 1 Grün, 2 Orange, sonst
  Golden-Angle-Hue.
- `col_class_loco(class) -> Color` — hellere Variante derselben Farbe.
- `col_coupling() -> Color` — dunkles Grau (dunkler als jeder Wagen).
- `col_cargo() -> Color` — klares Dunkelgrau (Ladung).

Die alten `col_freight`/`col_freight_head`/`col_train`/`col_head` für den Zug
werden durch die Klassen-Funktionen ersetzt (bleiben ggf. für anderes bestehen).

### 2. Körper- & Kopf-Rendering (`src/board/run_board.rs`, `redraw_trains`)
Kern ist ein Helper, der **ein Körper-Teilstück nach Distanz vom Kopf** zeichnet:
- `occupied_into` liefert die belegten Segmente `(edge, lo, hi)` **kopf-first**;
  am Kopf-Segment ist `hi` das kopfnahe Ende.
- Der Helper läuft diese Segmente ab, **akkumuliert die Distanz vom Kopf**, und
  zeichnet für den Teil jeder Kante, der in `[d0, d1]` fällt, ein `band` (per
  `point_world` + `lerp` wie heute). Ein Teilstück darf über Kanten-/Kurven-
  grenzen laufen (mehrere Bänder).

Damit wird der Körper zerlegt:
- **Lok**: `[0, 200]` in `col_class_loco` (heller); dazu am vordersten Punkt der
  **Klassen-Form-Marker**:
  - 0 → `lamp(diamond=false)` (Quadrat), 1 → `lamp(diamond=true)` (Raute),
    2 → `chevron` in Fahrtrichtung.
  - Fahrtrichtung = Kopf-Kanten-Richtung (`graph.edge(head).from→to` in Welt-
    koordinaten), am interpolierten Kopf.
- **Pro Anhänger i (1..N):** Kupplung `[…, …+20]` in `col_coupling`, dann Wagen
  `[…+20, …+220]` in `col_class_wagon`.
- **Güter-Ladung:** auf jedem Wagen zusätzlich ein schmaleres/kürzeres
  Cargo-Band (höheres z, `col_cargo`) mittig über dem Wagen → grüner Rahmen +
  graue Ladung = offener beladener Wagen.

Der alte „ein Band pro belegter Kante"-Loop entfällt; der Kopf-`lamp` in
Zug-/Güter-Farbe entfällt (ersetzt durch Lok + Form-Marker).

### 3. Sandbox: Wagenzahl statt LE-Länge (`src/ui/schedule_panel.rs`)
- Das `SchedFieldKind::Length`-Feld wird als **Wagen** dargestellt/editiert
  (Label z. B. „Wg"), Wertebereich **1..N_MAX** (z. B. 20).
- Konvertierung: Anzeige `n = round((length - 200) / 220).max(1)`;
  Commit `length = 200 + 220·n`.
- `LEN_MIN`/`LEN_MAX` werden zu Wagen-Min/Max. `ScheduleEntry.length` bleibt LE —
  **keine Schema-/Code-Änderung**.
- Read-only Kampagnen-Fahrplan zeigt ebenfalls die Wagenzahl.

### 4. Content: Kampagnen-Level anpassen (`assets/levels/…`)
- Jede Zug-`length` auf die **nächste glatte Wagenzahl** snappen
  (`n = round((length-200)/220).max(1)`, dann `length = 200+220·n`).
- Betrifft **alle Level mit Fahrplan** (k1_01…k1_10, k4_01).
- **Achtung — nicht nur Optik:** eine geänderte Länge ändert die
  Kollisionslänge und damit Timing/Ankunft. Deshalb pro berührtem Level
  **due/par neu blessen** (`due_suggest --write` → `par_suggest --write`) und den
  `par_proof`-CI-Test grün halten.

## Nicht betroffen (bewusst)
- **Sim-Kern**, `stellwerk_sim`-Typen: keine neuen Felder, Wagen sind reine
  Render-/UI-Konvention über `length`.
- **Sharing-Codes / `VERSION`**: unverändert (kein neues Serialisierungsfeld).
- **Replay-Hash**: unverändert (nichts am Sim-State).

## Reihenfolge
1. Palette-Farben.
2. Kopf/Lok + Waggons + Kupplungen im Renderer (Körper-Teilstück-Helper).
3. Güter-Ladung.
4. Sandbox-Wagen-Feld.
5. Kampagnen-Level anpassen + neu blessen.
6. Build + `clippy -D warnings` + Tests grün.

---

## Prüfer-Durchlauf (am Code gegengeprüft)

| Behauptung | Verdikt |
|---|---|
| `chevron()` für den Express-Kopf wiederverwendbar | ❌ **Korrektur:** `chevron` ist in `draw.rs` **privat** (`fn chevron`, kein `pub(super)`). Muss auf `pub(super)` gehoben werden (oder Express-Kopf inline aus 2 `band`). |
| `occupied_into` liefert (edge, lo, hi) kopf-first, nahes Ende = `hi` | ✅ bestätigt — der Distanz-Helper kann darauf aufbauen. |
| Kopf-Farbe/„Fracht" hängt heute an `stop.is_some()`, nicht an der Klasse | ✅ — Umstellung auf `train.class` nötig; Dwell-Scheibe bleibt an `stop`. |
| Sandbox-Feld: nur UI, `length` bleibt gespeichert, kein Format-Eingriff | ✅ — reine Konvertierung im Panel. |
| „Bestehende Level anpassen ist nur Optik" | ❌ **Korrektur:** Längenänderung ist **nicht** rein optisch — sie ändert Kollisionslänge + Timing ⇒ **par/due für jedes Level neu blessen**, `par_proof` grün halten. Größter Posten des Umbaus. |
| Kein Sim-/Codes-/Hash-Eingriff | ✅ bestätigt — reine Frontend-/Content-Änderung. |
| Wagen-Zeichnung quert Kanten/Kurven sauber | ⚠️ **Präzisierung:** der Teilstück-Helper muss pro Kante clippen und über Grenzen mehrere Bänder zeichnen — machbar, aber der aufwändigste Renderer-Teil. |
| Ladungs-Optik „Gondel" | ⚠️ **Präzisierung:** kein echtes Trapez/Haufen mit `band` — realisiert als **eingesetztes schmaleres Cargo-Band** über dem Wagen (grüner Rahmen + graue Füllung). Reicht für den „beladen"-Read. |

**Fazit:** Umsetzbar, rein Frontend+Content. Zwei echte Korrekturen (`chevron`
`pub(super)`; „Level anpassen" ⇒ par/due-Reblessing statt „nur Optik") plus zwei
Präzisierungen (Kanten-Clipping, Cargo als Inset-Band). Größter Aufwand: der
Körper-Teilstück-Helper und das Reblessing aller Level.
