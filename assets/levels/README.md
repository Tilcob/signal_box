# Levels

Lege hier deine LDtk-Welt als `world.ldtk` ab (Pfad konfiguriert in
`src/level.rs`, `WORLD_PATH`).

Konventionen (aus `src/level.rs`):

- **IntGrid-Wert `1` = Wand (solid)**, alle anderen Werte sind begehbar.
- Pro Level eine LDtk-Entity **`PlayerSpawn`** platzieren — der Level-Manager
  teleportiert den Spieler dorthin. Fehlt sie, spawnt er bei (0,0).
- Level heißen standardmäßig `Level_0`, `Level_1`, … (Debug-Levelwechsel über
  die Zifferntasten erwartet dieses Schema; eigene Namen funktionieren über
  `commands.transition_to_ldtk_level("MeinLevel", ...)`).

Tileset-Bilder relativ zur `.ldtk`-Datei speichern (z.B. `assets/levels/tiles.png`),
dann findet `bevy_ecs_ldtk` sie automatisch.
