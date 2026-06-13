# Aufgabenblatt — Lazy-Asset-Loading + Loading/MainMenu-States

> Du implementierst selbst. Hier sind Code-Skelette mit `todo!()`-Markern,
> Reihenfolge und Begründung. Referenz-Pattern: `C:\01_Programmieren\Rust\out_of_sight`
> (nutzt `bevy_asset_loader` + `Boot`-State).

## 0. Ehrliche Einordnung — was das bringt (und was nicht)

Stellwerk hat **fast keine Assets**:

| Asset | Größe | aktuell geladen |
|---|---|---|
| `fonts/DejaVuSansMono.ttf` | 336 KB | `std::fs::read` in `font.rs` (Plugin-Build) |
| 16 × `levels/*.ron` | ~60 KB gesamt | `std::fs` in `levels.rs` (Plugin-Build) |
| `i18n/de.ron`, `en.ron` | 2 KB | `std::fs` in `i18n.rs` (Plugin-Build) |

**Das Board rendert aus farbigen Sprite-Quads, nicht aus Texturen.** Es gibt
keine Bilder, keinen Sound. Das Laden passiert einmalig beim Start (ein kurzer
Hitch, bevor das Fenster da ist) — **nicht** während des Spielens.

**Konsequenz:** Lazy-Loading hält **nicht** allein die FPS bei 60. Der echte
Per-Frame-Killer ist `draw_run_board` (siehe §7). Was dieses Aufgabenblatt
trotzdem bringt:

1. **Start-Hitch weg** — blockierende `std::fs`-Reads → asynchrones Laden
   hinter einem Loading-Screen.
2. **Sauberes Hauptmenü** statt direkt in die Streckenwahl zu fallen.
3. **Zukunftssicher für M3** — Audio (`SimEvent`→Sound) und Pult-Kapsel-Bilder
   kommen; dann ist die Collection schon da und es gibt kein Pop-in.
4. **Eine Stelle für alle Handles** — genau dein "alles auf einem".

---

## 1. Dependencies (`Cargo.toml`)

```toml
# Asynchrones, gegatetes Preloading aller Assets in EINER Collection.
bevy_asset_loader = "0.26"
```

`bevy_common_assets` (RON-Assets) brauchst du **nicht zwingend**: wir laden die
Level weiterhin synchron in den `Catalog` (sind winzig + sofort da), die
Collection gatet nur den Font. Wenn du Level später als echte
`Handle<LevelAsset>` willst → eigenes Folgeblatt.

---

## 2. `GameState` erweitern (`src/state.rs`)

```rust
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Start-State: bevy_asset_loader preloadet GameAssets, dann → MainMenu.
    #[default]
    Loading,
    /// Titelbildschirm: Start / Sandbox / Beenden.
    MainMenu,
    LevelSelect,
    Edit,
    Run,
    Result,
}
```

⚠️ **Wichtig:** `Loading` muss `#[default]` sein — die App bootet dort. Alle
bestehenden `OnEnter(GameState::LevelSelect)`-Systeme bleiben unverändert; sie
feuern jetzt erst, wenn man aus dem MainMenu kommt.

---

## 3. Loading-State + Asset-Collection (`src/loading.rs`, neu)

Analog zu `out_of_sight/src/state/boot.rs`.

