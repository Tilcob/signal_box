# M2-Restfeature 01 — Sharing-Codes über die Zwischenablage

> Schließt **Umsetzungsnotiz 1** aus [M2-Plan](../M2-content-maschine.md) §8.
> Quelle: GDD §8.3 + M2-Plan §2.1 („Export-Knopf (in Zwischenablage)";
> „Import-Dialog … niemals Panik bei Müll-Eingabe").

## 1. Problem

Der Sharing-Code ist das Community-Feature von M2 — er ist dafür gebaut, in
**Foren** gepostet zu werden (Prefix `SW1-…`, GDD §12.5). Ausgeliefert wird er
aber über Dateien: Export schreibt `stellwerk_code.txt`, Import liest
`stellwerk_import.txt`. Der Spieler muss also eine Textdatei aus dem
Arbeitsverzeichnis fischen, bevor er etwas teilen kann. Das ist kein
M3-Polish, das ist der Kern-UX-Pfad des Features — und er fehlt.

Drei Aufrufstellen schreiben/lesen heute Dateien:
- [result.rs:249](../../../../src/ui/result.rs) — Lösungs-Code (Ergebnisbildschirm)
- [edit_hud.rs:361](../../../src/ui/edit_hud.rs) — Level-Code (Sandbox-Export)
- [actions.rs:57](../../../../src/ui/select/actions.rs) — Import (Level-Select)

## 2. Scope

**In:** Export legt den Code in die Systemzwischenablage; Import liest aus ihr.
Datei bleibt als **Fallback** (Headless/Wayland-ohne-Manager/CI), nie als
Primärpfad. Lokalisierte Statusmeldung sagt, welcher Weg genommen wurde.

**Nicht in:** QR-Codes, Web-Share, Code-Historie. Import bleibt „eine Quelle,
ein Knopf" — kein Dialog mit Vorschaubaum (das wäre M3-Politur).

## 3. Vorgehen

### 3.1 Dependency-Entscheidung (zuerst!)
- `arboard` aufnehmen — die Verschiebung nach M3 war allein wegen dieser
  Dependency. Das ist ein **GDD-§12.4-Vorgang**: Eintrag in die Ausnahmeliste
  (Begründung: plattformübergreifende Zwischenablage, keine sinnvolle
  Eigenbau-Alternative) **und** Historie-Eintrag in GDD §16. Ohne den Eintrag
  ist der PR formal nicht regelkonform.
- `arboard` ist plattformübergreifend, zieht aber auf Linux X11 die bekannte
  Eigenheit nach: Clipboard-Inhalt gehört dem Prozess und ist nach Beenden weg,
  falls kein Clipboard-Manager läuft (`SetExtLinux`/Wartetrick nötig). → Risiko.

### 3.2 Seam: `src/clipboard.rs`
Ein dünnes Modul, damit die drei Aufrufstellen identisch sind und die Logik
ohne OS-Clipboard testbar bleibt:
- `fn copy(text: &str) -> CopyOutcome` — versucht Clipboard, fällt auf
  `stellwerk_code.txt` zurück; gibt zurück, welcher Weg griff.
- `fn paste() -> Result<String, PasteError>` — Clipboard zuerst, dann
  `stellwerk_import.txt`.
- `CopyOutcome { Clipboard, File(PathBuf), Failed(String) }` →
  übersetzt in `result.exported` / `result.exported_file` / `*_failed`.

### 3.3 Aufrufstellen umstellen
Die drei `std::fs::write/read_to_string`-Stellen rufen nur noch
`clipboard::copy/paste` und mappen `CopyOutcome`/`PasteError` auf bestehende
i18n-Keys. Decode-Pfad in `actions.rs` bleibt unverändert (Müll-Eingabe ist
schon panic-frei und lokalisiert — nicht anfassen).

### 3.4 i18n
Neue Keys in `en.ron`+`de.ron`: `result.exported_file` („… in Datei,
Zwischenablage nicht verfügbar"), `select.import_clipboard_empty`,
`import.error.clipboard`. Paritäts- und Coverage-Test grün halten
(siehe `DECODE_ERROR_KEYS`-Muster in actions.rs).

## 4. Risiken

| Risiko | Plan |
|---|---|
| X11 ohne Clipboard-Manager: Code nach Quit weg | `SetExtLinux::wait()`/Doku; Datei-Fallback bleibt als Netz |
| Headless-CI/Tests haben kein Clipboard | Logik im Seam, `copy/paste` greifen nie im Test; Roundtrip-Tests bleiben auf `stellwerk_codes` (encode/decode), nicht auf OS |
| Neue Dependency ohne §12.4-Eintrag | PR-Checkliste: kein Merge ohne Ausnahmelisten- + Historie-Eintrag |
| Wayland-Backend variiert | `arboard` wählt Backend selbst; Fehler → Fallback, nie Panik |

## 5. Definition of Done

- [x] `arboard` in `Cargo.toml`, GDD §12.2-Crate-Tabelle + §16-Historie ergänzt
      (kein Ausnahmelisten-Eintrag nötig: `arboard` ist nicht Bevy-gekoppelt)
- [x] `src/clipboard.rs` mit `copy`/`paste` + Datei-Fallback; alle drei
      Aufrufstellen nutzen es, kein direktes `fs::write` auf die Code-Dateien mehr
- [x] Export meldet lokalisiert, ob Clipboard oder Datei; Import liest Clipboard
- [x] Müll-Eingabe weiterhin panik-frei und lokalisiert (Regression der
      bestehenden Decode-Tests)
- [x] i18n-Paritätstest grün; `clippy -D warnings` grün; `cargo test --workspace` grün
- [x] M2-Plan §8 Notiz 1 von „verschoben" auf „erledigt" aktualisiert

## 6. Umsetzungsnotiz

- **§12.4 Pkt. 1 vs. Realität:** Der GDD-Eintrag landete im selben Commit wie
  der `Cargo.toml`-Eintrag, nicht streng davor — die Reihenfolge-Regel ist hier
  pragmatisch als „im selben Schritt" gelesen.
- **`default-features = false` verworfen:** zunächst gesetzt, dann zurück auf
  schlichtes `arboard = "3"` — `image-data` ist ohnehin kein Default-Feature,
  und das Abschalten hätte unnötig die Wayland-Unterstützung gekappt.
- **Kein OS-Clipboard-Test:** der Clipboard-Pfad ist headless nicht
  zuverlässig testbar; die Logik sitzt deshalb im dünnen `clipboard`-Seam, die
  Roundtrip-Garantie bleibt bei den `stellwerk_codes`-encode/decode-Tests.
