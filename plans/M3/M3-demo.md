# M3 — Implementierungsplan Itch Early Demo (Polish + Ship)

> Abgeleitet aus [GDD](../../GDD.md) §9, §11, §13. GDD bleibt Single Source
> of Truth.
>
> **Re-Scope (2026-06-22):** Erster Schritt von M3 ist eine **Early Demo auf
> itch.io** zum Feedback-Sammeln — NICHT der volle Steam-Next-Fest-Auftritt.
> Damit fällt der teuerste, fristgebundene Teil von M3 (Steam-Page,
> SteamPipe, Next-Fest, Capsule, GIF-Trailer) raus und wird in den späteren
> **Steam-Push (M3-spät/M4)** geparkt (siehe §6). Das GDD §13 beschreibt
> weiterhin den vollen Steam-M3 — dafür gehört ein Historie-Eintrag ins GDD,
> dass die Itch-Demo dem vorgeschaltet ist.
>
> **Ziel:** ~20 Level (separater Content-Track, *nicht* dieser Plan) + ein
> polierter Loop, lauffähig als ZIP auf itch. Der Deadlock-USP muss erlebbar
> sein.
>
> **Exit-Kriterium:** Demo-Build von fremdem Rechner ohne Dev-Umgebung
> spielbar und auf itch.io hochgeladen.

## 1. Scope

**Dieser Plan deckt ab: Polish + Ship — alles außer den Leveln.**

- **Audio v1 fertigstellen** (Outcome-Stinger + Lautstärke-Optionen). Kern
  ist bereits gebaut (`src/audio/`).
- **Juice-Pass** (fixe Liste: Moduswechsel-Übergang, Medaillen-Reveal,
  Fahrstraßen-Glow-Puls).
- **Onboarding-System** (einmalige Kontext-Ersthilfen; optional
  Deadlock-Zyklus auf dem Board).
- **Barrierefreiheits-Pass** (Deuteranopie-Check + minimale Form/Muster-
  Differenzierer, GDD §9).
- **Ship** (Release-Build, Smoke-Test, ZIP, Itch-Page).

**Parallel, aber NICHT dieser Plan:** Content 8 → ~20 Level über die
bestehende Pipeline (`due_suggest --write` → `par_suggest --write`,
`par_proof` grün). Darin 2–3 gezielte Deadlock-/Kettensignal-Lehrstücke.

**Nicht in dieser Demo:** Steam (Page, SteamPipe, Next-Fest, Capsule,
Trailer), `demo`-Cargo-Feature, Kapitel-5/6-Inhalte, Balancing-Endpass.
Siehe §6.

## 2. Realstand-Befund (Code gelesen 2026-06-22)

Der frühere Skelett-Plan nahm „Audio = To-do, Content K1–4 fertig" an.
Beides stimmt nicht mehr:

- **Content ist eingestürzt, nicht fertig.** Commit `cb8e2a9` hat den
  gesamten M2-Content (K1–K4) gelöscht; Neuaufbau läuft, langsamer und in
  einer breiten K1 (`assets/levels/k1_01`–`k1_08`, alle `chapter: 1`). Der
  USP (Kreuzungen/Kettensignale) taucht ab `k1_08` auf. → Content-Track muss
  bis ~20 und bis zum gelehrten Deadlock laufen.
- **Audio v1 ist schon weitgehend gebaut** (`src/audio/` komplett:
  kira-Backend, Musik-Playlist, SFX-Observer). → kein Rebuild, nur die zwei
  Lücken in §3-A.
- **Kapitelstruktur ↔ GDD §8.1 offen:** GDD sagt 6 Kapitel, der Tree sagt
  eine breite K1. Für die *Itch*-Demo bewusst NICHT formalisieren — das ist
  eine Entscheidung für den Steam-M3.

## 3. Arbeitspakete

Jeweils: Befund (was der Code zeigt) → Plan → bewusster Cut.

### A. Audio — zwei Lücken, sonst fertig

**Befund:** SFX-Mapping in `src/run.rs` (`tick`): `TrainSpawned→Rail`,
`SignalBlocked→Signal`, `TrainArrived→TrainHorn`. `SimEvent::RunEnded`
löst **keinen** Sound aus → keine Erfolg-/Crash-/Deadlock-Stinger. Kanäle
(`MusicChannel`/`SfxChannel`) existieren, aber **kein `set_volume`, keine
Optionen-UI, keine Persistenz** → DoD „Mixer-Optionen" offen.

