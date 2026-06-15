# M2-Restfeature 02 — Sandbox-Level-Edits in den Undo-Stack

> Schließt **Umsetzungsnotiz 2** aus [M2-Plan](../M2-content-maschine.md) §8
> (und die DoD-§8-Aussage „Werkzeug-Lücke = Pipeline-Problem"). Quelle: M2-Plan
> §2.2 („die Sandbox ist auch das interne Level-Autorenwerkzeug").

## 1. Problem

Undo/Redo (`Editor.undo`/`redo`, [ops.rs](../../../src/editor/ops.rs)) deckt
**nur Layout-Bauaktionen** ab: Gleis, Weiche, Signal. Die sandbox-eigenen
Mutationen am **Level** stehen daneben und sind **nicht rückgängig zu machen**:
- Quelle/Senke setzen — [tools.rs:194-219](../../../src/editor/tools.rs)
  (`active.level.sources/sinks.push`, direkt am Level, ohne Op)
- Fahrplan-Zeilen add/remove/edit — [schedule_panel.rs](../../../src/ui/schedule_panel.rs)
  (`active.level.schedule…`, direkt)

Ein Autorenwerkzeug ohne Undo für genau die Aktionen, die man beim
Leveldesign am häufigsten macht, sabotiert das M2-Exit-Kriterium
„< 1 Tag/Level". Eine versehentlich gelöschte Fahrplanzeile bedeutet
Neutippen.

## 2. Scope

**In:** Quelle/Senke setzen & löschen, Fahrplanzeile add/remove und jede
Feld-Änderung kommen auf **denselben** Undo-Stack wie Layout-Ops — eine
durchgehende Ctrl+Z-Zeitachse, keine zweite.

**Nicht in:** Undo für die Sandbox-Flächengröße (das ist ein Setup-Schritt vor
dem Editor, kein Edit). Coalescing mehrerer schneller Edits zu einem Op =
optional (siehe §3.4).

## 3. Vorgehen

### 3.1 Op-Vokabular erweitern, nicht parallelisieren
`EditOp` operiert heute über `apply(layout: &mut Layout, …)`. Quelle/Senke/
Fahrplan leben auf `Level`. **Kein zweiter Stack** (sonst zwei Ctrl+Z-Zeiten,
die sich beim Interleaving widersprechen). Stattdessen ein gemeinsames
Edit-Ziel:

```
struct EditTarget<'a> { layout: &'a mut Layout, level: &'a mut Level }
```

`apply`/`invert` arbeiten künftig über `EditTarget`. Neue Varianten:
- `PlaceSource(SourceDef)` / `RemoveSource(SourceDef)`
- `PlaceSink(SinkDef)` / `RemoveSink(SinkDef)`
- `ScheduleAdd(ScheduleEntry)` / `ScheduleRemove { row, entry }`
- `ScheduleEdit { row, before: ScheduleEntry, after: ScheduleEntry }`

`ScheduleEdit` speichert den ganzen Eintrag vor/nach — invertierbar ohne
Feld-Granularität, robust gegen künftige Feld-Erweiterungen.

### 3.2 `do_op`-Signatur
`do_op(editor, op)` sieht heute nur `editor.layout`. Es bekommt das
Edit-Ziel (Layout **und** Level, also `&mut Editor` + `&mut Level` aus
`ActiveLevel`). Alle Aufrufstellen in `tools.rs` mitziehen. Layout-only-Ops
funktionieren unverändert (rühren `level` nicht an).

### 3.3 Aufrufstellen umstellen
- `tools.rs` Quelle/Senke: statt `active.level.sources.push(...)` →
  `do_op(.., EditOp::PlaceSource(def))`.
- `schedule_panel.rs` `schedule_clicks`: jede Mutation wird ein Op. Cycle/Bump
  erzeugen `ScheduleEdit{before, after}` (alten Eintrag klonen, mutieren, als
  Op buchen). Add/Remove analog.

### 3.4 Coalescing (optional)
Bump-Knöpfe (depart/due 10× tippen) erzeugen sonst 10 Undo-Einträge. Minimal:
je Klick ein Op (akzeptabel). Optional: aufeinanderfolgende `ScheduleEdit` auf
dieselbe `row`+dasselbe Feld innerhalb eines kurzen Fensters verschmelzen.
Bewusst nachrangig — erst Korrektheit.

### 3.5 Wechselwirkung mit Restfeature 03
Der Fahrplan-Editor mit Eingabefeldern (Plan 03) bucht jede committed Eingabe
als `ScheduleEdit`. Dieser Plan liefert das Op-Vokabular, Plan 03 die UI.
Reihenfolge: **02 vor 03**.

## 4. Risiken

| Risiko | Plan |
|---|---|
| Op über Level in Kampagnen-Leveln angewandt | Level-Ops entstehen nur im Sandbox-Pfad (`active.sandbox`); Guard bleibt |
| `do_op`-Signaturbruch → viele Call-Sites | Mechanisch, vom Compiler geführt; in einem Commit |
| Stale `row`-Index nach Remove+Undo | `ScheduleRemove` speichert Eintrag + Position; invert fügt an exakt der Stelle wieder ein |
| Borrow-Konflikt (Editor + ActiveLevel je `ResMut`) | beide sind eigene Resources — getrennt entlehnbar |

## 5. Definition of Done

- [ ] `EditOp` deckt Source/Sink/Schedule ab; `apply`/`invert` über `EditTarget`
- [ ] Quelle/Senke/Fahrplan in `tools.rs` + `schedule_panel.rs` laufen über `do_op`
- [ ] Ctrl+Z/Ctrl+Y machen Level-Edits in der Sandbox rückgängig/wieder
- [ ] Property-Test: `invert∘apply == identity` über zufällige Level-Op-Folgen
- [ ] `clippy -D warnings` + bestehende Editor-Tests grün
- [ ] M2-Plan §8 Notiz 2 + Modul-Doku in [editor/mod.rs:8-10](../../../src/editor/mod.rs)
      („sit outside the undo stack") aktualisiert
