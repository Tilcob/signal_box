# M2 — Implementierungsplan Content-Maschine

> Abgeleitet aus [GDD](../../GDD.md) §8, §12, §13. GDD bleibt Single Source
> of Truth.
>
> **Rolling-Wave:** Wird bei M2-Start gegen den realen Post-M1-Stand
> geschärft; Wochen-Angaben entstehen erst dann.
>
> **Ziel (GDD §13):** Level-Pipeline, Kapitel 1–4 komplett, Sandbox,
> Sharing-Codes, Lokalisierungs-Setup EN/DE.
> **Exit-Kriterium:** 30+ Level spielbar; ein Level bauen kostet < 1 Tag.
> **Zeitrahmen:** 6 Wochen. **Voraussetzung:** M1-Exit bestanden (sonst ist
> Content-Produktion verfrüht — der ganze Sinn des Slice-Gates).

## 1. Scope

**In M2:**
- `stellwerk_codes`-Crate: Lösungs- und Level-Codes (GDD §8.3)
- Sandbox-Modus inkl. Fahrplan-Editor (GDD §8.2)
- Level-Pipeline: Autorenworkflow, Par-Härtung per Headless-CI, Metadaten
- Lokalisierung: Stringtabellen EN + DE, vollständige Abdeckung, DE-Pass
- Save v2: mehrere Lösungs-Slots pro Level + Bestwerte je Achse (GDD §7.7)
- Content: Kapitel 1–4 komplett (~32 Level gesamt, inkl. der 8 aus M1);
  Zugtypen als Level-Daten eingeführt (Kapitel 4 „Sortierwerk")

**Nicht in M2:** Kapitel 5–6 (M3), Audio, Demo/Steam, Tages-Challenge
(GDD §8.4), Workshop/Server-Anything.

## 2. Die drei Systeme

### 2.1 `stellwerk_codes` (Woche 1)

- **Lösungs-Code** = Level-Referenz (Level-Id + Format-Version) + Spieler-
  `Layout` inkl. Weichenkonfiguration. **Level-Code** = vollständiges
  `Level` (Sandbox-Setups als Custom-Puzzle).
- Format: `postcard` → optional `miniz_oxide` (erst ab ~1500 Zeichen) →
  `base64`; **erstes Byte = Formatversion** (GDD §12.5: Codes überleben
  Updates). Prefix `SW1-…` für Erkennbarkeit in Foren.
- Tests: Roundtrip-Property über zufällige (Seed-LCG) Layouts; je ein
  eingefrorener Goldcode pro Typ, der für immer dekodierbar bleiben muss
  (Regressionsschutz für Versionsbruch).
- UI: Export-Knopf (in Zwischenablage) im Ergebnisbildschirm + Sandbox;
  Import-Dialog mit Validierungsbericht (niemals Panik bei Müll-Eingabe —
  jedes Fehlverhalten hier ist ein Community-Bugreport).

### 2.2 Sandbox + Fahrplan-Editor (Woche 2)

- Neue leere Fläche (Größe wählbar), Quellen/Senken platzieren, Fahrplan
  als Tabelle: Zeile = (Zugtyp, Länge, Speed, Quelle, Ziel, Abfahrt, Soll).
- Kein Ziel, keine Bewertung (GDD §8.2) — aber Export als Level-Code, und
  **genau dieselbe Editor-/Sim-Strecke wie die Kampagne** (keine Forks!).
- Die Sandbox ist ab jetzt auch das interne Level-Autorenwerkzeug — der
  Designer-Workflow ist der Spieler-Workflow plus Par-Felder.

### 2.3 Level-Pipeline (Woche 3)

- Level-Metadaten erweitern: Kapitel, Reihenfolge, Name-Key, Auftrags-Key,
  optionale Schwer-Markierung (GDD §8.1 „optional-schwer").
- **Par-Härtung:** Zu jedem Level liegt mindestens eine Designer-Lösung als
  Lösungs-Code im Repo (`assets/levels/solutions/`). CI fährt alle headless:
  Lösung muss `Success` liefern, und pro Achse muss der Par-Wert von einer
  hinterlegten Lösung erreicht werden — unerreichbare Pars sind damit
  *technisch unmöglich*.
- Level-Lint als Test: Quelle/Senke-Anzahl plausibel, Fahrplan nicht leer,
  Erreichbarkeit der Designer-Lösung, Text-Keys existieren in EN **und** DE.
- **Tempo-Messung ab Woche 4:** Datum/Aufwand je Level in einer simplen
  Tabelle (`plans/M2/content-log.md`) — das Exit-Kriterium braucht Daten,
  kein Gefühl.

## 3. Lokalisierung & Save v2 (parallel zu Content, Woche 4)

- i18n-Shim aus M1 bekommt die DE-Tabelle; ein Test zählt fehlende Keys
  pro Sprache (rot bei Lücke). Sprachumschalter im Menü.
- Schriftprüfung: DIN-artige Font mit vollständigen Umlauten/ß (GDD §10).
- Save v2: Lösungs-Slots (z. B. 3 pro Level) + „Bestwert je Achse" mit
  zugehöriger Lösung (GDD §7.7); Migration von M1-Autosave → Slot 1,
  Migrationstest mit eingefrorenem M1-Save als Fixture.

## 4. Content-Produktion (Wochen 4–6)

| Kapitel | Level-Ziel | Neu (GDD §8.1) |
|---|---|---|
| 1 Blockstrecke | 8 (4 aus M1 überarbeitet) | — |
| 2 Ausweiche | 8 (2 aus M1) | Zielregeln, Gegenverkehr |
| 3 Der Knoten | 8 (2 aus M1) | Kettensignale, Deadlock-Design |
| 4 Sortierwerk | 8 | Zuglängen, Zugtyp-Regeln, Reihenfolge-Aufträge |

- Pro Level: bauen → Designer-Lösungen je Achse → Pars setzen → Lint grün →
  Kurztext EN/DE → ins Kapitel einsortieren.
- Schwierigkeitskurve je Kapitel: 1–2 Lehr-Level (eine Idee, kaum Druck),
  3–5 Anwendung, 2–3 optional-schwer. M1-Playtest-Erkenntnisse einarbeiten.

## 5. Wochenplan

| Woche | Liefert |
|---|---|
| **W1** | `stellwerk_codes` + Tests + Export/Import-UI |
| **W2** | Sandbox + Fahrplan-Editor, Level-Format-Erweiterung |
| **W3** | Par-Härtung in CI, Level-Lint, Autorenworkflow rund; **Levelformat eingefroren** (Codes hängen dran!) |
| **W4** | Lokalisierung komplett (EN/DE), Save v2 + Migration; Content K1 |
| **W5** | Content K2 + K3 (Tempo-Log!) |
| **W6** | Content K4, Kurven-Pass über alles, Exit-Messung |

## 6. Risiken

| Risiko | Plan |
|---|---|
| Levelformat ändert sich nach Code-Release → Codes brechen | Formatfreeze Ende W3; danach nur additive, versionierte Änderungen |
| Content-Tempo > 1 Tag/Level | Log ab W4; bei Überschreitung: Werkzeug-Lücke suchen (Pipeline-Problem), nicht „schneller designen" |
| Par-Werte zu streng/zu lasch | Pars sind CI-beweisbar erreichbar; Feinbalance bewusst erst M4 (mit Telemetrie) |
| Sandbox-Scope-Creep (Skripting-Wünsche) | GDD §8.4 zitieren; Wünsche nach §16 |
| DE-Texte driften | Key-Lücken-Test + Übersetzung im selben Commit wie der EN-Text |

## 7. Definition of Done (M2)

- [ ] 30+ Level, alle mit CI-geprüften Pars und EN/DE-Texten
- [ ] Sharing-Codes: Roundtrip-Tests + Goldcodes; Import übersteht Fuzz-Müll
- [ ] Sandbox erstellt ein Level, das als Code exportiert/importiert/gespielt werden kann
- [ ] Save-Migration M1→M2 getestet; Lösungs-Slots funktionieren
- [ ] Content-Log zeigt < 1 Tag/Level über die letzten 10 Level
- [ ] GDD-Abgleich (Historie), Plan M3 geschärft
