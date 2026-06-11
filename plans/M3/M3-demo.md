# M3 — Implementierungsplan Demo & Steam-Page

> Abgeleitet aus [GDD](../../GDD.md) §8, §10, §11, §13. GDD bleibt Single
> Source of Truth.
>
> **Rolling-Wave:** Wird bei M3-Start geschärft; Wochen-Angaben folgen dann.
>
> **Ziel (GDD §13):** Kapitel 5–6, Polish-Pass (Audio, Juice, Onboarding),
> Demo-Build (Kapitel 1–3 — der Deadlock-USP muss erlebbar sein), Steam-Page.
> **Exit-Kriterium:** Demo veröffentlicht; Next-Fest-Anmeldung raus.
> **Zeitrahmen:** 4 Wochen. **Harte externe Termine:** Next-Fest-Fristen und
> Steams Page-Review-Dauer — **beides schon während M2 nachschlagen und
> rückwärts terminieren**; dieser Plan nimmt an, dass die Fristen passen.

## 1. Scope

**In M3:**
- Content: Kapitel 5 „Gebirge" (~6 Level: Material-Limits als Puzzle-Twist,
  lange eingleisige Abschnitte) und Kapitel 6 „S-Bahn-Takt" (~6 Level:
  gemischte Zugtypen, enge Soll-Zeiten, Pünktlichkeit als Hauptachse)
- Audio v1 (GDD §11): `bevy_kira_audio`-Integration, `SimEvent`s → Sounds
  (Relais-Klack, Fahrstraßen-Klack, Zuglauf-Ticken, Pult-Brummen),
  Erfolgs-/Crash-/Deadlock-Stinger, Edit-Ambient, Lautstärke-Optionen
- Onboarding-Pass: Betriebsauftrags-Panels, kontextuelle Ersthilfen in
  Kapitel 1 (einmalige Hinweise, keine Tutorial-Zwangsführung), bessere
  Fehlschlag-Erklärtexte aus M1-Playtest-Erkenntnissen
- Juice-Pass: Moduswechsel-Übergang, Medaillen-Reveal, Glow-Puls bei
  Fahrstraßenbildung — klein halten, Liste vorab fixieren
- Barrierefreiheits-Check (GDD §9): Deuteranopie-Simulation über alle
  Zustände; UI-Skalierung minimal prüfen
- Demo-Build: Kapitel 1–3, technisch ein `demo`-Feature des Hauptspiels;
  Fortschritt der Demo bleibt mit der Vollversion kompatibel (gleiche
  Save-Struktur — Demo-Spieler verlieren beim Kauf nichts)
- Steam: Hauptspiel-Page + Demo-Page (Capsule im Pult-Look, 5+ Screenshots,
  GIF-Trailer v1), Upload via SteamPipe, Next-Fest-Anmeldung
- Performance-Pass: große Anlage (Sandbox-Maximum) bei 16× flüssig

**Nicht in M3:** `steamworks-rs`-Code-Integration (Achievements/Cloud = M4;
Demo-Upload braucht nur das Steamworks-Webpanel + SteamPipe), Trailer in
Endqualität (Launch-Trailer = M4), Balancing-Endpass (M4, mit Telemetrie).

## 2. Schlüsselentscheidungen

| Thema | Entscheidung |
|---|---|
| Sounddesign extern | Budget 1–2 k€ (GDD §11). **Beauftragung spätestens in M2-Woche 5** (Vorlauf!); M3 integriert. Fallback: kuratierte Bibliothekssounds, externer Pass dann in M4. |
| Demo-Gating | Cargo-Feature `demo` + Level-Manifest-Filter; ein Repo, ein Code-Stand, zwei Steam-Depots. Sharing-Codes in der Demo: Import nur für K1–3-Level, Export erlaubt (Marketing!). |
| GIF-Pipeline | Feste Kamerafahrten + Aufnahmemodus hinter `dev`-Feature (sauber croppen statt OBS-Gefummel) — die GIFs sind das Marketing-Werkzeug bis Launch (GDD §2). |
| Seiten-Texte | EN primär, DE-Seite mit; Untertitel „a railway signaling puzzle" finalisieren (GDD §16 → Entscheidung fällt hier). |

## 3. Wochenplan

| Woche | Liefert |
|---|---|
| **W1** | Kapitel-5-Level (Material-Limit-Twist), Audio-Integration beginnt (Events → erste Sounds) |
| **W2** | Kapitel-6-Level (Takt), Onboarding-Pass, Audio komplett verdrahtet |
| **W3** | Juice-Pass (fixe Liste), Barrierefreiheits-Check, Performance-Pass, Demo-Feature + Demo-Smoketest auf frischem Rechner |
| **W4** | Steam-Pages (Capsule, Screenshots, GIFs, Texte EN/DE), SteamPipe-Upload, Review-Schleife, Next-Fest-Anmeldung, Demo live |

## 4. Risiken

| Risiko | Plan |
|---|---|
| Next-Fest-Frist verpasst | Termine in M2 verifiziert; notfalls Demo trotzdem launchen und nächstes Fest nehmen — Wishlist-Aufbau zählt, nicht das eine Event |
| Steam-Review verzögert Page | Page-Entwurf in W3 einreichen, nicht erst W4 |
| Audio-Extern liefert spät | Event→Sound-Schnittstelle ist von Bibliothekssounds aus testbar; Austausch ist dann nur Asset-Tausch |
| Juice/Polish frisst endlos | Fixe Liste in W3, alles weitere ins M4-Backlog |
| Demo zu großzügig (kannibalisierend) | K1–3 ist beschlossen (GDD §13); Demo endet mit Ausblick auf K4–6 + Wishlist-Hinweis |

## 5. Definition of Done (M3)

- [ ] 40+ Level (K1–6) mit CI-geprüften Pars und EN/DE-Texten
- [ ] Audio: jedes zentrale `SimEvent` hat einen Sound; Stinger unterscheidbar; Mixer-Optionen
- [ ] Farbenblind-Check dokumentiert bestanden (Screenshots der Simulation in `plans/M3/`)
- [ ] Demo-Build von fremdem Rechner ohne Dev-Umgebung spielbar
- [ ] Steam-Page + Demo live; Next-Fest-Anmeldung bestätigt
- [ ] GDD-Abgleich (u. a. Untertitel-Entscheidung §16 → Kopfdaten), Plan M4 geschärft