**Plan:**
1. **Outcome-Stinger:** neue `SfxKind` Success/Crash/Deadlock (Misrouting +
   Stalled teilen sich „Crash"). Trigger in `run.rs::finish()` per
   `Outcome`-match (der Branch existiert dort). 3 Bibliotheks-WAVs nach
   `assets/audio/sfx/`, in `src/audio/assets.rs` laden.
2. **Lautstärke:** Music- + SFX-Volume in `Progress` persistieren (wie
   `lang`, `src/levels.rs`), bei Änderung `channel.set_volume()`. UI als zwei
   +/−-Reihen im Pause-Menü (`src/ui/pause.rs`) + Hauptmenü.

**Cut:** kein Mixer-Screen, keine Pro-Sound-Regler. Pult-Brummen-Ambient +
Fahrstraßen-Klack sind nice, nicht demo-kritisch → defer.

### B. Barrierefreiheit — echte Arbeit, kein Screenshot-Pass

**Befund:** `src/board/palette.rs` ist rein farbcodiert; `src/board/draw.rs`
malt Volltonflächen, der Run-Board recoloriert nur die Farbe. Besetzt (rot) /
reserviert (gelb) / frei und Signal grün/rot unterscheiden sich **nur über
Farbe**. Signale haben Form (Block = Quadrat, Kette = Raute), aber grün vs.
rot ist formgleich. → **GDD §9 („Farbe nie alleiniger Träger") ist aktuell
nicht erfüllt.**

**Plan (demo-tauglich):**
1. Deuteranopie-Check fahren, Screenshots → `plans/M3/`.
2. Billigste High-Impact-Differenzierer: Signal-rot bekommt eine Form-
   Markierung (Stopp-Balken), besetzt vs. reserviert ein Streifen-Sprite.
3. Volles §9-System (wandernde Füllung besetzt, Schraffur reserviert) →
   **Steam-M3, nicht Early Demo.**

**Entschieden (§5): minimal** (Schritte 1–2).

### C. Juice — Hooks bestätigt, klein halten

**Befund/Plan:** drei isolierte Effekte:
1. **Moduswechsel-Übergang:** `OnEnter(Run)`/`OnEnter(Edit)` → kurzes
   Vollbild-Overlay, Alpha lerpt auf 0.
2. **Medaillen-Reveal:** `src/ui/result.rs` malt die `dot`-Medaillen sofort;
   bei Success Scale-Pop mit Versatz pro Achse.
3. **Fahrstraßen-Glow-Puls:** Helligkeits-Puls beim Bandübergang
   frei→reserviert (`col_reserved`).

**Umsetzung:** ein winziges Lerp-Helfer-Modul — GDD §12.3 erlaubt genau das.
**Kein `bevy_tweening`** (versionsgekoppelt, Bedarf zu klein). Fixe Liste,
danach Stop.

### D. Onboarding — System ja, Texte gehören zu den Leveln

**Befund:** `briefing` existiert (`src/state.rs::ActiveLevel`); Fehlschlag-
Texte in `src/ui/result.rs::describe()`. Der Deadlock wird nur als Text-Kette
gezeigt — GDD §7.6 will den Wartezyklus **auf dem Board** hervorheben. Fehlt.

**Plan (System-Teil):**
1. **Einmalige Kontext-Ersthilfen:** Set gesehener Hint-Ids in `Progress`
   persistieren; Trigger bei `OnEnter(Edit)` für frühe Level / Erst-Benutzung
   eines Tools; wegklickbar.
2. **(Wenn Zeit) Deadlock-Zyklus auf dem Board** statt nur Text — höchster
   Onboarding-Hebel, weil er den USP zeigt.

**Out (= Level):** konkrete Briefing-/Hint-Texte, Level-Namen,
Register-Umstellung. **Register entschieden (§5): Du** — `k1_01` ist schon
„Du", die Umstellung k1_02–k1_08 von „Sie" auf „Du" läuft im Content-Track.

### E. Ship

**Befund:** dev/authoring/dev_tools sind hinter `cfg(feature="dev")`
(`src/main.rs`); `SaveModalOpen` ist immer da, das Modal aber dev-only →
`--no-default-features` droppt egui sauber.

**Plan:** Release-Build `cargo build --release --no-default-features`,
Smoke-Test auf fremdem Rechner ohne Rust, ZIP, Itch-Page. **Kein
`demo`-Cargo-Feature** (YAGNI — kein bezahlter Build zum Abgrenzen).
Performance-Check Sandbox-Max @16× separat, falls überhaupt.

## 4. Reihenfolge

1. **Audio** (Stinger + Volume) — klein, isoliert, hoher gefühlter Gewinn.
2. **Juice** — Lerp-Helfer einmal bauen, dann die 3 Effekte.
3. **Onboarding-Hint-System.**
4. **Barrierefreiheit** — Check + minimale Differenzierer (Tiefe vorher
   entscheiden).
5. **Ship-Pass** zuletzt (sobald der Content-Track ~20 erreicht).

## 5. Entscheidungen

| Thema | Entscheidung |
|---|---|
| Anrede-Register | **Du** (entschieden 2026-06-22). Gilt für Briefings, Hints und UI-Texte. Nacharbeit: k1_02–k1_08 von „Sie" auf „Du" umstellen — Content-Track, nicht dieser Plan. |
| Farbenblind-Tiefe | **Minimal** (entschieden 2026-06-22): Check + High-Impact-Differenzierer. Volles §9 → Steam-M3. |
| Kapitelstruktur ↔ GDD §8.1 | **Offen** — für die Itch-Demo bewusst nicht formalisieren; reconcilen vor dem Steam-M3. |

## 6. Geparkt (Steam-Push, M3-spät/M4)

Bewusst aus der Early Demo herausgenommen — nichts davon ist verloren, nur
verschoben:

- Steam-Page (Haupt + Demo), Capsule im Pult-Look, 5+ Screenshots,
  GIF-Trailer v1, SteamPipe-Upload, Next-Fest-Anmeldung.
- `demo`-Cargo-Feature + Level-Manifest-Filter (zwei Depots, Save-Kompat) —
  erst sinnvoll, wenn ein bezahlter Steam-Build existiert.
- Volles §9-Barrierefreiheits-System (wandernde Füllung / Schraffur).
- Audio-Ambient (Pult-Brummen, Fahrstraßen-Klack im Run), externer
  Sounddesign-Pass (reiner Asset-Tausch hinter `SfxKind`).
- Performance-Endpass, Balancing mit Telemetrie.

## 7. Definition of Done (Itch Early Demo)

- [ ] ~20 Level spielbar, alle mit CI-bewiesenen Pars (Content-Track,
      separat); Deadlock-USP wird in ≥2 Leveln gelehrt
- [ ] Audio: Erfolg-/Crash-/Deadlock-Stinger hörbar unterscheidbar;
      Music- + SFX-Lautstärke einstellbar und persistiert
- [ ] Juice-Liste (Moduswechsel, Medaillen-Reveal, Fahrstraßen-Puls) drin —
      und geschlossen, kein offenes Polish-Fass
- [ ] Onboarding: einmalige Kontext-Ersthilfen in den frühen Leveln
- [ ] Deuteranopie-Check dokumentiert (Screenshots in `plans/M3/`),
      schlimmste Rot/Grün-Verwechslungen entschärft
- [ ] Release-Build `--no-default-features` auf fremdem Rechner ohne Rust
      spielbar; ZIP auf itch.io hochgeladen
- [x] GDD-Historie-Eintrag: Itch-Demo dem Steam-M3 vorgeschaltet (2026-06-22)

## 8. Umsetzungsstand (2026-06-22)

Polish-Pakete A–E eingebaut (alles außer Leveln); `cargo test --workspace`
grün, `clippy -D warnings` sauber.

**Code fertig:**
- **A Audio:** Outcome-Stinger (`SfxKind::Success/Crash/Deadlock`, getriggert in
  `run::finish`); Lautstärke (Musik/SFX) linear in `Progress`, als dB angewandt
  (`audio::apply_volume`), Regler in Pause- **und** Hauptmenü (`ui/options.rs`).
- **B Farbenblind:** Stopp-Balken erscheint an roten Signalen (`ui`-loses
  Form-Cue, `board::run_board::StopBar`). Blöcke hatten via Bandbreite schon
  einen Cue.
- **C Juice:** Moduswechsel-Fade + Medaillen-Pop (`ui/juice.rs`),
  Fahrstraßen-Glow-Puls (`run_board`, frei→reserviert). Eigene Lerp-Helfer,
  kein `bevy_tweening`.
- **D Onboarding:** Einmalige Hint-Overlays (`ui/hints.rs`); ein
  `hint.<level_id>`-i18n-String aktiviert einen Hint, einmal gezeigt → in
  `Progress::seen_hints`. **Deadlock-Zyklus-auf-Board (Plan §3-D Pkt. 2,
  „wenn Zeit") bewusst vertagt.**
- **E Ship:** `cargo check --no-default-features` grün (egui/dev sauber raus).

**Manuell offen (Content/Assets/extern — nicht Code):**
- 3 WAVs nach `assets/audio/sfx/`: `success.wav`, `crash.wav`, `deadlock.wav`
  (sonst spielt der Stinger stumm — kein Crash).
- Deuteranopie-Check fahren, Screenshots → `plans/M3/`.
- `hint.<level_id>`-Texte je Frühlevel in `de.ron`/`en.ron` (Anrede **Du**).
- Content-Track 8 → ~20 Level inkl. Du-Umstellung k1_02–k1_08.
- Release-Build packen, Smoke-Test fremder Rechner, ZIP, Itch-Page.
- Optional-Cleanup: `numeric_field::text()` ist ohne `dev`-Feature ungenutzt
  (dead-code-Warnung nur im Ship-Build).
