# M2-Restfeature 04 — DIN-artige Schrift + Schriftprüfung

> Schließt die offene GDD-§10-Forderung, die [M2-Plan](../M2-content-maschine.md)
> §3 als DoD führt („Schriftprüfung: DIN-artige Font mit vollständigen
> Umlauten/ß"), für die es aber **kein erfülltes Häkchen und keinen Test** gibt.
> Quelle: GDD §10 + §280 („technischer Beschriftungsschilder (DIN-artig)").

## 1. Problem

Ausgeliefert wird `assets/fonts/DejaVuSansMono.ttf`
([font.rs:8](../../../../src/font.rs)) — eine generische Monospace, die zufällig
Umlaute hat. **DIN-artig ist sie nicht**, und die im DoD versprochene
„Schriftprüfung" existiert nirgends. Das Häkchen bei M2-Plan §3 (Schrift) ist
also unbelegt: Coverage ist nicht getestet, und die Ästhetik (technische
Beschriftungsschilder, GDD §280) ist verfehlt.

DejaVu wurde laut Modul-Doku bewusst gewählt, weil Bevys Default-Font ein
ASCII-Subset ist (Umlaute → Tofu). Das war die richtige Notlösung — aber eben
eine Notlösung.

## 2. Scope

**In:** (a) eine DIN-artige, OFL/Apache/PD-lizenzierte Schrift mit
vollständiger Latin-1-/ß-Abdeckung auswählen und einbinden; (b) ein
**automatisierter Coverage-Test** als die „Schriftprüfung".

**Nicht in:** Mehrere Schriftschnitte/Gewichte, Font-Hot-Reload, CJK. Ein
Display-Schnitt reicht für M2.

## 3. Vorgehen

### 3.1 Schrift auswählen (Kriterien, nicht Bauchgefühl)
Shortlist OFL-/Apache-lizenzierter, DIN-/Signage-naher Schriften mit voller
Latin-1+ß-Abdeckung — z. B. **Barlow (Semi)Condensed**, **Saira (Semi)Condensed**,
**Overpass**, **Public Sans**. Auswahl-Kriterien:
1. Lizenz OFL/Apache/PD (GDD §12.4-Asset-Politik) — Lizenzdatei mit ausliefern,
   wie heute `DejaVuSansMono-LICENSE.txt`.
2. Vollständige Glyphen für **beide** i18n-Tabellen (de/en) **plus** den
   UI-Sonderzeichen-Satz: `● ○ ✓ → ·` (font.rs nennt sie explizit).
3. **Tabellenziffern** (tabular figures) ODER ein begleitender Mono-Schnitt für
   Spalten — sonst zerfällt die Fahrplan-Ausrichtung (siehe §3.3).

### 3.2 Einbinden — Atlas-Falle beachten
`font.rs` lädt **eine Face unter eigenem Handle**; der Kommentar dort
dokumentiert die Atlas-Korruption (Memory `bevy-text-atlas-korruption`): zwei
Faces unter einem Asset-Id teilen einen Glyph-Atlas → Riesen-/Garbage-Glyphen,
gefixt im vendored `bevy_text`. **Den Mechanismus nicht aufweichen**: neue
Schrift = neuer Pfad, eigenes Handle, alte Datei entfernen. Nach dem Tausch
mit `STELLWERK_WINDOWED`/`STELLWERK_AUTOCYCLE` durch viele Menüwechsel prüfen,
ob die Korruption wirklich wegbleibt.

### 3.3 Mono vs. proportional — die Fahrplan-Spalten
Der read-only Kampagnen-Fahrplan
([schedule_panel.rs:73-88](../../../../src/ui/schedule_panel.rs)) richtet Spalten
heute über Monospace + `·`-Trenner aus. Eine proportionale DIN-Schrift bricht
das. Zwei Wege:
- **A (bevorzugt):** Display-Schrift mit Tabular-Ziffern + Spaltenlayout über
  feste `Node`-Breiten statt Space-Padding (deckt sich mit Restfeature 03 §3.2).
- **B:** Zwei Rollen — DIN-Display fürs UI, schmaler Mono nur für Tabellen.
  Mehr Code, mehr Atlas-Risiko. Nur falls A scheitert.

### 3.4 Die „Schriftprüfung" als Test
Ein Test (`tests/font_coverage.rs`), der die ausgelieferte `.ttf` parst (via
`ttf-parser`, transitiv über cosmic-text/swash bereits im Baum) und behauptet:
für **jedes** Zeichen aus `en.ron` ∪ `de.ron` ∪ dem UI-Sonderzeichensatz hat
die Face ein Glyph (cmap-Lookup ≠ 0). Rot bei jeder Lücke — das ist die im DoD
versprochene, bisher fehlende Prüfung. Der UI-Sonderzeichensatz wird **eine
benannte Konstante** (wie `DECODE_ERROR_KEYS`), damit Code und Test nicht
auseinanderdriften.

## 4. Risiken

| Risiko | Plan |
|---|---|
| Atlas-Korruption kehrt zurück | Handle-Disziplin aus font.rs halten; AUTOCYCLE-Sichtprüfung im DoD |
| Proportionalschrift zerschießt Fahrplan-Spalten | Weg A (Tabular + feste Breiten); B als Rückfall |
| Lizenz unklar/inkompatibel | nur OFL/Apache/PD; Lizenzdatei mitliefern; §12.4-Notiz |
| „DIN-artig" ist Geschmackssache | Kriterienliste §3.1 statt Bauchgefühl; finale Wahl ist Design-Call, kein Blocker |
| `ttf-parser` doch nicht im Baum | dann als dev-dependency aufnehmen (Test-only, kein Laufzeit-§12.4-Vorgang) |

## 5. Definition of Done

- [x] **Schriftprüfung** vorhanden: `font::tests::shipped_font_covers_all_ui_glyphs`
      prüft volle Glyph-Abdeckung (de ∪ en ∪ `UI_GLYPHS`) und ist rot bei Lücke.
      (Unit-Test in `font.rs` statt `tests/font_coverage.rs` — der bin-Crate hat
      kein lib-Target, ein Integrationstest käme nicht an `PATH`/Konstanten.)
- [x] **DIN-artige Schrift eingebunden:** Saira Semi Condensed (OFL) als
      `PATH` in `font.rs`; DejaVu + dessen Lizenz entfernt, `OFL.txt`
      ausgeliefert. Schriftprüfung grün gegen Saira.
- [x] **Symbol-Icons gezeichnet (Weg A):** Saira hat keine ●○✓✗▶◀. Medaillen
      und „gelöst" sind jetzt UI-Shapes (`widgets::dot` — Kreis via
      `Node.border_radius`); ▶◀✗ wurden zu »«× (Saira). Betrifft Ergebnis-
      Screen, Level-Liste, Start-/Schließen-Buttons, Diagnosezeile.
- [x] **Fahrplan-Ausrichtung:** kein Problem — der read-only-Kampagnenfahrplan
      ist eine `·`-getrennte Inline-Zeile (keine vertikalen Spalten), die
      Sandbox-Felder haben feste Breiten. Kein Tabular-Figures-Umbau nötig.
- [x] `clippy -D warnings` + `cargo test --workspace` grün. `ttf-parser` als
      dev-dependency (test-only).
- [~] **Atlas-Korruption nach AUTOCYCLE:** manueller Sicht-Check am laufenden
      Spiel steht noch aus (headless nicht prüfbar).
- [ ] **Nach M2 (in [M4](../../M4/M4-launch.md) eingeplant):** echte PNG-Icons
      statt der gezeichneten Shapes (play/back/check/cross/medal-voll/-leer).

## 6. Umsetzungsnotiz

Beide Hälften aus §2 sind drin: Die **Schriftprüfung** (reiner Code, der in der
DoD fälschlich als erledigt galt) deckt i18n (inkl. Umlaute/ß) plus die
hartkodierten Symbole ab. Die **DIN-Schrift** ist eingebaut (Saira Semi
Condensed, OFL). Stolperstein: Saira ist eine reine Textschrift **ohne**
Symbol-Glyphen (▶◀●○✓✗ fehlten — die Prüfung fing es). Statt zweiter Font wurde
**Weg A** gewählt: Status-Icons als `bevy_ui`-Shapes (`dot` = Kreis über
`Node.border_radius`, gefüllt/Ring), Pfeile/Kreuz als Saira-Interpunktion
(»«×). Vor Release ersetzen echte PNG-Icons die Shapes.
