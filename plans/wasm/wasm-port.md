# Wasm/Browser-Port — Implementierungsplan

> Eigener Meilenstein **nach** der Early Demo (nicht Teil von M3). Ziel: das
> Spiel im Browser spielbar machen (itch.io HTML5), ohne den Desktop-Build zu
> brechen. GDD bleibt Single Source of Truth.
>
> **Ziel:** `cargo build --target wasm32-unknown-unknown --release` grün und im
> Browser lauffähig.
> **Exit-Kriterium:** Browser-Build lädt Katalog/i18n/Font, persistiert über
> localStorage, spielt Audio nach erster Geste — auf itch als HTML5 lauffähig.
> **Aufwand (geschätzt):** 2–4 Tage.

## 0. Leitprinzip — wasm ist ein Sub-Build, Desktop bleibt „normal"

Harte Vorgabe: **der Windows/Linux-Build ändert sein Verhalten nicht.**

- `cargo build` / `cargo run` / `cargo test` bleiben der Desktop-Build, exakt
  wie heute. Wasm läuft **nur** über einen expliziten zweiten Weg
  (`--target wasm32-unknown-unknown` bzw. `trunk`).
- Jede Änderung ist **additiv und gegated**: der Desktop-Pfad ist
  `#[cfg(not(target_arch = "wasm32"))]` und enthält den **bestehenden Code
  unverändert**; der wasm-Pfad ist `#[cfg(target_arch = "wasm32")]` neu daneben.
- wasm-only Crates (`web-sys`) hängen unter
  `[target.'cfg(target_arch = "wasm32")'.dependencies]`; `arboard`/`directories`
  wandern unter `[target.'cfg(not(...))'.dependencies]` — sie landen nie im
  wasm-Baum, der Desktop-Baum bleibt identisch.
- **Einbetten passiert nur auf wasm.** Desktop liest Font/i18n/Level weiter via
  `std::fs` aus `assets/` — sonst verlöre es Hot-Reload und den Authoring-
  Workflow (Level-Edit ohne Recompile). Das ist genau der „normale" Zustand.

Damit ist wasm jederzeit abschaltbar/ignorierbar, ohne dass am Desktop etwas
kippt. Die M3-Polish-Änderungen waren bereits alle plattformneutral — der
native Release baut grün, alle Tests grün; dieser Port fasst das nicht an.

## 1. Befund — warum es kein Build-Flag ist

Der Browser hat kein Dateisystem; das Spiel liest aber Level, i18n, Font und
Saves direkt über `std::fs` + nutzt zwei desktop-only Crates. Verifiziert per
`cargo check --target wasm32-unknown-unknown` (stirbt schon an `arboard`).

**Ship-Code-Blocker** (dev-Tools wie `authoring.rs` sind `#[cfg(feature="dev")]`
→ irrelevant fürs Browser-Artefakt):

| Stelle | Bruch | Art |
|---|---|---|
| `clipboard.rs` (`arboard`) | kein wasm-`platform`-Impl | **Compile** |
| `levels.rs:132` (`directories`) | kein Configdir im Browser | **Compile** (sehr wahrsch.) |
| `font.rs:38` `std::fs::read` | Font-Datei | Laufzeit → Fallback auf Tofu-Font (keine Umlaute) |
| `i18n.rs:20` `std::fs::read_to_string` | i18n-Tabellen | Laufzeit → keine Texte |
| `levels.rs:335` `read_dir` | Level-Katalog | Laufzeit → **keine Level** |
| `levels.rs` Progress/Sandbox `std::fs` | Saves | Laufzeit → kein Speichern |
| `clipboard.rs` `std::fs`-Fallback | Datei-Fallback | Laufzeit |

**Kein Problem:** `bevy_kira_audio` (Web-Audio-Support), `stellwerk_sim`
(reine Integer-Arithmetik → **wasm bleibt bit-identisch**; Sharing-Codes
Desktop↔Browser kompatibel, kein Float-Determinismus-Risiko), Bevy selbst
(webgl2 default).

## 2. Strategie

Zwei Hebel, beide klein und beide **cfg-gegated** (§0):
1. **Read-only-Assets auf wasm einbetten** (Font, i18n, Level): der wasm-Pfad
   liest aus `include_*`, der **Desktop-Pfad bleibt `std::fs`** (Hot-Reload +
   Authoring unberührt). Ein dünner Wrapper pro Asset mit zwei `cfg`-Armen.
