# bevy_template_2d

Schlankes 2D-Starter-Template für Bevy 0.18 mit
[`bevy_ldtk_integration`](https://github.com/Tilcob/bevy_ldtk_integration) als
Level-Manager, hot-reloadbaren Konstanten und Dev-Tools.

## Loslegen

```sh
cargo run                                      # mit Dev-Tools (default)
cargo build --release --no-default-features    # Release ohne Dev-Bloat
```

Das Template läuft sofort — auch ohne Level-Datei (der Spieler bewegt sich dann
frei). Eigene Welt: LDtk-Datei als `assets/levels/world.ldtk` speichern
(Konventionen siehe `assets/levels/README.md`).

## Steuerung

| Taste | Aktion |
|-------|--------|
| WASD / Pfeile | Bewegen |
| Shift | Rennen |
| Esc | Pause |

## Dev-Tools (Feature `dev`, default an)

| Taste | Tool |
|-------|------|
| F1 | Debug-Overlay: Collision-Gizmos + Levelwechsel über Zifferntasten 1–9 |
| F3 | FPS-Overlay |
| F11 | Tunables-Inspector (Konstanten temporär zur Laufzeit ändern) |
| F12 | World-Inspector (Entities, Components, Resources, Assets) |

**Asset-Hot-Reload** läuft automatisch: jede gespeicherte Datei unter `assets/`
(Tunables-RON, LDtk-Welt, Sprites) wird im laufenden Spiel neu geladen.

## Konstanten tunen

Alle Gameplay-Konstanten leben in `assets/config/game.tunables.ron` und werden
in die `Tunables`-Resource gespiegelt:

- **Persistent:** RON-Datei editieren und speichern → Werte gelten sofort im
  laufenden Spiel (Hot Reload).
- **Temporär:** F11-Inspector → Werte per Slider ändern; der nächste
  Datei-Save überschreibt sie wieder.

Neue Konstante = Feld in `src/tunables.rs` (mit `Default`-Wert) + Eintrag im
RON. Felder sind `#[serde(default)]` — alte RON-Dateien brechen nicht.

## Architektur

Vier Modulgruppen mit je einem Bündel-Plugin; Querabhängigkeiten laufen nur
über Components/Resources:

```
src/
├── main.rs              # Composition Root: Window, Assets, Plugin-Liste
├── core/                # CorePlugin — Fundament
│   ├── states.rs        #   GameState (Loading → Playing), Pause-Substate, GameplaySet
│   └── tunables.rs      #   hot-reloadbare Konstanten-Resource
├── level/               # LevelPlugin — LDtk-Welt + Level-Manager
│   ├── collision.rs     #   Collider, CollisionMap-Rebuild bei Level-Load/Hot-Reload
│   └── collision/map.rs #   Grid-Storage, Sweep + Slide, Unit-Tests
├── player/              # PlayerPlugin
│   ├── spawn.rs         #   Entity-Setup, Collider-Sync, Wand-Snap-Safety-Net
│   ├── input.rs         #   WASD/Pfeile + Shift → Velocity
│   └── movement.rs      #   kinematisches Movement gegen die CollisionMap
├── graphics/            # GraphicsPlugin — Präsentation
│   ├── camera.rs        #   Follow-Cam mit Deadzone, Lead und Live-Zoom
│   ├── y_sort.rs        #   Y-Sorting für Top-Down-Tiefe
│   └── animation.rs     #   generische Spritesheet-Animation
└── dev_tools.rs         # DevToolsPlugin — nur mit Feature `dev`
```

Wiederkehrende Patterns:

- **Gameplay-Systeme** kommen in `.in_set(GameplaySet)` — pausiert automatisch,
  läuft nur in `Playing`.
- **LDtk-Entities** registrierst du in `src/level.rs` mit einer Zeile:
  `.register_ldtk_entity::<MyBundle>("MyIdentifier")`.
- **Levelwechsel** aus Gameplay-Code:
  `commands.transition_to_ldtk_level("Level_1", Some(PLAYER_SPAWN_TAG.into()))`
  (Trait `ldtk_integration::LdtkCommandExt`).
- Der Spieler trägt `LdtkLevelPlayer` — der Level-Manager teleportiert ihn nach
  jeder Transition zum aufgelösten Spawn-Point. Ein Safety-Net snappt ihn aus
  Wänden, falls der Spawn blockiert ist.
