# M4 — Implementierungsplan Launch

> Abgeleitet aus [GDD](../../GDD.md) §12, §13, §15. GDD bleibt Single Source
> of Truth.
>
> **Rolling-Wave:** Gröbster Plan der Reihe — er wird bei M4-Start mit den
> Demo-/Next-Fest-Erkenntnissen konkretisiert; Wochen-Angaben folgen dann.
>
> **Ziel (GDD §13):** Playtest-Feedback, Balancing (Par-Werte!),
> Achievements, Trailer/GIFs, Bugfixing.
> **Exit-Kriterium:** 1.0 auf Steam.
> **Zeitrahmen:** 4–8 Wochen — die Spanne ist der Feedback-Puffer: Was die
> Demo an Problemen zeigt, wird hier abgearbeitet, nicht wegerklärt.

## 1. Scope

**In M4:**
- Feedback-Auswertung: Demo-Telemetrie (lokal, opt-in — GDD §15: Datei, die
  Spieler freiwillig schicken; kein Server) + Next-Fest-Rückmeldungen +
  gezielte Playtests der späten Kapitel
- Balancing-Endpass: Par-Werte über Headless-Sweeps (GDD §12.1) +
  Telemetrie; Schwierigkeitskurve glätten; Problemlevel umbauen
- `steamworks-rs` hinter `steam`-Feature (GDD §12.2): Init, ~20
  Achievements, Cloud-Saves; Overlay-Verträglichkeit testen
- Launch-Trailer (aus der GIF-Pipeline von M3) + Presskit
- Release-Engineering: Versionsschema, Release-Branch, reproduzierbare
  Builds (`--release --no-default-features --features steam`),
  Save-/Code-Migrationstests, Crash-Verhalten (Panic-Hook → Logdatei +
  freundlicher Dialog), Day-1-Patch-Prozess
- QA: kompletter Kampagnen-Durchlauf je Release-Kandidat, Performance,
  frische-Maschine-Test ohne Dev-Umgebung
- v1.x-Triage: GDD-§16-Liste priorisieren (Histogramme? Tages-Challenge?) —
  Launch ist nicht das Ende der Roadmap, der Plan dafür entsteht hier

**Nicht in M4:** neue Features, neue Bausteine, neue Modi — Feature-Freeze
ab M4-Start; alles Neue ist v1.x (GDD §15: Scope-Creep-Schutz).

## 2. Schlüsselentscheidungen

| Thema | Entscheidung |
|---|---|
| Cloud-Saves | Steam **Auto-Cloud** (Konfiguration im Partner-Portal, null Code) statt Cloud-API — unsere Saves sind kleine RON-Dateien in einem Verzeichnis; genau der Auto-Cloud-Fall. |
| Achievements | Datengetrieben aus Fortschritt/Score ableiten (Kapitel geschafft, erste Goldene Anlage, alle Medaillen eines Kapitels, „Deadlock in unter 10 s debuggt"-Charakterstücke …); Liste wird als Tabelle in diesem Plan gepflegt, Strings EN/DE über die i18n-Tabellen. |
| Telemetrie-Format | Eine RON-Datei pro Spielstand: pro Level Versuche/Fehlschlag-Typen/Lösungszeit/Scores. Opt-in-Schalter im Menü, Inhalt im UI einsehbar (Vertrauen), Versand manuell durch den Spieler. |
| Plattformen zum Launch | Windows sicher; Linux nur, wenn die seit M0 grüne Linux-CI durch einen echten Spieltest bestätigt wird (GDD-Kopfdaten: „nach Determinismus-Prüfung") — sonst kurz nach Launch. macOS v1.x. |
| Preis | 10–13 € (GDD): finale Festlegung nach Next-Fest-Wishlist-Stand; Launch-Rabatt 10 %. |
| Steam Deck | Nicht angestrebt (GDD §9: maus-spielbar, kein Controller); Kategorie-Eintrag ehrlich setzen. |

## 3. Wochenplan (Kern; Puffer dahinter)

| Woche | Liefert |
|---|---|
| **W1** | Feedback-Triage (eine Liste, drei Eimer: Launch-Blocker / Balancing / v1.x); Telemetrie-Auswertung; Balancing-Sweep-Harness erweitert |
| **W2** | Balancing-Umsetzungen + Problemlevel-Umbauten; `steam`-Feature: Init + Achievements + Auto-Cloud-Konfig |
| **W3** | Trailer-Schnitt + Presskit; Release-Engineering (Branch, Builds, Migrationstests, Panic-Hook) |
| **W4** | QA-Durchläufe, Release-Kandidat, Store-Finalisierung, Launch-Checkliste; **Launch** |
| **W5–8** | Puffer: weitere Feedback-Runden, zweiter RC, Launch-Timing (z. B. auf Festival-Nachlauf) — ungenutzter Puffer = früherer Launch |

## 4. Launch-Checkliste (wird hier abgehakt)

- [ ] Alle Launch-Blocker zu; bekannte Bugs dokumentiert und bewusst vertagt
- [ ] Kompletter Kampagnen-Durchlauf auf dem Release-Build (nicht dev!)
- [ ] Save-Migration von jeder je veröffentlichten Version (Demo!) getestet
- [ ] Goldcodes aus M2 dekodieren noch (Sharing-Kompatibilität)
- [ ] Achievements feuern; Cloud-Sync zwischen zwei Rechnern geprüft
- [ ] Build ohne `dev`-Feature, ohne Inspector, ohne file_watcher (GDD-Release-Disziplin)
- [ ] Presskit + Trailer live; Launch-Post-Texte (Steam/Reddit/Discord) vorbereitet
- [ ] Day-1-Patch-Prozess einmal trocken geübt (Hotfix-Branch → Build → Depot)

## 5. Risiken

| Risiko | Plan |
|---|---|
| Demo-Feedback verlangt Strukturelles | Dafür ist die 8-Wochen-Spanne da; echte Designbrüche gehen zurück ins GDD und verschieben den Launch bewusst statt heimlich |
| Balancing nach Gefühl statt Daten | Nur Telemetrie-/Sweep-begründete Änderungen; jede Par-Änderung erneuert die CI-Lösungen (M2-Pipeline erzwingt das) |
| Steamworks-Integration spät und zickig | Früh in W2, hinter Feature-Flag; Builds laufen jederzeit auch ohne (GDD §12.2) |
| Launch-Termin-Fixierung | Termin erst nach RC1 öffentlich machen; vorher nur „Coming Soon" |
| Nach-Launch-Loch | v1.x-Triage ist M4-Deliverable: die ersten zwei Post-Launch-Patches sind vor dem Launch grob geplant |

## 6. Definition of Done (M4)

- [ ] 1.0 live auf Steam (Windows; Linux je nach §2-Entscheid)
- [ ] Launch-Checkliste vollständig abgehakt
- [ ] v1.x-Plan (priorisierte §16-Liste + Patch-1-Inhalt) liegt als `plans/v1x-backlog.md`
- [ ] GDD-Abschluss-Abgleich: Kopfdaten (Preis, Plattformen, Untertitel) final
