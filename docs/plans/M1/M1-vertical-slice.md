# M1 — Implementierungsplan Vertical Slice

> Abgeleitet aus [GDD](../../../GDD.md) §5, §9, §10, §12, §13. GDD bleibt Single
> Source of Truth; Design-Konflikte fließen zuerst dorthin zurück.
>
> **Rolling-Wave:** Dieser Plan wird bei M1-Start gegen den realen
> Post-M0-Stand geschärft. Die Wochen-Angaben (LocalVault-Stil) entstehen
> erst dann — ihre Code-Skelette müssen zum tatsächlichen Code passen.
>
> **Ziel (GDD §13):** Editor (Bauen/Undo), Run Mode mit Speed-Controls,
> Debugging-Report, Bewertung, 8 Level aus Kapitel 1–3, Pult-Look v1.
> **Exit-Kriterium:** Fremde Testspieler lösen Kapitel 1 ohne Hilfe;
> „noch ein Versuch"-Sog spürbar — sonst Design nachschärfen, bevor Content
> entsteht.
> **Zeitrahmen:** 6 Wochen. **Voraussetzung:** M0 DoD komplett (CI grün).

## 1. Scope

**In M1:**
- Neues Bevy-Frontend in `src/` — ersetzt den Live-Dispatcher-Prototyp
  vollständig (der Prototyp wird jetzt erst gelöscht; bis dahin war er
  Referenz für Tempo/Gefühl, GDD §14)
- App-Zustände: Levelauswahl → Edit Mode ⇄ Run Mode → Ergebnis
- Editor: Platzieren/Abreißen aller vier Bausteine, Klick-Drag-Gleisziehen,
  Hotkeys 1–4, R = rotieren, Flächen-Abriss, Undo/Redo,
  Weichen-Konfiguration (Grundstellung + Regelliste), Live-Validierung +
  Erreichbarkeits-Warnung (`check_reachability` aus M0)
- Run Mode: Sim-Anbindung mit Tick-Akkumulator, Pause/1×/4×/16×,
  Einzel-Tick, Interpolation der Zugpositionen
- Debugging: Kollisions-/Deadlock-/Fehlleitungs-Report mit
  Zyklus-Hervorhebung; klickbarer Zug zeigt Route + Haltegrund (GDD §3.4)
- Bewertung: Ergebnisbildschirm 3 Achsen vs. Par, Medaillen; Autosave der
  Lösung pro Level
- Pult-Look v1 (GDD §10): lyon-Meshes, Block-Ausleuchtung, HDR-Bloom,
  Zustands-Muster (Barrierefreiheit §9)
- 8 Level aus Kapitel 1–3 als RON-Assets + Levelauswahl
- i18n-Shim: alle UI-Strings über Key-Lookup (Tabelle EN); DE-Inhalte erst M2
- Playtest-Runde mit Auswertung (Exit-Kriterium)

**Nicht in M1:** Sandbox, Sharing-Codes, DE-Übersetzung, Audio (nur
Platzhalter-Klicks), Steam, mehrere Lösungs-Slots (ein Autosave pro Level;
Slots in M2), Menü-Polish.

## 2. Architektur

```
src/
├── main.rs          # Composition Root (DefaultPlugins, Plugin-Liste)
├── core/            # GameState-Maschine, Tunables (aus Prototyp übernommen)
├── ui_kit/          # Pult-Styling-Bausteine für bevy_ui (Panel, Knopf,
│                    # DIN-Beschriftung) — einmal bauen, überall nutzen
├── i18n/            # Key-Lookup über RON-Stringtabellen (EN in M1)
├── board/           # Darstellung der Anlage: lyon-Tessellation → Meshes,
│                    # Ausleuchtungs-Zustände, Bloom-Setup, Punkt→Welt-Mapping
├── camera.rs        # Pan/Zoom (Action-Maps via bevy_enhanced_input)
├── editor/          # EditOps + Command-Stack, Werkzeuge, Weichen-Panel,
│                    # Validierungs-/Erreichbarkeits-Anzeige
├── run/             # Sim-Treiber: Akkumulator, Speed, Interpolation, Events
├── report/          # Fehlschlag-Reports + Zugdetails (Haltegrund)
├── score/           # Ergebnisbildschirm, Medaillen, Par-Vergleich
└── levels/          # Level-Assets laden, Fortschritt + Lösungs-Autosave
```

Grundsätze:
- Frontend hält im Edit Mode `Level` + `Layout` + Command-Stack; im Run Mode
  zusätzlich `Sim`. Es redet mit dem Kern **nur** über dessen API (GDD §12.1).
