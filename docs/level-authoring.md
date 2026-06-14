# Level bauen — Autoren-Anleitung

Praktische Anleitung für den Kampagnen-Level-Workflow mit den dev-Werkzeugen
(Plan `plans/optimierung/07`). Designidee + Begründung stehen dort; hier steht
nur, **wie man es bedient**.

## Voraussetzungen

- Alles läuft nur im **dev-Build** (Standard) und **aus dem Repo-Verzeichnis**
  heraus — die Werkzeuge schreiben in `assets/`.
- `cargo run` startet das Spiel. Die zwei CLIs brauchen `--bin` (siehe unten).
- Ein Ship-Build (`cargo build --release --no-default-features`) enthält **keine**
  dieser Werkzeuge.

## Der Loop auf einen Blick

```
1. Sandbox: Puzzle bauen (Fläche, Quellen/Senken, Fahrplan)
2. Knopf  „Als Kampagnen-Level speichern"   → assets/levels/<id>.ron
3. Datei:  id umbenennen, meta/briefing feilen   (optional)
4. Level öffnen, Lösung bauen, START
5. Knopf  „DEV: Lösung sichern"             → solutions/<id>.ron
6. CLI    par_suggest --write               → par: in die Datei
7. CLI    i18n_fill + en.ron übersetzen     → Texte
8. cargo test                               → par_proof + i18n grün
```

Jeder Schritt schneidet eine frühere Handarbeit weg. Schritte 6–8 sind die
„bless"-Kür; 1–5 die eigentliche Bauarbeit.

---

## 1. Puzzle in der Sandbox bauen

- Streckenwahl → **NEUE SANDBOX** → Größe wählen (klein/mittel/groß).
- Werkzeuge: `6` Quelle, `7` Senke auf Anschlüsse setzen; Fahrplan-Panel unten
  links: **+ ZUG**, dann pro Zeile mit den Zyklus-Knöpfen Quelle/Ziel/Klasse/
  Abfahrt/Soll/Tempo/Länge einstellen.
- Gleise/Weichen/Signale ziehst du wie immer (`1`–`5`, `R` dreht).

> **Wichtig — was wo landet:** Das gezeichnete Gleis ist der **Spieler-Build**
> (= deine spätere Lösung), nicht die Level-Infrastruktur. Gespeichert wird als
> Level nur die **Definition**: Baufläche, Quellen, Senken, Fahrplan (und ein
> *leeres* `fixed`). Willst du vorplatzierte, unveränderliche Designer-Gleise
> (`fixed`), trägst du die nachträglich in der `.ron` von Hand ein.

## 2. Als Kampagnen-Level speichern

Im Sandbox-Editor unten rechts das dev-Panel **„DEV: Als Kampagnen-Level
speichern"**:

- **Kapitel +** / **Order +10** wählen (Order in 10er-Schritten lässt Platz zum
  späteren Einschieben).
- **💾 Speichern** schreibt `assets/levels/<id>.ron` mit
  - `meta`: dein Kapitel/Order, `optional_hard: false`, leeres `briefing`,
  - `sim`: die Sandbox-Definition,
  - und legt Platzhalter-i18n-Keys in **beide** Tabellen an.
- Die id wird generiert (`k<kapitel>_<order>_neu`, bei Kollision `_2`, `_3`, …)
  und im Panel als `id≈…` vorab angezeigt. Der Katalog wird sofort neu geladen.

## 3. (Optional) Datei feilen

In `assets/levels/<id>.ron`:
- **id umbenennen** (Dateiname) auf den endgültigen Stamm, z. B.
  `k2_05_kreuzungstakt`. Der Stamm ist der stabile Schlüssel für Fortschritt
  und Sharing-Codes — also **jetzt** festlegen, nicht später.
- `briefing` füllen (oder erst in Schritt 7 über i18n).
- Bei Bedarf `fixed`-Gleise eintragen.

> Beim Umbenennen der Datei: die in Schritt 2 angelegten i18n-Keys tragen noch
> die alte id — am einfachsten in Schritt 7 `i18n_fill` neu laufen lassen (legt
> die Keys für die neue id an) und die alten von Hand entfernen.

## 4./5. Lösung bauen und sichern

- Zurück in die Streckenwahl → das neue Level öffnen.
- Lösung bauen → **START**. Bei **Erfolg** erscheint im Ergebnis-Screen der
  dev-Knopf **„DEV: Lösung sichern"** → schreibt `assets/levels/solutions/<id>.ron`.
- Mehrere Lösungen pro Achse? Eine zweite Variante legst du als
  `solutions/<id>__material.ron` (o. ä.) ab — `par_suggest` und `par_proof`
  lesen alle Varianten und nehmen pro Achse das Beste.

## 6. Par setzen (CLI `par_suggest`)

```sh
cargo run --bin par_suggest                # Dry-run: alle Level anzeigen
cargo run --bin par_suggest -- <id>        # nur ein Level
cargo run --bin par_suggest -- --write     # par:-Zeile zurückschreiben
```

Fährt jede Designer-Lösung headless und zeigt den **erreichten** Bestwert je
Achse gegen den aktuellen Par. `--write` ersetzt zielgenau nur den
`par: (…)`-Block — Kommentare/Formatierung bleiben. Default ist Dry-run, damit
das Schärfen bewusst passiert.

## 7. Texte (CLI `i18n_fill`)

```sh
cargo run --bin i18n_fill
```

Ergänzt fehlende `level.<id>.name` / `level.<id>.briefing` / `station.<label>`-
Keys in **beide** Tabellen (DE = authored Wert, EN = mit `[TODO]` markiert),
ordnungserhaltend, ohne bestehende zu überschreiben. Danach in
`assets/i18n/en.ron` die `[TODO]`-Zeilen übersetzen (das Tool listet sie am Ende
auf).

## 8. Verifizieren

```sh
cargo test
```

Relevant:
- `par_proof` — jede Lösung läuft auf `Success`, jeder Par ist erreichbar.
- `language_tables_cover_identical_keys` + `every_level_*_has_a_key` — i18n
  vollständig und in beiden Sprachen.

---

## Dev-Knöpfe in der Streckenwahl

Nur im dev-Build sichtbar:

- **🗑 (neben jedem Level)** — löscht **dieses** Level komplett: `.ron`,
  Solutions, i18n-Keys, Fortschritt. Räumt sauber auf (keine Waisen).
- **DEV: ALLE Level löschen** — wie oben für alle Level, mit **Zwei-Klick-
  Bestätigung** (erster Klick warnt, zweiter führt aus).
- **DEV: Fortschritt zurücksetzen** — leert Builds, Slots, gelöst-Status und
  Bestwerte aller Level (Sprache bleibt). Löscht **keine** Dateien.

## Troubleshooting

- **`cargo run` „could not determine which binary"** — sollte nicht mehr
  auftreten (`default-run = "signal_box"` ist gesetzt). Sonst: `--bin signal_box`.
- **i18n-Paritätstest rot nach neuem Level** — `cargo run --bin i18n_fill`
  laufen lassen; er füllt die fehlenden Keys in beiden Tabellen.
- **`par_proof` rot: „keine Designer-Lösung"** — Schritt 5 vergessen
  (`solutions/<id>.ron` fehlt).
- **Maschinen-geschriebene `.ron` sieht anders aus** (`Tick(60)` statt `60`,
  `Some(…)`) — Absicht: serde-RON schreibt die gewickelte Form ohne den
  `#![enable(…)]`-Header. Parst identisch; bei Bedarf von Hand angleichen.