2. **Plattform-Split** für die zwei OS-Crates und die Saves: Desktop wie bisher,
   wasm über `web-sys` (localStorage, Clipboard) — target-spezifische
   Dependencies, der Desktop-Baum bleibt identisch.

Kein Async-Umbau am Asset-Server nötig — Einbetten ist der faulere Weg als
asynchrones `AssetServer`-Laden und vermeidet ein Manifest. Audio bleibt über
`AssetServer` (trunk liefert `assets/audio/` aus; Desktop unverändert).

## 3. Phasen

### Phase 1 — kompiliert auf wasm (Compile-Blocker raus)
- **`arboard`** auf target-spezifisch umstellen:
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies] arboard = "3"`.
  `clipboard.rs` per `cfg` splitten (Phase 4 füllt den wasm-Pfad).
- **`directories`** gleich behandeln; `config_dir()` cfg-splitten.
- Wahrscheinlich **`getrandom` mit `js`-Feature** nötig (transitiver
  wasm-Klassiker): `[target.wasm…] getrandom = { version = "…", features = ["js"] }`.
- Zwischenziel: `cargo build --target wasm32-unknown-unknown` linkt durch.

### Phase 2 — Read-only-Assets auf wasm einbetten (Desktop unverändert)
Jeweils ein `cfg`-Split: Desktop-Arm = **bestehender `std::fs`-Code 1:1**,
wasm-Arm = eingebettet.
- **Font:** wasm-Arm `include_bytes!(PATH)`, Desktop-Arm bleibt
  `std::fs::read` (`font.rs:38`). Glyph-Test (nativ) bleibt grün.
- **i18n:** wasm-Arm `include_str!` der zwei Tabellen, `set_lang` parst daraus;
  Desktop-Arm bleibt der fs-Read (`i18n.rs:20`), inkl. Hot-Reload.
- **Level-Katalog:** wasm-Arm liest aus eingebetteten Dateien, Desktop-Arm
  behält `read_dir` (`levels.rs:335`) — Authoring-Workflow unangetastet.
  Einbettungsweg für den wasm-Arm: `include_dir` (1 wasm-only Dep) **oder**
  `build.rs`-Manifest (dep-frei) — §5. Solutions bleiben fs/dev-only.

### Phase 3 — Persistenz im Browser
- Kleine Storage-Abstraktion: zwei Funktionen `load_string(key)` /
  `save_string(key, val)`.
  - **native:** wie bisher (`directories`-Pfad + `std::fs`), inkl. der
    `.bak`-Degradationslogik.
  - **wasm:** `web-sys` `window().local_storage()` (Features `Storage`,
    `Window`) — keine neue Top-Level-Dep (web-sys ist im wasm-Baum schon da).
- `parse_progress` bleibt unverändert geteilt (ist schon eine reine Funktion).
  Progress **und** Sandbox darüber. Desktop-Saves und Browser-localStorage sind
  getrennte Welten — Browser-Spieler starten frisch (ok, kein Migrationszwang).

### Phase 4 — Clipboard, Fenster, Tooling
- **Clipboard (wasm):** `web-sys` —
  Copy: `navigator.clipboard().write_text(code)` (fire-and-forget; erlaubt in
  der Klick-Geste). Paste: **`window.prompt("Code:")`** — synchron, umgeht das
  async-Clipboard-Read-Problem (die bestehende `paste()` ist synchron). Datei-
  Fallback entfällt auf wasm.
- **Fenster:** `main.rs` cfg-splitten — wasm kann kein
  `BorderlessFullscreen`; stattdessen
  `Window { canvas: Some("#bevy".into()), fit_canvas_to_parent: true, .. }`.
- **Audio-Autoplay:** Browser blockt Ton bis zur ersten User-Geste — Musik
  startet faktisch erst nach „Start" im Menü (deckt die Geste ab); ggf. einen
  „resume on first click" sicherstellen.
- **Tooling:** `cargo install trunk`; `index.html` mit `<canvas id="bevy">`;
  `trunk serve` (lokal), `trunk build --release` → `dist/` auf itch als HTML5.
  `wasm-opt` (in trunk) für Größe.

### Phase 5 — Verify & Ship
- **Desktop-Regression-Gate zuerst:** `cargo build`, `cargo run`,
  `cargo test --workspace`, `clippy -D warnings` und der native Release laufen
  unverändert grün — kein Verhaltensunterschied zu heute. Das ist die wichtigste
  Prüfung dieses Ports (§0).
- `cargo build --target wasm32-unknown-unknown --release` grün.
- `trunk serve`: Katalog lädt, Umlaute korrekt (eingebetteter Font), Audio nach
  Klick, Sprache umschaltbar.
- Reload behält Fortschritt (localStorage).
- Sharing: Export kopiert, Import via Prompt.
- **Determinismus-Spotcheck:** ein auf dem Desktop erzeugter Lösungscode läuft
  im Browser identisch durch (Integer-Sim-Versprechen).
- `trunk build --release` → itch HTML5-Upload.

## 4. Risiken

| Risiko | Plan |
|---|---|
| wasm-Binärgröße (Bevy ist groß, ~15–25 MB) | `wasm-opt -Oz` via trunk; itch-Ladezeit akzeptabel; nicht weiter optimieren bis es stört |
| `getrandom`/`fastrand`-Seeding auf wasm (Musik-Shuffle) | in Phase 1 verifizieren; notfalls RNG explizit seeden |
| Audio-Autoplay-Block | Musikstart an die erste Geste hängen |
| `directories` doch nur Laufzeit statt Compile | egal — wird so oder so target-gegated |
| Level-Edit braucht Recompile (Einbetten) | für eine Demo ok (Kampagne ist fix); Sandbox bleibt zur Laufzeit editierbar (localStorage) |

## 5. Offene Entscheidungen

| Thema | Entscheidung |
|---|---|
| Browser als offizielles Ziel? | **Nein — experimenteller Sub-Build** (entschieden 2026-06-22). GDD §1/§13 bleiben unberührt; wird erst zum Thema, wenn der Browser-Build wirklich released wird. |
| Level-Einbettung (wasm-Arm) | `include_dir` (1 wasm-only Dep, schnell) vs. `build.rs`-Manifest (dep-frei, §12.4-konform). **Default: `include_dir`**, wasm-gegated. |
| Sharing-Paste auf wasm | `window.prompt` (lazy, sofort) vs. In-Game-Eingabefeld (schöner). **Default: `window.prompt`.** |

## 6. Dependency-Politik (GDD §12.4)

Neue/erweiterte Deps müssen vor `Cargo.toml` in GDD §12.2 begründet werden:
`web-sys` (wasm-only, Features Storage/Window/Clipboard/Navigator — schon
transitiv da), `include_dir` (wasm-only). `getrandom`-`js`-Feature stellte sich
als **nicht nötig** heraus (wasm kompiliert ohne). Alle wasm-gegated → kein
Desktop-Risiko.

## 7. Umsetzungsstand (2026-06-22)

Phasen 1–4 im Code eingebaut, **alles `cfg`/target-gegated** (§0).

**Desktop unverändert (Regression-Gate bestanden):** `cargo build`/`check`,
`clippy -D warnings`, `cargo test --workspace` — alle grün wie zuvor.

**wasm kompiliert:** `cargo check --target wasm32-unknown-unknown` grün
(getrandom war kein Problem). Konkret umgesetzt:
- **clipboard.rs:** Desktop `arboard`+Datei (1:1), wasm `web-sys` Clipboard-Write
  + `window.prompt` fürs Import.
- **levels.rs:** Persistenz cfg-split — Desktop fs+`directories`, wasm
  localStorage; `load_catalog` Desktop `read_dir`, wasm `include_dir`.
- **font.rs / i18n.rs:** Desktop fs (+ Hot-Reload), wasm `include_bytes!` /
  `include_str!`.
- **main.rs:** Fenster cfg-split (Desktop fullscreen/windowed, wasm Canvas
  `#bevy` + `fit_canvas_to_parent`).
- **Cargo.toml:** `arboard`/`directories` unter `cfg(not(wasm))`, `web-sys` +
  `include_dir` unter `cfg(wasm)`.
- **index.html** für trunk (Canvas, `assets/`-Copy, `--no-default-features`).

**Manuell offen (Browser-Runtime — hier nicht testbar):**
- `cargo install trunk`; `trunk serve` (lokal testen) / `trunk build --release`
  → `dist/` auf itch. **Falls** trunk an `getrandom` linkt-meckert: in
  `.cargo/config.toml` `rustflags = ['--cfg','getrandom_backend="wasm_js"']`
  fürs wasm-Target setzen (Build-time hier nicht aufgetreten).
- Im Browser prüfen: Katalog/Umlaute/Audio-nach-Klick, localStorage-Persistenz
  über Reload, Sharing Copy/Paste.
- wasm-bindgen-CLI-Version ggf. an die Crate-Version angleichen (trunk meldet's).
