# Kapitel 2 — 10 große, schwere Level

Status: Design abgestimmt (Scope + Größe + Detailtiefe entschieden), noch nicht gebaut.

Entschieden:
- **Mechanik: alles erlaubt** — Weichen + programmierte Regeln (Klasse/Ziel),
  Block- & Kettensignale, Signal-Prioritäten, Ausweichgleise, flache Kreuzungen,
  Frachtbahnsteige/Dwell, mehrere Zugklassen. Kapitel 2 ist damit eine reine
  Schwierigkeitsstufe (ignoriert die Lern-Reihenfolge der Kapitel).
- **Größe: 7 Riesig + 3 Gewaltig.**
- **Detailtiefe: Design-Brief pro Level** (kein Zell-Layout).

---

## Rahmen & Konventionen

- **Dateien/Reihenfolge:** `k2_01`…`k2_10`, `meta.chapter: 2`, `order: 10…100`.
- **Zugklassen (durchgängig):** `0` Nahverkehr · `1` Güter (trägt Bahnsteig-Halt)
  · `2` Express (enges `due` + höhere Signal-Priorität).
- **Größen:** Riesig ≈ **22×13** Bauzellen, 8–12 Züge, 4–6 Ziele ·
  Gewaltig ≈ **28×16**, 12–18 Züge, 6–8 Ziele. (Board-Limit steht auf 50×50, alle
  Größen sind im Sandbox-Editor baubar.)
- **Medaillen-Achsen:** Durchsatz (letzte Ankunft) · Material (Gleis+Signale) ·
  Pünktlichkeit (Lateness vs. `due`). Jedes Level betont eine andere Primärachse.
- **Schwierigkeit = 3 Akte:** I „Das Geflecht" (1–3, Routing/Sequenz) →
  II „Unter Last" (4–7, Kreuzungen/Prioritäten/Fracht) →
  III „Volllast" (8–10, Gewaltig-Integration).

## Design-Leitplanken (aus dem Sim-Kern, nicht verhandelbar)

- **Gedächtnisloses Routing:** eine Weiche löst pro `(Klasse, Ziel)` **immer
  gleich** auf. ⇒ Zwei Züge, die sich physisch trennen müssen, brauchen
  **unterschiedliches Ziel oder Klasse**. Kein Level verlangt, dass
  gleich-`(Klasse,Ziel)`-Züge verschiedene Wege nehmen.
- **Fracht:** Bahnsteig liegt auf **Durchfahrt-Gleis** vor dem Ziel; pro
  `(Klasse,Ziel)`-Route **ein** Pflicht-Bahnsteig. Mehrere Güterzüge zu
  verschiedenen Rampen ⇒ verschiedene Ziele.
- **Nur vorwärts, kein Kopfmachen.** „Pendeln" = viele Einweg-Züge in beide
  Richtungen.
- **Deadlock/Stall:** Kettensignal schützt ganze Fahrstraße durch einen Knoten;
  Blocksignal nur einen Block; Priorität bricht Gleichstand.

---

## Übersicht

| # | Titel | Größe | Fokus | Primärmedaille |
|---|---|---|---|---|
| 1 | Der Fächer | Riesig | Ziel-Routing-Baum | Material |
| 2 | Gegenverkehr | Riesig | Bidirektional + Ausweichen | Durchsatz |
| 3 | Das Nadelöhr | Riesig | Engpass sättigen | Durchsatz |
| 4 | Kreuz und quer | Riesig | Flache Kreuzungen | Material |
| 5 | Zwei Klassen | Riesig | ClassIs + Priorität | Pünktlichkeit |
| 6 | Fracht im Fluss | Riesig | Dwell als Hindernis | Durchsatz |
| 7 | Rangier-Ballett | Riesig | Multi-Fracht-Routing | Pünktlichkeit |
| 8 | Der Verschiebebahnhof | **Gewaltig** | Alles kombiniert | alle drei |
| 9 | Zwei Bahnhöfe | **Gewaltig** | Gegenverkehr XXL + Fracht | Durchsatz+Pünktl. |
| 10 | Hauptbahnhof | **Gewaltig** | Integrations-Finale | alle drei (eng) |

---

## Die Level

### k2_01 · Der Fächer *(Riesig)*
- **Roster:** ~8 Züge Klasse 0, 1 Quelle, **5 Ziele**.
- **Kern:** großer Weichen-Verteilbaum; jede Weiche per `DestIs` programmieren,
  Sammelläufe mit Blocksignal sichern.
- **Warum schwer:** eine falsche Regel = Fehlleitung; dichter Takt erzeugt
  Kollisionen an Ver-/Zusammenläufen.
- **Medaillen:** Material (schlankster Baum), Durchsatz.
- **Lösungsskizze:** Hauptast fix → Spieler baut die Verteilweichen + je Ziel eine
  `DestIs`-Regel + ein Blocksignal je Sammelpunkt.
