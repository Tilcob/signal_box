# signal_box — „Stellwerk"-Prototyp

Prototyp zu Spielidee #8 („Stellwerk"): ein Zachlike über Zugsignale, gebaut
mit Bevy 0.18. Bring alle Züge des Fahrplans kollisionsfrei zum Bahnhof ihrer
Frachtfarbe — mit nichts als Signalen und einer Weiche.

> **Design:** Single Source of Truth ist [GDD.md](GDD.md). Der hiesige
> Prototyp ist ein Live-Dispatcher-Spike und weicht bewusst vom Zieldesign ab
> (strikte Bau-/Sim-Phasentrennung) — Details in GDD §14.

## Loslegen

```sh
cargo run                                      # mit Dev-Tools (default)
cargo build --release --no-default-features    # Release ohne Dev-Bloat
```

## Spielprinzip

```
A ────●──╗(Signal)              ╔──●──── ORANGE
         ╠══════ Weiche ════════╣
B ────●──╝(Signal)              ╚──●──── BLAU
```

- Aus den Quellen **A** und **B** fahren Züge nach festem Fahrplan los —
  orange und blaue.
- Beide Linien teilen sich das Mittelgleis. Fahren zwei Züge gleichzeitig auf
  die Zusammenführung zu: **Kollision, Run vorbei.**
- **Klick auf ein Signal** (Lampe neben dem Gleis) stellt es auf Rot/Grün —
  rote Signale halten Züge vor der Zusammenführung an.
- **Klick auf die Weiche** (gelber Kreis) wechselt den Abzweig — orange Züge
  müssen nach ORANGE, blaue nach BLAU.
- Der Fahrplan enthält gleichzeitige Abfahrten: ohne Signaleinsatz kracht es.
- Sieg: alle 12 Züge abgefertigt. Perfekt: alle 12 richtig zugestellt.

## Steuerung

| Eingabe | Aktion |
|---------|--------|
| Linksklick auf Signal | Rot/Grün umschalten |
| Linksklick auf Weiche | Abzweig wechseln |
| Esc | Pause (Umschalten geht auch pausiert — planen erlaubt) |
| R | Neustart |

## Dev-Tools (Feature `dev`, default an)

| Taste | Tool |
|-------|------|
| F3 | FPS-Overlay |
| F11 | Tunables-Inspector (Konstanten temporär zur Laufzeit ändern) |
| F12 | World-Inspector (Entities, Components, Resources, Assets) |

**Hot-Reload:** `assets/config/game.tunables.ron` speichern → Zuggeschwindigkeit
& Co. gelten sofort im laufenden Spiel.

## Architektur

```
src/
├── main.rs              # Composition Root: Window, Assets, Plugin-Liste
├── core/                # Fundament
│   ├── states.rs        #   GameState (Playing/Ended), Pause-Substate, GameplaySet
│   └── tunables.rs      #   hot-reloadbare Konstanten-Resource
├── sim/                 # Die Simulation
│   ├── graph.rs         #   Gleisnetz als Graph: Knoten, Kanten, Weichen + Level
│   ├── signal.rs        #   Blocksignale (Komponente + Spawn + Färbung)
│   └── train.rs         #   Fahrplan, Bewegung, Ankunft, Kollision, Run-Ende
├── interaction.rs       # Maus-Picking: Signale & Weichen umschalten
├── render.rs            # Kamera, Bahnhofs-Sprites, Gizmo-Gleise & -Weichen
├── ui.rs                # HUD, Pause-/End-Overlay, Neustart
└── dev_tools.rs         # nur mit Feature `dev`
```

Patterns:

- **Simulationssysteme** laufen in `.in_set(GameplaySet)` — pausiert
  automatisch, läuft nur in `Playing`. Reihenfolge pro Frame:
  Abfahrten → Bewegung → Kollisionscheck → Abschlusscheck.
- Das **Gleisnetz** ist eine einzige `TrackGraph`-Resource; Züge sind die
  einzigen Sim-Entities (plus Signal-Lampen). Routing entscheidet sich am
  Knoten: Weiche → gewählter Abzweig, sonst die einzige Fortsetzung.
- Der **Fahrplan** ist eine feste Liste (deterministisch = debugbar);
  `delay: 0.0`-Paare sind die beabsichtigten Konfliktmomente.

## Prototyp-Status / nächste Schritte

Bewusst nicht drin: mehrere Level, Bewertung (Durchsatz/Pünktlichkeit),
Level-Sharing, Sound. Erste sinnvolle Ausbauten: mehr Gleislayouts als Daten
statt Hardcode, Blocklogik (Signal schaltet automatisch auf Rot, solange der
Block besetzt ist), Zeitbewertung pro Run.

`_template_unused/` enthält die ausgemusterten Module des 2D-Templates
(LDtk-Level, Player, Follow-Kamera) — kann gelöscht werden.