```rust
//! Loading-State: einmaliges Preloading aller geteilten Assets via
//! bevy_asset_loader. Gatet Loading → MainMenu, sobald alles resident ist.

use bevy::prelude::*;
use bevy::text::Font;
use bevy_asset_loader::prelude::*;

use crate::state::GameState;

/// Preload-Manifest — die EINE Collection. Wächst in M3 um Audio/Bilder.
#[derive(AssetCollection, Resource)]
pub struct GameAssets {
    #[asset(path = "fonts/DejaVuSansMono.ttf")]
    pub font: Handle<Font>,
    // M3: #[asset(path = "audio/...")] pub klack: Handle<AudioSource>, ...
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::MainMenu)
                .load_collection::<GameAssets>(),
        )
        // Level + i18n + Progress materialisieren wir hier synchron (instant).
        // Reihenfolge egal: LoadingState gatet auf den Font, das hier ist
        // längst fertig, bevor der Font resident ist.
        .add_systems(OnEnter(GameState::Loading), todo!("setup_catalog_and_i18n"));
    }
}

/// Zieht den jetzigen Inhalt von `LevelsPlugin::build` hierher:
/// Progress laden → Sprache setzen → Catalog bauen → als Resources einfügen.
fn setup_catalog_and_i18n(mut commands: Commands) {
    todo!("Code aus levels.rs::LevelsPlugin::build hierher: \
           Progress::load(), set_lang(...), insert_resource(load_catalog()), \
           insert_resource(progress)");
}
```

**Migration `levels.rs`:** Die Lade-Logik (`Progress::load`, `set_lang`,
`load_catalog`) bleibt als `pub(crate)` Funktionen in `levels.rs`, aber
`LevelsPlugin` macht **nichts mehr beim Build** — die Resources werden in
`setup_catalog_and_i18n` eingefügt. So bleibt `Catalog`/`Progress`-API für
ui/run/editor identisch (kein Aufruf-Site-Churn).

---

## 4. MainMenu-State (`src/ui/main_menu.rs`, neu)

Eigenes Sub-Plugin im `ui`-Modul (siehe `[[modulare-struktur-kurze-files]]`).

```rust
use bevy::prelude::*;
use super::widgets::{button, text_bundle, despawn_all, BUTTON_BG, BUTTON_BG_PRIMARY, TEXT_BRIGHT, TEXT_DIM};
use crate::loading::GameAssets;
use crate::state::GameState;

#[derive(Component)] struct UiMainMenu;
#[derive(Component, Clone, Copy)] enum MenuAction { Start, Sandbox, Quit }

pub(super) struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_all::<UiMainMenu>)
            .add_systems(Update, menu_clicks.run_if(in_state(GameState::MainMenu)));
    }
}

fn spawn_main_menu(mut commands: Commands, assets: Res<GameAssets>) {
    let font = assets.font.clone();
    // todo!(): zentrierter Column-Node mit Titel "STELLWERK" (große Schrift),
    //          Untertitel, dann Buttons Start / Sandbox / Beenden.
    //          Reuse `button(parent, &font, label, bg, MenuAction::...)`.
    todo!()
}

fn menu_clicks(
    mut interactions: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed { continue; }
        match action {
            MenuAction::Start => next.set(GameState::LevelSelect),
            MenuAction::Sandbox => todo!("Sandbox direkt starten ODER LevelSelect"),
            MenuAction::Quit => { exit.write(AppExit::Success); }
        }
    }
}
```

In `ui/mod.rs` registrieren: `main_menu::MainMenuPlugin` zur Plugin-Liste.

**Esc aus LevelSelect → MainMenu:** In der Streckenwahl ein
`leave_to_main_menu`-System ergänzen (Esc → `GameState::MainMenu`), damit man
zurück ins Menü kommt.

---

## 5. Font aus der Collection beziehen (`src/font.rs`)

`UiFont(pub Handle<Font>)` **bleibt als Typ** — alle `text_bundle(&font, …)`
Aufrufer unverändert. Nur die Befüllung ändert sich:

```rust
pub struct FontPlugin;

impl Plugin for FontPlugin {
    fn build(&self, app: &mut App) {
        // Race aus dem alten Kommentar ist weg: Text wird erst ab MainMenu
        // gespawnt, also lange NACH dem Loading-Gate. Keine PreStartup-Tricks.
        app.add_systems(OnExit(GameState::Loading), install_ui_font);
    }
}

fn install_ui_font(mut commands: Commands, assets: Res<GameAssets>) {
    commands.insert_resource(UiFont(assets.font.clone()));
}
```

Den ganzen `std::fs::read` + `Font::try_from_bytes`-Block löschen. Der
vendored-`bevy_text`-Fix ist davon unberührt (sitzt in der Rasterung).

