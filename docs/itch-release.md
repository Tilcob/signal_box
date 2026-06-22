# Builds auf itch.io packen

Für itch lädst du **drei getrennte Builds** als drei Uploads (Kanäle) auf
*eine* Projektseite hoch — Windows, Linux, Web. Jede Plattform hat eigene
Befehle.

> **Ship-Flag:** Native Builds immer mit `--no-default-features` bauen — das
> wirft die dev-Tools (egui-Inspector, FPS-Overlay) raus.

## Web (wasm)

```powershell
trunk build --release
```

→ erzeugt `dist/`. **Den ganzen `dist/`-Ordner zippen** und hochladen; bei itch
„This file will be played in the browser" anhaken. `dist/` ist self-contained:
Font, i18n und Levelkatalog sind in die wasm-Binary eingebettet, die Audio-
Assets kopiert trunk hinein.

Voraussetzung einmalig: `cargo install trunk` (zieht wasm-bindgen + wasm-opt
selbst nach).

## Windows (nativ)

```powershell
cargo build --release --no-default-features
```

→ `target/release/signal_box.exe`. **Zippen: `signal_box.exe` + den kompletten
`assets/`-Ordner daneben.** → als Windows-Download hochladen.

## Linux (nativ)

Muss **auf Linux** gebaut werden — Cross-Compile von Windows aus ist Schmerz.
Praktisch über **WSL** (Ubuntu) oder CI:

```bash
# einmalig die Bevy-Systemlibs:
sudo apt install libasound2-dev libudev-dev pkg-config

cargo build --release --no-default-features
```

→ `target/release/signal_box` (ohne `.exe`). **Zippen: `signal_box` +
`assets/`.** → als Linux-Download hochladen.

## Der eine Stolperstein: `assets/`

Die **nativen** Builds (Win/Linux) lesen Level, i18n, Font *und* Audio zur
Laufzeit aus `assets/` — der Ordner **muss** neben der Binary in der ZIP
liegen, sonst startet das Spiel ohne Inhalte. Nur die Web-Version braucht das
nicht (dort steckt alles in `dist/`).

## Effizienter: butler (itchs CLI)

**butler** ist itch.ios offizielles Kommandozeilen-Tool zum Hochladen von
Builds — kein eigenes „Web-Butler", sondern einfach `butler`. Das `:html` unten
ist nur der *Kanal*-Name, der itch sagt „das ist die Browser-Version".

Warum statt Drag-and-drop im Web-Dashboard:

- **Diff-Upload:** lädt nur die *geänderten* Bytes hoch, nicht jedes Mal die
  ganze Binary — bei häufigen Updates massiv schneller.
- **Auto-Updates:** mit butler gepushte Builds kann die itch-Desktop-App
  automatisch aktualisieren.
- **Scriptbar:** ein Befehl pro Plattform.

Der Web-Uploader (Drag-and-drop) tut es für die *erste* Demo genauso — butler
lohnt sich, sobald du regelmäßig Updates schiebst.

**Einrichten (Windows):**

1. `butler.exe` von <https://itch.io/docs/butler/> laden, in einen Ordner im
   `PATH` legen.
2. Einmal einloggen: `butler login` (öffnet den Browser zur Bestätigung).
3. Pushen:

```powershell
butler push dist          DEINNAME/stellwerk:html
butler push windows-zip/  DEINNAME/stellwerk:windows
butler push linux-zip/    DEINNAME/stellwerk:linux
```

`DEINNAME/stellwerk` = dein itch-Projekt, `:html`/`:windows`/`:linux` = der
jeweilige Kanal.

## Vor dem Packen

1. **Erst bauen lassen, dann packen:** `cargo check` (Desktop) und
   `trunk build --release` (Web) müssen grün durchlaufen, damit nichts
   halbfertig in die ZIP wandert.
2. **Fehlende Sound-Dateien** (`rail.wav`, `success.wav`, `crash.wav`,
   `deadlock.wav`) erzeugen auf Web harmlose Konsolen-Fehler — fürs saubere
   Web-Release vorher die WAVs nach `assets/audio/sfx/` legen (oder ignorieren,
   das Spiel läuft trotzdem).
3. **Web-Audio startet erst nach dem ersten Klick** — Browser-Autoplay-Policy,
   kein Bug. Gilt für jedes Web-Spiel.

## Für eine Early Demo

**Windows + Web reicht.** Linux kannst du nachreichen, sobald du WSL oder CI
startklar hast — es ist der aufwändigste Build (eigene Linux-Umgebung +
Systemlibs nötig).
