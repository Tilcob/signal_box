# Autoren-CLIs: `par_suggest` & `i18n_fill`

Zwei dev-only Kommandozeilen-Werkzeuge, die die manuellen Nähte beim
Level-Bauen schließen (Content-Maschine-Plan
[plans/M2/M2-content-maschine.md](../plans/M2/M2-content-maschine.md)).
Sie sind in den [Autorenworkflow](level-bauen.md#3-der-autorenworkflow-schritt-für-schritt)
eingebettet — hier steht die ausführliche Referenz mit Beispielen.

> **Voraussetzungen:** dev-Build (Standard) und Ausführung **aus dem
> Repo-Wurzelverzeichnis** — beide lesen/schreiben unter `assets/`. Sie linken
> **kein** Bevy (nur `stellwerk_sim` + `ron`), bauen also in Sekunden. Im
> Ship-Build (`--no-default-features`) existieren sie nicht.

---

## 1. `par_suggest` — Par-Werte beweisbar setzen

**Was es tut:** Fährt **jede** hinterlegte Designer-Lösung (`solutions/<id>.ron`
und Varianten `<id>__*.ron`) headless durch die Sim — exakt wie
`tests/par_proof.rs` — und meldet den **besten erreichten Wert je Achse**
(Minimum über alle Lösungen). Mit `--write` ersetzt es zielgenau nur den
`par: (…)`-Block der Level-Datei; Kommentare, `fixed`-Gleise und Formatierung
bleiben unangetastet.

Damit ist ein Par per Konstruktion **erreichbar** — du erfindest ihn nicht,
du liest ab, was eine echte Lösung schafft.

### Aufrufe

| Befehl | Wirkung |
|---|---|
| `cargo run --bin par_suggest` | Dry-run über **alle** Level: zeigt erreichte vs. aktuelle Pars |
| `cargo run --bin par_suggest -- <id>` | nur dieses eine Level |
| `cargo run --bin par_suggest -- --write` | erreichte Werte in die `par:`-Zeilen zurückschreiben |
| `cargo run --bin par_suggest -- --write <id>` | nur dieses Level schreiben |

### Ausgabeformat

```
<id>: par (throughput: <neu>, material: <neu>, lateness: <neu>)   [aktuell: <alt>, <alt>, <alt>]
```

- `throughput` = Tick der **letzten** Ankunft, `material` = Bau-Kosten,
  `lateness` = Summe Verspätung. **Niedriger ist besser**, Bestwert ≤ Par → Medaille.
- Lösung fehlt / validiert nicht / scheitert → Zeile auf **stderr**, Level übersprungen.

### Beispiel 1 — frisches Level: Par ablesen, dann schreiben

Du hast `k4_04_rangierfahrt.ron` angelegt (Par noch Platzhalter `0,0,0`) und
über „DEV: Lösung sichern" eine Lösung abgelegt. Erst ansehen:

```sh
$ cargo run --bin par_suggest -- k4_04_rangierfahrt
k4_04_rangierfahrt: par (throughput: 142, material: 9, lateness: 0)   [aktuell: 0, 0, 0]

(dry-run — mit `-- --write` in die Level-Dateien zurückschreiben)
```

Sieht plausibel aus → zurückschreiben (nur dieses Level):

```sh
$ cargo run --bin par_suggest -- --write k4_04_rangierfahrt
k4_04_rangierfahrt: par (throughput: 142, material: 9, lateness: 0)   [aktuell: 0, 0, 0]
  → geschrieben
```

Die `par:`-Zeile in `assets/levels/k4_04_rangierfahrt.ron` steht jetzt auf
`(throughput: 142, material: 9, lateness: 0)`.

### Beispiel 2 — Batch-Check, Bestwert über zwei Lösungen

Nach einer Bau-Session über mehrere Level einmal alles prüfen. Legst du neben
`solutions/k2_04_ueberholung.ron` (durchsatz-optimal) noch
`solutions/k2_04_ueberholung__material.ron` (material-sparsam) ab, nimmt
`par_suggest` **je Achse das Beste aus beiden**:

```sh
$ cargo run --bin par_suggest
k1_01_erste_fahrt: par (throughput: 60, material: 4, lateness: 0)   [aktuell: 60, 4, 0]
k2_04_ueberholung: par (throughput: 188, material: 12, lateness: 0)   [aktuell: 200, 14, 0]
k3_04_engpass: keine Designer-Lösung — übersprungen
…
(dry-run — mit `-- --write` in die Level-Dateien zurückschreiben)
```

Hier zeigt `k2_04` einen schärferen Par als bisher (die Material-Variante
drückt `material` von 14 auf 12), und `k3_04` erinnert dich per stderr, dass
noch eine Lösung fehlt. Wenn die Vorschläge passen: `-- --write`.

---

## 2. `i18n_fill` — fehlende Text-Keys ergänzen

**Was es tut:** Geht alle Level unter `assets/levels/` durch und legt **fehlende**
`level.<id>.name` / `level.<id>.briefing` / `station.<LABEL>`-Keys in **beiden**
Tabellen an:

- **`de.ron`** bekommt den authored Wert (deutscher Name/Briefing/Label).
- **`en.ron`** bekommt denselben Wert mit dem Präfix **`[TODO] `** als
  Übersetzungs-Marker.

Es **überschreibt nie** einen vorhandenen Key (echte Übersetzungen sind sicher),
sortiert die Datei **nicht** um (hängt nur vor der schließenden `}` an, kleine
Diffs) und listet zum Schluss alle noch unübersetzten EN-Keys auf.

> Dynamische Sandbox-Senken `Z<n>` (z. B. `Z0`) bekommen **keinen** Key — sie
> fallen auf den Rohwert zurück.

### Aufruf

```sh
cargo run --bin i18n_fill
```

Kein Argument, keine Flags — immer der volle Abgleich.

### Beispiel 1 — neues Level, Keys anlegen + übersetzen

`k4_04_rangierfahrt` ist neu, mit Senke `RANGIER` und einem Briefing. Lauf:

```sh
$ cargo run --bin i18n_fill
i18n_fill: de +3, en +3 key(s).
  English entries marked with "[TODO] " need translating.
Untranslated English keys (3):
  level.k4_04_rangierfahrt.briefing
  level.k4_04_rangierfahrt.name
  station.RANGIER
```

In `en.ron` stehen jetzt z. B.:

```ron
    "level.k4_04_rangierfahrt.name": "[TODO] 4.4 Die Rangierfahrt",
    "station.RANGIER": "[TODO] RANGIER",
```

Die `[TODO] `-Zeilen von Hand übersetzen (Präfix mitsamt Leerzeichen
entfernen): `"4.4 The Shunting Run"`, `"SHUNTING"`. Gegenprüfen:

```sh
$ cargo run --bin i18n_fill
i18n_fill: de +0, en +0 key(s).
All English keys translated.
```

`+0/+0` heißt: nichts fehlt mehr; „All English keys translated" heißt: kein
`[TODO] ` mehr offen.

### Beispiel 2 — idempotent & sicher nach Umbenennung

`i18n_fill` ist gefahrlos wiederholbar. Hast du eine Level-`id` umbenannt
(z. B. den generierten `k4_40_neu` auf `k4_04_rangierfahrt`), legt ein erneuter
Lauf die Keys für den **neuen** Stamm an — die **alten** bleibt es liegen
(es löscht nie):

```sh
$ cargo run --bin i18n_fill
i18n_fill: de +3, en +3 key(s).
…
```

Die verwaisten `k4_40_neu.*`-Keys musst du selbst aus beiden Tabellen
entfernen (sonst meckert kein Test, aber sie sind tote Einträge). Tipp: direkt
nach dem Umbenennen erledigen, solange du weißt, welcher alte Stamm es war.

---

## 3. Wo sie im Workflow sitzen

Reihenfolge beim Bauen (Details: [level-bauen.md §3](level-bauen.md#3-der-autorenworkflow-schritt-für-schritt)):

1. In Sandbox entwerfen → „DEV: Als Kampagnen-Level speichern"
2. Datei feilen (id, `fixed`, `briefing`)
3. Lösung bauen → „DEV: Lösung sichern"
4. **`par_suggest`** → Par scharfstellen
5. **`i18n_fill`** → Text-Keys ergänzen, dann `[TODO]` übersetzen
6. `cargo test` grün ([§4](level-bauen.md#4-was-grün-sein-muss))
7. Tempo in [content-log.md](../plans/M2/content-log.md) notieren

> Merke: `par_suggest` **liest** (Sim-Wahrheit), `i18n_fill` **legt an**
> (nie überschreiben). Beide sind dry-run-freundlich — `par_suggest` ohne
> `--write`, `i18n_fill` durch seine „nie überschreiben"-Garantie.
