# Audio-Assets

Geladen in `src/audio/assets.rs`. Solange eine Datei fehlt, bleibt das Spiel
stumm — kein Crash (`Option<Res<AudioAssets>>`-Guards + kira ignoriert leere
Handles).

## Musik

| Datei | Verwendung |
|---|---|
| `music/menu_music.ogg` | Loop in Hauptmenü + Streckenwahl |
| `music/*.ogg` (Pool, siehe unten) | Zufalls-Playlist am Pult (Edit + Run) |

**Pult-Playlist:** Statt eines einzelnen Loops spielt am Pult eine zufällige
Playlist über den `level_tracks`-Pool (`src/audio/assets.rs`, Konstante
`LEVEL_TRACKS`): je ein Track, danach eine zufällige Stille-Pause
(`GAP_SECS` in `src/audio/music.rs`, aktuell 20–30 s), dann der nächste — nie
zweimal derselbe direkt hintereinander. Die Playlist läuft über Edit<->Run
durch und setzt erst beim Rückkehr ins Menü zurück. Pool erweitern = Dateinamen
in `LEVEL_TRACKS` ergänzen.

## SFX

| Datei | Format | Verwendung |
|---|---|---|
| `sfx/button_click.wav` | WAV | jeder UI-Knopfdruck |
| `sfx/switch.wav` | WAV | Weiche aufs Brett gesetzt |
| `sfx/signal.wav` | WAV | Zug hält am Signal (Relais-Klack) |
| `sfx/rail.wav` | WAV | Zug fährt in die Welt ein |

**Formate:** OGG für Musik, WAV für SFX (null Decode-Latenz). Decoder über die
kira-Features `ogg` (default) + `wav` (`Cargo.toml`). MP3 bewusst aus — sein
Encoder-Padding reißt Loops auf; die Playlist-Tracks sind darum als OGG
abgelegt.

Platzhalter durch echte Dateien ersetzen — Lautstärke-/Mix-Feinschliff ist der
M3-Audio-Pass (GDD §11).