- **Briefing:** „Acht Züge, fünf Ziele, ein Verteiler. Jede Weiche muss wissen,
  wohin — und jeder Zusammenlauf muss gesichert sein."

### k2_02 · Gegenverkehr *(Riesig)*
- **Roster:** ~10 Züge Klasse 0, 2 Terminals (Quelle+Ziel je Ende), eingleisige
  Hauptstrecke, **2 Ausweichen**.
- **Kern:** bidirektionaler Einspurbetrieb; Begegnungen nur an den Loops,
  Kettensignale an den Einfahrten.
- **Warum schwer:** falsche Sequenz = Frontal-Deadlock; enger Takt für die
  Durchsatz-Medaille.
- **Medaillen:** Durchsatz, Material.
- **Lösungsskizze:** Kette an jeder Loop-Einfahrt („warte, bis Gegenrichtung
  frei"); Reihenfolge über Abfahrtstakt gesteuert.
- **Briefing:** „Eine Spur, zwei Richtungen. Begegnungen nur an den Ausweichen —
  sonst steht alles."

### k2_03 · Das Nadelöhr *(Riesig)*
- **Roster:** ~12 Züge aus **4 Quellen** → 1 Engpass (1 Block) → **3 Ziele**.
- **Kern:** Fan-in durch einen Ein-Block-Engpass, danach Auffächerung; Kette vor
  dem Engpass.
- **Warum schwer:** Engpass darf nie zwei Züge halten, aber auch nie leerlaufen ⇒
  Durchsatz am Limit.
- **Medaillen:** Durchsatz, Pünktlichkeit.
- **Lösungsskizze:** Kettensignal vor dem Engpass (niemand strandet drin),
  `DestIs`-Verteilung dahinter; Prioritäten glätten die Quellen-Konkurrenz.
- **Briefing:** „Zwölf Züge, ein Nadelöhr. Halten Sie den Fluss dicht, ohne dass
  jemand darin strandet."

### k2_04 · Kreuz und quer *(Riesig)*
- **Roster:** ~8–10 Züge, 4 Quellen/4 Ziele über Eck, **mehrere flache
  Kreuzungen** im Zentrum.
- **Kern:** jede Kreuzung mit Kettensignal schützen — aber nicht übersichern.
- **Warum schwer:** zu gierige Ketten → Gridlock/Stillstand; die Balance ist der
  Puzzle-Reiz.
- **Medaillen:** Material (wenige Signale), Durchsatz.
- **Lösungsskizze:** nur real konfligierende Schnittpunkte mit Kette,
  Prioritäten für die Durchgangsrichtung.
- **Briefing:** „Vier Linien, ein Kreuz. Sichern Sie jeden Schnittpunkt — aber
  nicht so streng, dass sich alle gegenseitig lähmen."

### k2_05 · Zwei Klassen *(Riesig)*
- **Roster:** ~10 Züge, Klasse **0** + **2** (Express), gemeinsame Quellen,
  klassenspezifische + ein geteiltes Ziel.
- **Kern:** `ClassIs`-Weichen trennen die Klassen; Express hat enges `due` und
  höhere Signal-Priorität.
- **Warum schwer:** gleiche Quelle, sofort auseinandersortieren; Express muss
  trotz Gedränge pünktlich → Priorität an den richtigen Konfliktsignalen.
- **Medaillen:** Pünktlichkeit (Express-`due`), Material.
- **Lösungsskizze:** `ClassIs`-Trennung früh, Express-Route bekommt Priorität, wo
  sie den Nahverkehr kreuzt/mergt.
- **Briefing:** „Nahverkehr und Express teilen sich den Start. Nach Klasse
  sortieren — und dem Express Vorfahrt geben, wo es zählt."

### k2_06 · Fracht im Fluss *(Riesig)*
- **Roster:** 8× Klasse 0 + **2× Güter** (Klasse 1) mit Bahnsteig-Halt auf der
  Hauptader.
- **Kern:** haltender Güterzug = zeitlich begrenztes Hindernis; Bypass-Ausweiche
  + Signale.
- **Warum schwer:** der Dwell blockiert einen Block auf der dichten Strecke;
  Personenzüge müssen gestaffelt vorbei, ohne den Takt zu killen.
- **Medaillen:** Durchsatz, Pünktlichkeit.
- **Lösungsskizze:** Umfahrungsgleis um den Bahnsteig-Block; Blocksignal staffelt
  Personenzüge, während die Fracht ablädt.
- **Briefing:** „Der Güterzug lädt mitten auf der Strecke ab. Bauen Sie eine
  Umfahrung, damit der Takt nicht zusammenbricht."

### k2_07 · Rangier-Ballett *(Riesig — Fracht-Höhepunkt)*
- **Roster:** ~12 Züge: **4 Güter zu 4 verschiedenen Bahnsteigen** (verschiedene
  Ziele) + 8 Personen, 5 Ziele, mehrere Ausweichen.
- **Kern:** jeden Güterzug per `DestIs` über SEINEN Bahnsteig routen, Dwells
  staffeln, Personen dazwischenfädeln.
- **Warum schwer:** Routing + 4 gleichzeitige Dwell-Fenster + Kollisionsfreiheit;
  Blöcke müssen rechtzeitig frei werden.
- **Medaillen:** Pünktlichkeit, Durchsatz.
- **Lösungsskizze:** je Güter-Ziel ein Ast mit Bahnsteig; Ketten an den
  Sammelpunkten; dwell-bewusster Abfahrtstakt.
- **Briefing:** „Vier Güterzüge, vier Rampen, acht Personenzüge dazwischen. Jede
  Fracht an ihre Rampe — ohne dass der Betrieb erstarrt."

### k2_08 · Der Verschiebebahnhof *(Gewaltig)*
- **Roster:** ~13 Züge, **3 Klassen**, 6 Ziele, mehrere Bahnsteige + Ausweichen.
- **Kern:** gestufter Sortierbaum (**erst Klasse, dann Ziel**), Bahnsteige auf den
  Güter-Ästen, Prioritäten für Express, Ketten an den Rückführungen.
- **Warum schwer:** alles gleichzeitig auf großer Fläche — sortieren, abladen,
  pünktlich.
- **Medaillen:** alle drei eng.
- **Briefing:** „Der ganze Bahnhof auf einmal: sortieren, abladen, pünktlich sein.
  Vollauslastung."

### k2_09 · Zwei Bahnhöfe *(Gewaltig)*
- **Roster:** ~15 Züge, 2 große Terminals (je 3–4 Ziele) an den Enden,
  **mehrgleisige Hauptstrecke mit Überleitstellen (Crossovers)**, Güter-Sidings.
- **Kern:** Gegenverkehr im XXL-Maßstab; Crossovers/Sidings nur zum Überholen und
  Fracht-Ausweichen; Ketten an allen Überleitstellen.
- **Warum schwer:** massiver bidirektionaler Fluss + Fracht dazwischen, ohne
  Deadlock.
- **Medaillen:** Durchsatz + Pünktlichkeit.
- **Briefing:** „Zwei Endbahnhöfe, ununterbrochener Gegenverkehr. Beide Richtungen
  in Bewegung halten — und die Fracht dazwischen."

### k2_10 · Hauptbahnhof *(Gewaltig — Finale)*
- **Roster:** 16–18 Züge, **8 Ziele**, 3 Klassen, mehrere Bahnsteige, zentrale
  Nabe mit Radiallinien + Kreuzungen.
- **Kern:** absolut alles — Ziel+Klassen-Routing, Kreuzungen, Ketten/Block,
  Prioritäten, Fracht-Dwell, dichtester Takt.
- **Warum schwer:** ein zentraler Knoten, an dem sich alles trifft; jeder Fehler
  kaskadiert in Deadlock/Verspätung. Echte Trade-offs (Durchsatz gegen Material,
  Pünktlichkeit gegen Umwege).
- **Medaillen:** alle drei, sehr eng.
- **Briefing:** „Der Hauptbahnhof. Alles auf einmal — jeden Zug ans Ziel,
  pünktlich, ohne einen einzigen Stillstand."

---

## Authoring-/Umsetzungsnotizen

- Pro Level: **Level-RON** + **mind. eine Designer-Lösung** (`solutions/`), dann
  **`due_suggest --write` → `par_suggest --write`** blessen (Dwell fließt
  automatisch ein, da die Tools die echte Sim laufen lassen).
- **i18n je Level:** `level.<id>.name`, `.briefing`, `station.<label>` (alle
  benannten Quellen/Ziele), `hint.<id>` (Ersthilfe — v. a. bei 4/6/7/8/10).
- **Solvability-Risiko (Gewaltig):** eine garantiert lösbare Handlösung für 15–18
  Züge ist heikel. Vorschlag: diese drei **im Sandbox-Editor bauen + real
  durchsimulieren** (Start bis „Success"), notfalls Layout iterieren, dann
  blessen. Bei Bedarf Geometrie per kleinem Generator-Check absichern (wie beim
  Sortierwerk-Level k4_01).

## Offene Punkte (vor dem Bauen zu klären)

1. **Kapitelname:** `chapter.2.name` ist aktuell „Ausweichen & Regeln" — passt zu
   Fracht/Kreuzungen nicht mehr. Umbenennen (z. B. „Volllast" / „Das Netz" /
   „Betrieb")? Oder Name lassen?
2. **Express-Klasse (2):** OK, sie als wiederkehrendes Kapitel-2-Element mit
   Priorität + engem `due` einzuführen?
3. **Reihenfolge der Gewaltig-Level:** aktuell 8/9/10 als Finale-Trio. Lieber eins
   als Mid-Boss (z. B. Slot 5) vorziehen?
