# M2 Content-Log

Tempo-Messung pro Level (Exit-Kriterium: < 1 Tag/Level). „Aufwand" = reine
Bau-+Beweiszeit über die Pipeline (Level-RON + Lösungs-RON + Par-Bless).

| Datum | Level | Aufwand | Notizen |
|---|---|---|---|
| 2026-06-12 | k1_01–k3_02 (8 Stück) | je < 1 h | M1-Bestand, nachträglich mit Lösungen + Par-Beweis gehärtet; 1 Lösungsfehler (k2_02 fehlendes Stück) vom Harness gefunden |
| 2026-06-12 | k1_05_schnellzuege | < 1 h | Harness-Fund: Tempodelta 240 reißt im Quellblock auf — Lehre: Spawn-Lücke ist 500 LE, max. sinnvolles Delta begrenzen |
| 2026-06-12 | k2_03_doppelter_gegenzug | < 1 h | Harness-Fund: Wellen müssen Ankunft der Vorwelle abwarten (Quelle = Senke der Gegenrichtung) |
| 2026-06-12 | k2_04_ueberholung | ~1,5 h | Überhol-Mechanik braucht 3 Block-Schnitte (Quelle/Schleifen-Ein-/Ausfahrt); Güterzug auf 1000 LE gekürzt |
| 2026-06-12 | k3_03_kreuzungstakt | < 0,5 h | Variation von k3_01, gleiche Lösung |
| 2026-06-12 | k4_01_sortierwerk | ~1 h | Klassenregel + Quellsignal + Ast-Schnitt |
| 2026-06-12 | k4_02_drei_ziele | < 1 h | Weichen-Kaskade, zwei Zielregeln |
| 2026-06-12 | k4_03_mischverkehr | < 0,5 h | k3_01-Topologie mit Klassen/Tempi, gleiche Lösung |

**Stand: 15 Level** (K1: 5, K2: 4, K3: 3, K4: 3) — alle mit CI-bewiesenen
Pars. Ziel laut Plan: 30+ (K1–K4 je 8). **Offen: ~17 Level** — die Pipeline
liefert nachweislich < 1 Tag/Level (gemessen deutlich darunter); der Rest
ist Fleißarbeit über denselben Weg: Lösung bauen → Level definieren →
`cargo test --test par_proof -- --nocapture` → Par eintragen.

Wiederkehrende Design-Regeln (aus Harness-Funden destilliert):
1. Spawn-Lücke = 500 LE: Folgezug-Tempodelta klein halten oder Abfahrt
   spät genug legen, sonst Auffahrunfall vor dem ersten möglichen Signal.
2. Quelle, die zugleich Senke der Gegenrichtung ist: Wellen takten oder
   per Signalblock vor der Einfahrt schützen.
3. Schleifen brauchen Schnitte an BEIDEN Enden, sonst verschmilzt ihr Block
   mit der Hauptstrecke (Ring-Regel aus M0 §9).