- **Koordinaten-Mapping:** Sim-Punktgitter (verdoppelte Koordinaten) → Welt:
  `welt = punkt × (ZELLE_PX / 2)`; eine Konstante, ein Helfer in `board/`,
  nirgendwo sonst gerechnet.
- **Editor-Operationen** als invertierbares `enum EditOp` (Platzieren,
  Abreißen, Weichenkonfig ändern …) mit `apply`/`invert` — derselbe
  Operationsbegriff trägt in M2 das Sharing-Format.
- **Tick-Treiber:** Akkumulator mit nominal 10 Ticks/s × Speed-Faktor;
  Render interpoliert zwischen letztem und aktuellem Snapshot (zwei
  Positionsstände je Zug genügen). `SimEvent`s aus `step()` speisen später
  Audio (M3) — jetzt schon durchschleifen.

## 3. Schlüsselentscheidungen (vorab festgelegt)

| Thema | Entscheidung |
|---|---|
| Weichen-Konfig-UI | Klick auf Weiche öffnet Panel: Grundstellung umschalten + Regelliste (Zeile = Bedingung-Dropdown + Zweig-Knopf, Reihenfolge per ↑/↓). Kein Drag-and-drop, kein Freitext. |
| Validierungsfehler | Nie modal: fehlerhafte Elemente leuchten am Pult (Muster + Tooltip), der Start-Schalter ist gesperrt, solange Fehler existieren. |
| Erreichbarkeits-Warnung | Nicht blockierend (Warnung ≠ Fehler): Start trotzdem möglich — die Fehlleitung zu *sehen* ist Lernmoment (Säule 4). |
| Run→Edit-Rückweg | Ein Klick; Undo-Stack überlebt den Run (GDD §9). Die Sim-Historie-Zeitleiste aus GDD §5 Phase 3 ist M1-minimal: letzter Outcome-Report bleibt einsehbar, volle Zeitleiste erst, wenn Playtests sie fordern. |
| Levelformat | Exakt das M0-RON (`Level` + Designer-`Layout` als `fixed`); Level = eine Datei unter `assets/levels/kNN_MM_name.ron`. |
| Fortschritt | Eine RON-Datei im Plattform-Konfigverzeichnis (`directories`-Crate → in GDD §12.2 eintragen, bevor sie in Cargo.toml landet). |

## 4. Wochenplan

| Woche | Liefert | Nachweis |
|---|---|---|
| **W1** | Prototyp raus; App-Gerüst: Zustände, Kamera (Pan/Zoom), Input-Action-Maps, Board-Rendering v0 (Linien statt Schönheit) für ein hartkodiertes Level | Level sichtbar, Kamera fühlt sich gut an |
| **W2** | Editor-Kern: alle EditOps + Undo/Redo, Drag-Gleisziehen, Validierungs-Anzeige, Weichen-Panel | Anlage aus s06 (M0) von Hand nachbaubar, Fehler sichtbar |
| **W3** | Run Mode: Sim-Treiber, Speeds, Einzel-Tick, Interpolation, Moduswechsel-Schalter; **Greybox-Validierung beginnt** (GDD §13: Kernfrage vor Optik!) | M0-Szenarien im Frontend abspielbar; erste Eigen-Playtests |
| **W4** | Reports (Kollision/Deadlock/Fehlleitung + Zyklus-Highlight), Zug-Klick → Haltegrund, Ergebnisbildschirm + Medaillen, Lösungs-Autosave | Deadlock aus s13 wird verständlich erklärt |
| **W5** | Pult-Look v1: lyon-Meshes, Ausleuchtungszustände (frei/reserviert/besetzt/Fahrstraße — Farbe **und** Muster), Bloom, Zug-Lichtbänder + Nummern | Screenshot ist capsule-tauglich „genug" |
| **W6** | 8 Level (K1: 4, K2: 2, K3: 2), Levelauswahl, Fortschritt; **Playtest-Runde** ≥ 3 fremde Tester, Protokoll, Exit-Bewertung | Exit-Kriterium dokumentiert erfüllt/verfehlt |

## 5. Exit-Kriterium konkret machen

- ≥ 3 Tester ohne Vorwissen, Aufgabe: „Spiel Kapitel 1 durch" — ohne
  mündliche Hilfe. Protokoll je Tester: Wo gestockt? Welche Fehlermeldung
  nicht verstanden? Wie oft freiwillig „noch ein Versuch" nach Fehlschlag?