---

## 6. `main.rs` — Plugin-Reihenfolge

```rust
.add_plugins((
    font::FontPlugin,
    loading::LoadingPlugin,   // NEU — fügt LoadingState hinzu
    state::StatePlugin,
    levels::LevelsPlugin,     // baut jetzt nichts mehr beim Build
    camera::CameraPlugin,
    board::BoardPlugin,
    editor::EditorPlugin,
    run::RunPlugin,
    ui::UiPlugin,
))
```

⚠️ `LoadingState` braucht den `GameState`-Resource → `StatePlugin` (mit
`init_state`) muss **vor** `LoadingPlugin` registriert sein, ODER `LoadingPlugin`
ruft `init_state` selbst. Im Referenzprojekt wird der LoadingState in
`StatePlugin` nach `init_state` registriert — sauberster Weg: **`LoadingState`
in `StatePlugin::build` registrieren**, `GameAssets` in `loading.rs` lassen.

---

## 7. Verifikation

- [ ] `cargo build` grün, `cargo clippy --workspace --all-targets` ohne Warnung.
- [ ] Start zeigt (kurz) Loading → MainMenu → Start → Streckenwahl. Esc-Kette
      zurück bis MainMenu.
- [ ] **FPS messen:** `STELLWERK_WINDOWED=1` + F3 (FPS-Overlay) im Run-Modus auf
      einem großen Sandbox-Layout. Erwartung: identisch zu vorher (Loading ändert
      In-Game-FPS nicht — siehe §0).
- [ ] `STELLWERK_AUTOCYCLE` läuft noch (autocycle setzt States direkt; ggf. um
      `MainMenu` erweitern, sonst startet er aus LevelSelect — passt weiterhin).
- [ ] Alle 63 Tests grün (`par_proof`/`levels` lesen direkt von Platte, von der
      State-Änderung unberührt).

---

## 8. Separat: die ECHTEN Optimierungen (NICHT in diesem Blatt umsetzen)

Reihenfolge nach Wirkung auf 60 FPS:

1. **`draw_run_board` baut JEDEN Frame ALLE Sprites neu** (`src/board/run_board.rs`):
   despawnt und respawnt im Run-Modus hunderte Entities pro Frame. Auf großen
   Sandbox-Layouts ist **das** der FPS-Killer. Fix: statische Geometrie
   (Gleisbänder, Stationen, Zell-Index) einmal bei `OnEnter(Run)` spawnen; pro
   Frame nur Farbe/Breite der Block-Bänder, Signal-Lampen-Farbe und
   Zug-Positionen in-place ändern (`Query<&mut Sprite>` + `&mut Transform`).
   Die `LiveGfx`-Entities behalten, nicht neu erzeugen.

2. **`signal_display_state` baut die `(Point,Point)→EdgeId`-Map pro Frame neu**
   (`run_board.rs`): einmal pro Run cachen (Resource), nicht jeden Frame.

3. **`Train::occupied()` alloziert einen `Vec` pro Aufruf** (`stellwerk_sim`),
   in der Render-Schleife mehrfach pro Zug pro Frame aufgerufen. Reuse-Buffer
   oder Iterator. (Sim-Crate — Determinismus-Hash nicht berühren.)

4. **vendored `bevy_text`: frischer `SwashCache` pro neuer Glyphe** — minimal
   mehr Allokation, nur bei Erst-Rasterung (einmal pro Glyph+Größe, dann Atlas-
   Cache). Vernachlässigbar; nur erwähnt der Vollständigkeit halber.

5. **Release-Build prüfen:** `cargo run --release --no-default-features`
   (ohne `dev`-Feature → kein egui-Inspector/FPS-Overlay-Overhead). Für echte
   FPS-Messung immer Release.

> Sag mir, welche davon ich angehen soll — Punkt 1 ist mit Abstand der größte
> Hebel und der eigentliche Weg zu konstant 60 FPS.
