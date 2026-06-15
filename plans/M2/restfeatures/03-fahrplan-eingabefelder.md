# M2-Restfeature 03 — Fahrplan-Editor mit echten Eingabefeldern

> Schließt **Umsetzungsnotiz 3** aus [M2-Plan](../M2-content-maschine.md) §8
> („Fahrplan-Editor ist bewusst grob — Zyklus-Knöpfe statt Eingabefelder").
> Quelle: M2-Plan §2.2.

## 1. Problem

Der Fahrplan-Editor in [schedule_panel.rs](../../../src/ui/schedule_panel.rs)
setzt numerische Felder über **Zyklus-Knöpfe**: `BumpDepart` springt in
10er-Schritten modulo 200, `CycleSpeed` rotiert durch `[60,100,150,240]`,
`CycleLength` durch `[800,1400,1800]`. Wer einen Soll-Tick von 137 oder eine
Länge von 1200 LE will, klickt sich tot — oder kann es gar nicht. Für 17
weitere Level (Exit-Kriterium) ist das der langsamste Pfad im
Autorenwerkzeug.

## 2. Scope

**In:** Direkt-Eingabe für die **numerischen** Felder — `depart`, `due`,
`speed`, `length`. Fokussierbares Feld, Tippen, Backspace, Enter committed,
Esc verwirft; Bereichs-Clamp.

**Nicht in:** `source`/`sink`/`class` bleiben Zyklus-Knöpfe (kleine, diskrete
Mengen — da ist Zyklus richtig). Kein Maus-Drag-to-scrub, kein Spinner-Polish
(M3). `train`-Id bleibt automatisch.

## 3. Vorgehen

### 3.1 Wiederverwendbares `numeric_field`-Widget
`bevy_ui` hat kein Texteingabe-Widget — wir bauen ein minimales, fokussierbares
Zahlenfeld in [widgets.rs](../../../src/ui/widgets.rs), damit es auch für
künftige Editoren (Pars, Sandbox-Größe) taugt. Ein eigenes Mini-Modul
`ui/numeric_field.rs` (Modul-pro-Verantwortlichkeit).

- Komponente `NumericField { value: i64, min, max, kind: FieldKind }` plus
  `Focused`-Marker. Klick fokussiert, Klick woanders blurrt.
- Eingabe-System liest **`ButtonInput<Key>` (logische Zeichen), NICHT
  `KeyCode`** — QWERTZ-Falle (siehe Memory `qwertz-tastatur-keycode-falle`):
  Ziffern/Backspace/Enter/Esc als `Key`, sonst tippt der User auf einer
  QWERTZ-Tastatur ins Leere.
- Enter/Blur → clamp auf `[min,max]` → Commit-Event mit altem+neuem Wert.

### 3.2 In den Fahrplan einhängen
`rebuild_schedule_panel`: die vier `small_button(..Bump/Cycle..)` für
depart/due/speed/length durch `numeric_field` ersetzen. `source`/`sink`/`class`
bleiben. Spalten-Ausrichtung über feste `Node`-Breiten (nicht über
Monospace-Spaces — diese kollidiert sonst mit Restfeature 04).

### 3.3 Commit = ein Undo-Op
Jeder committed Wert bucht **einen** `ScheduleEdit{before, after}` über `do_op`
(liefert Restfeature 02 — **02 ist Voraussetzung**). Verworfene/unveränderte
Eingaben buchen nichts.

### 3.4 Validierung am Rand, nicht modal
Clamp statt Fehlermeldung: `depart`/`due` ≥ 0, `speed` in sinnvollem Band,
`length` > 0. Sim-Plausibilität (z. B. Spawn-Lücke 500 LE, content-log §1)
bleibt Sache des Par-Harness, nicht des Editors — der Editor verhindert nur
Unsinn (negativ/Null), kein Game-Design.

## 4. Risiken

| Risiko | Plan |
|---|---|
| Eigenes Texteingabe-Widget = Aufwand/Bugs | Scope eng: nur Ziffern, ein Feld fokussiert, reine Funktionen testbar |
| `KeyCode` statt `Key` → QWERTZ tippt falsch | Hart auf `ButtonInput<Key>`; im DoD geprüft |
| Fokus-Verlust verliert Eingabe | Blur committet (clamp), verwirft nicht |
| Proportionalschrift (Plan 04) zerschießt Spalten | feste Feld-/Spaltenbreiten statt Space-Padding |
| Editor erzwingt Design-Regeln, die das Harness prüft | nur Hard-Clamp (>0), keine Balance-Logik im Editor |

## 5. Definition of Done

- [x] `numeric_field`-Widget (`ui/numeric_field.rs`): fokussierbar, `Key`-basiert,
      clamp, `NumericFieldCommit`-Message (Bevy 0.18: Message statt Event)
- [x] depart/due/speed/length im Sandbox-Fahrplan direkt eingebbar;
      source/sink/class weiter als Zyklus
- [x] Jeder Commit = genau ein `ScheduleEdit` auf dem Undo-Stack (Restfeature 02)
- [x] Reine `commit_value`-Parse/Clamp-Funktion headless unit-getestet
- [~] QWERTZ: Code liest `ButtonInput<Key>` (logisch), nicht `KeyCode` — korrekt
      per Konstruktion; der manuelle Tastatur-Test am Spiel steht noch aus
- [x] `clippy -D warnings` + `cargo test --workspace` grün; M2-Plan §8 Notiz 3 aktualisiert

## 6. Umsetzungsnotizen (Abweichungen)

1. **Tasten-Gating statt nur Feld-Fokus.** Zifferntasten lösen sonst die
   Tool-Hotkeys (1/2/3/4/6/7) aus, Enter den Run-Start. Gelöst über eine
   Resource `FocusedField` + Run-Condition `no_field_focused` (in `state.rs`),
   die `editor::tools::hotkeys`/`pointer` **und** `start_button` deaktiviert,
   solange ein Feld fokussiert ist. Auch der Blur-Klick auf das Board legt so
   kein Gleis.
2. **Esc verworfen, nicht implementiert.** Der Plan nannte „Esc verwirft", aber
   Esc öffnet in Edit das Pausemenü — Hijacking wäre überraschend. Commit per
   Enter; Verwerfen entfällt (Blur committet den geclampten Wert).
3. **Buffer auf aktuellen Wert vorbefüllt** statt leer: klick rein, editier per
   Backspace/Tippen; leerer Buffer = unverändert.
4. **Feld-zu-Feld-Klick:** committet das erste Feld → Panel-Rebuild despawnt das
   eben fokussierte zweite Feld; Fokus wird auf das tote Entity erkannt und
   gelöscht. Folge: zum Editieren des zweiten Felds nochmal klicken. Kleiner
   Wermutstropfen, dokumentiert.
5. **Kurze Präfixe (ab/soll/v/L)** bleiben wie zuvor hartkodiert (keine
   i18n-Keys) — sie waren es bei den Zyklus-Knöpfen schon.