- **Bestanden:** alle Tester schaffen K1; mindestens zwei optimieren
  freiwillig ein gelöstes Level erneut.
- **Verfehlt:** Stoppen, Ursachen ins GDD (§16 oder Säulen-Anpassung),
  M2 verschiebt sich — genau dafür ist der Slice da.

## 6. Risiken

| Risiko | Plan |
|---|---|
| bevy_ui frisst Zeit | `ui_kit/` zuerst, dann nur noch komponieren; egui bleibt dev-only-Tabu (GDD §12.2) |
| Weichen-Panel wird UX-Sumpf | Festgelegte Minimal-Form (§3); jede Erweiterung erst nach Playtest-Beleg |
| Interpolations-Ruckeln | Snapshot-Paar + fester Akkumulator von Anfang an; nie Positionen im Frontend „weiterrechnen" |
| Optik vor Spielgefühl | Pult-Look ist bewusst W5, Greybox-Test W3 — Reihenfolge ist der Schutz |
| Bevy-Upgrade-Verlockung | Version gepinnt bis M1-Ende (GDD §12.2) |

## 7. Definition of Done (M1)

- [x] Code-Stand: 8 Level laden + validieren (Test `tests/levels.rs`); Editor
      (Werkzeuge, Drag-Zeichnen, Undo/Redo, Weichen-Panel, Live-Validierung,
      Erreichbarkeits-Warnung), Run-Mode (Pause/1×/4×/16×, Einzel-Tick),
      Ergebnis-Overlay mit Par-Vergleich + Medaillen, Fortschritts-Autosave
- [ ] **Manuell verifizieren:** alle 8 Level im Frontend durchspielen;
      Par-Werte sind angesetzt, nicht bewiesen (Harness kommt in M2)
- [x] Alle Fehlschlag-Typen erzeugen Reports (Kollision/Deadlock-Zyklus/
      Fehlleitung mit Schuld-Weiche/Stillstand) — Texte im Ergebnis-Overlay
- [x] Undo/Redo über invertierbare `EditOp`s (auch `Group` für Drags und
      `Configure` fürs Weichen-Panel); Layout überlebt Runs
- [x] Workspace-CI erweitert (`app`-Job: `cargo test --workspace` + clippy
      auf Windows/Linux) — Beweis steht nach dem ersten Push aus
- [ ] **Offen: Playtest-Runde** (≥ 3 fremde Tester, Protokolle nach
      `plans/M1/playtest/`, Exit-Entscheid) — das Exit-Kriterium ist NICHT
      bewertet; ohne bestandenen Test startet M2 nicht
- [x] GDD-Abgleich: Abweichungen dokumentiert (§8 unten + GDD-Historie)

## 8. Umsetzungsnotizen (Abweichungen des Slice)

1. **`bevy_enhanced_input` noch nicht integriert** — Kamera/Hotkeys laufen
   über Bevys eingebautes Input-API. Die Action-Maps kommen zusammen mit dem
   Rebinding-UI (GDD §9); bis dahin eine Dependency weniger im Risiko.
2. **`lyon` noch nicht nötig:** Im Stub-Modell ist jede Kante geradlinig —
   rotierte Quads + HDR-Bloom ergeben den Pult-Look ohne Tessellation.
   Wieder prüfen, wenn Kurvenoptik/runde Kappen gewünscht sind.
3. **Fortschritt** liegt in `./stellwerk_progress.ron` (Arbeitsverzeichnis);
   Umzug ins Plattform-Konfigverzeichnis (`directories`) mit Save v2 in M2.
4. **i18n-Shim** steht (alle UI-Texte über `i18n::t`), gefüllt mit DE-Strings;
   EN/DE-RON-Tabellen kommen in M2 (GDD-Reihenfolge EN primär gilt ab dann).
5. **Weichen-Panel:** Regel-Reihenfolge fix (Ziel-Regeln vor Klassen-Regeln)
   statt frei sortierbar — M1-minimal laut §3, freie Ordnung in M2.
6. **Interpolation:** Zugkörper tick-genau, Kopflicht zwischen Ticks
   interpoliert — bewusste Vereinfachung; bei 10 Ticks/s im Playtest prüfen.
7. **Kein Kantenbau per Klick auf Endzellen:** Drag-Zeichnen füllt nur
   Innenzellen des Pfads; Anschluss an festes Gleis durch Start/Ende AUF dem
   festen Gleis. Einzelklick platziert die R-Variante.
