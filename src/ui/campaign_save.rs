//! Dev-only tool: "save the sandbox as a campaign level".
//! A small docked button opens a centered modal (dim, see-through backdrop)
//! that collects the metadata — chapter/order, the name (→ file id AND display
//! name) and a briefing pasted from the clipboard — shows a live id/name
//! preview, then writes the sandbox `sim` to `assets/levels/<id>.ron` with a
//! filled `meta` block plus placeholder i18n keys.
//!
//! While the modal is open the board is frozen ([`SaveModalOpen`] gates the
//! edit input systems), so clicks/keys behind the backdrop don't edit the level.
//! The briefing is drafted in a real editor and pulled in via the clipboard —
//! no in-game multi-line widget.

use bevy::prelude::*;
use stellwerk_sim::level::{LEVEL_SCHEMA_VERSION, LevelMeta};

use super::numeric_field::{NumericField, numeric_field, numeric_field_focus, text_field};
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button, set_text, text_bundle,
};
use crate::console::ConsoleLog;
use crate::font::UiFont;
use crate::i18n::set_lang;
use crate::levels::{Catalog, Progress, SANDBOX_ID, load_catalog};
use crate::state::{ActiveLevel, Editor, FocusedField, GameState, SaveModalOpen};

/// Chapter range matches the campaign; order steps of 10 leave room to insert.
const CHAPTER_MIN: i64 = 1;
const CHAPTER_MAX: i64 = 8;
const ORDER_MIN: i64 = 10;
const ORDER_MAX: i64 = 200;
/// Cap on the typed level-name suffix — keeps the id a sane file-stem length.
const NAME_MAX: usize = 24;
/// Cap on the pasted briefing — long enough for an operating order, short
/// enough to keep the i18n line manageable.
const BRIEFING_MAX: usize = 600;

/// Inputs that don't come from the numeric fields. Reset each time the edit
/// screen opens. (The save result is logged to the in-level console.)
#[derive(Resource)]
struct CampaignDraft {
    hard: bool,
    /// Fold the on-screen build into `sim.fixed` on save. On = the build becomes
    /// pre-placed infrastructure (makes a level valid standalone); off = only the
    /// definition is written and `fixed` is left as authored.
    fold: bool,
    /// Briefing pulled from the clipboard; written to `meta.briefing` on save.
    briefing: String,
}

impl Default for CampaignDraft {
    fn default() -> Self {
        Self { hard: false, fold: true, briefing: String::new() }
    }
}

#[derive(Component)]
struct UiCampaignSave;
#[derive(Component)]
struct OpenModalButton;
#[derive(Component)]
struct SaveModalRoot;
#[derive(Component)]
struct ChapterField;
#[derive(Component)]
struct OrderField;
#[derive(Component)]
struct NameField;
#[derive(Component)]
struct HardButton;
#[derive(Component)]
struct FoldButton;
#[derive(Component)]
struct PasteButton;
#[derive(Component)]
struct CancelButton;
#[derive(Component)]
struct SaveButton;
#[derive(Component)]
struct InfoText;
#[derive(Component)]
struct BriefingText;

/// When this sandbox session was opened from a real campaign level (the SBX
/// dev button), saving overwrites THAT file with its original meta preserved,
/// instead of minting a fresh `_neu` level. `None` = a true sandbox.
fn overwrite_target(active: &ActiveLevel, catalog: &Catalog) -> Option<(String, LevelMeta)> {
    if active.id == SANDBOX_ID {
        return None;
    }
    catalog
        .0
        .iter()
        .find(|e| e.id == active.id)
        .map(|e| (e.id.clone(), e.meta.clone()))
}

pub(super) struct CampaignSavePlugin;

impl Plugin for CampaignSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampaignDraft>()
            .add_systems(OnEnter(GameState::Edit), spawn_trigger)
            .add_systems(OnExit(GameState::Edit), super::widgets::despawn_all::<UiCampaignSave>)
            .add_systems(
                Update,
                // `save_click` after `numeric_field_focus`: clicking Save blurs
                // the focused field, committing its typed buffer first.
                (
                    open_modal,
                    sync_modal,
                    toggle_hard,
                    toggle_fold,
                    paste_briefing,
                    cancel_modal,
                    save_click.after(numeric_field_focus),
                    update_info,
                )
                    .run_if(in_state(GameState::Edit)),
            );
    }
}

/// The docked button that opens the save modal (sandbox only). Also the reset
/// point: every Edit entry starts with the modal closed and a fresh draft.
fn spawn_trigger(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
    mut draft: ResMut<CampaignDraft>,
    mut modal: ResMut<SaveModalOpen>,
) {
    modal.0 = false;
    if !active.is_some_and(|a| a.sandbox) {
        return;
    }
    *draft = CampaignDraft::default();
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(10.0),
                ..default()
            },
            Interaction::default(),
            UiCampaignSave,
        ))
        .with_children(|c| {
            button(c, &font, "DEV: Level speichern", BUTTON_BG_PRIMARY, OpenModalButton);
        });
}

fn open_modal(
    interactions: Query<&Interaction, (Changed<Interaction>, With<OpenModalButton>)>,
    mut modal: ResMut<SaveModalOpen>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        modal.0 = true;
    }
}

/// Briefing box content: the pasted text, or a hint while empty.
fn briefing_display(briefing: &str) -> String {
    if briefing.trim().is_empty() {
        "(leer - Text im Editor kopieren, dann unten einfügen)".into()
    } else {
        briefing.to_string()
    }
}

/// Spawns the centered modal when it opens, despawns it when it closes. Runs on
/// `SaveModalOpen` change so it only rebuilds on a real toggle. A high
/// `GlobalZIndex` keeps the backdrop above the HUD it overlays.
fn sync_modal(
    modal: Res<SaveModalOpen>,
    existing: Query<Entity, With<SaveModalRoot>>,
    mut commands: Commands,
    ui_font: Res<UiFont>,
    draft: Res<CampaignDraft>,
    mut focus: ResMut<FocusedField>,
) {
    if !modal.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !modal.0 {
        focus.0 = None; // drop focus on a now-despawned field so input un-gates
        return;
    }
    let font = ui_font.0.clone();
    let row = || Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(100),
            Interaction::default(),
            UiCampaignSave,
            SaveModalRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        width: Val::Px(460.0),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    Interaction::default(),
                ))
                .with_children(|card| {
                    card.spawn(text_bundle(&font, "Level speichern".into(), 20.0, TEXT_BRIGHT));
                    card.spawn(row()).with_children(|r| {
                        r.spawn(text_bundle(&font, "Kapitel".into(), 13.0, TEXT_DIM));
                        numeric_field(r, &font, CHAPTER_MIN, CHAPTER_MIN, CHAPTER_MAX, ChapterField);
                        r.spawn(text_bundle(&font, "Order".into(), 13.0, TEXT_DIM));
                        numeric_field(r, &font, ORDER_MIN, ORDER_MIN, ORDER_MAX, OrderField);
                    });
                    card.spawn(row()).with_children(|r| {
                        r.spawn(text_bundle(&font, "Name".into(), 13.0, TEXT_DIM));
                        text_field(r, &font, "neu", NAME_MAX, NameField);
                    });
                    card.spawn(text_bundle(
                        &font,
                        "Briefing (im Editor schreiben, kopieren, dann einfügen):".into(),
                        12.0,
                        TEXT_DIM,
                    ));
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(60.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            text_bundle(&font, briefing_display(&draft.briefing), 13.0, TEXT_BRIGHT),
                            BriefingText,
                        ));
                    });
                    button(card, &font, "Aus Zwischenablage einfügen", BUTTON_BG, PasteButton);
                    card.spawn(row()).with_children(|r| {
                        button(r, &font, "hart umschalten", BUTTON_BG, HardButton);
                        button(r, &font, "Bau einbacken umschalten", BUTTON_BG, FoldButton);
                    });
                    card.spawn((text_bundle(&font, String::new(), 12.0, TEXT_DIM), InfoText));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|r| {
                        button(r, &font, "Abbrechen", BUTTON_BG, CancelButton);
                        button(r, &font, "Speichern", BUTTON_BG_PRIMARY, SaveButton);
                    });
                });
        });
}

fn toggle_hard(
    hard: Query<&Interaction, (Changed<Interaction>, With<HardButton>)>,
    mut draft: ResMut<CampaignDraft>,
) {
    if hard.iter().any(|i| *i == Interaction::Pressed) {
        draft.hard = !draft.hard;
    }
}

fn toggle_fold(
    fold: Query<&Interaction, (Changed<Interaction>, With<FoldButton>)>,
    mut draft: ResMut<CampaignDraft>,
) {
    if fold.iter().any(|i| *i == Interaction::Pressed) {
        draft.fold = !draft.fold;
    }
}

fn paste_briefing(
    interactions: Query<&Interaction, (Changed<Interaction>, With<PasteButton>)>,
    mut draft: ResMut<CampaignDraft>,
    mut log: ResMut<ConsoleLog>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    match crate::clipboard::paste() {
        Ok(text) => {
            draft.briefing = text.trim().chars().take(BRIEFING_MAX).collect();
            log.info("Briefing aus Zwischenablage übernommen.");
        }
        Err(_) => log.warn("Zwischenablage leer - erst Briefing-Text kopieren."),
    }
}

/// Cancel button or Esc closes the modal (Esc is free here — the pause toggle is
/// gated off while the modal is open).
fn cancel_modal(
    interactions: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut modal: ResMut<SaveModalOpen>,
) {
    if !modal.0 {
        return;
    }
    let clicked = interactions.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Escape) {
        modal.0 = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn save_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    name: Query<&NumericField, With<NameField>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut catalog: ResMut<Catalog>,
    draft: Res<CampaignDraft>,
    mut modal: ResMut<SaveModalOpen>,
    mut log: ResMut<ConsoleLog>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(active) = active.filter(|a| a.sandbox) else {
        return;
    };
    let (Ok(chapter), Ok(order)) = (chapter.single(), order.single()) else {
        return;
    };
    // Opened from a real campaign level (SBX) → overwrite that file, keeping its
    // original meta (chapter/order/briefing). A true sandbox → mint a level from
    // the typed fields. Fields are clamped, so the casts never truncate.
    let overwrite = overwrite_target(&active, &catalog);
    let (id, meta, display_name) = match &overwrite {
        // SBX overwrite keeps the level's existing name and meta (briefing edits
        // for an existing level stay a file/i18n task).
        Some((id, meta)) => (id.clone(), meta.clone(), active.level.name.clone()),
        None => {
            let chapter = chapter.value() as u8;
            let order = order.value() as u16;
            let slug = name.single().map(|f| slugify(f.text())).unwrap_or_else(|_| "neu".into());
            // Display name matches the `level.<id>.name` convention, e.g.
            // "1.2 Kurvige Strecke" — chapter.order + the title-cased slug.
            let display = format!("{}.{} {}", chapter, order / 10, titleize(&slug));
            (
                unique_id(chapter, order, &slug),
                LevelMeta {
                    schema_version: LEVEL_SCHEMA_VERSION,
                    chapter,
                    order,
                    optional_hard: draft.hard,
                    briefing: draft.briefing.trim().to_string(),
                },
                display,
            )
        }
    };
    let mut sim = active.level.clone();
    // Replace the carried-over "Sandbox" name with the derived one (unchanged
    // for an SBX overwrite, which reuses the level's own name).
    sim.name = display_name;
    // Forced fold for an SBX overwrite: there the layout IS the level's authored
    // track (lifted out of `fixed` on open), so not folding would drop it.
    if draft.fold || overwrite.is_some() {
        sim.fixed = sim.fixed.merged(&editor.layout);
    }

    match crate::authoring::write_campaign_level(&id, meta, sim.clone()) {
        Ok(path) => {
            // i18n keys were appended; reload the live table, then refresh the
            // catalog so the new level shows up back in the level select.
            let lang = if progress.lang.is_empty() {
                "de"
            } else {
                &progress.lang
            };
            set_lang(lang);
            *catalog = load_catalog();
            // When the build was folded into `fixed` AND we overwrote the level
            // in place, the per-level autosave now duplicates that build — drop
            // it so re-opening via SBX doesn't stack a second copy on top.
            if overwrite.is_some() && draft.fold {
                progress.entry(&id).layout = stellwerk_sim::Layout::default();
                progress.save();
            }
            // Validate with an EMPTY player layout — exactly what
            // `tests/levels.rs` enforces.
            let errors = stellwerk_sim::validate(&sim, &stellwerk_sim::Layout::default());
            if errors.is_empty() {
                log.info(format!("Gespeichert: {} · Katalog neu geladen", path.display()));
            } else {
                log.warn(format!(
                    "Gespeichert ({}), aber ungültig mit leerem Layout: {}",
                    path.display(),
                    errors[0]
                ));
            }
        }
        Err(e) => log.error(format!("Fehler beim Speichern: {e}")),
    }
    modal.0 = false; // close the modal; the result is in the console
}

#[allow(clippy::too_many_arguments)]
fn update_info(
    draft: Res<CampaignDraft>,
    chapter: Query<&NumericField, With<ChapterField>>,
    order: Query<&NumericField, With<OrderField>>,
    name: Query<&NumericField, With<NameField>>,
    active: Option<Res<ActiveLevel>>,
    catalog: Option<Res<Catalog>>,
    mut info_texts: Query<&mut Text, (With<InfoText>, Without<BriefingText>)>,
    mut briefing_texts: Query<&mut Text, (With<BriefingText>, Without<InfoText>)>,
) {
    // No fields → the modal is closed; nothing to mirror.
    let (Ok(chapter), Ok(order)) = (chapter.single(), order.single()) else {
        return;
    };
    let slug = name.single().map(|f| slugify(f.text())).unwrap_or_else(|_| "neu".into());
    // Mirror `save_click`'s target choice so the line never lies about what
    // "Speichern" will do.
    let overwrite = match (active.as_ref(), catalog.as_ref()) {
        (Some(a), Some(c)) => overwrite_target(a, c),
        _ => None,
    };
    // SBX always folds (the layout is the level's authored track) — show that
    // rather than the raw toggle.
    let forced_fold = overwrite.is_some();
    let display_name = if forced_fold {
        active.as_ref().map(|a| a.level.name.clone()).unwrap_or_default()
    } else {
        format!("{}.{} {}", chapter.value() as u8, order.value() as u16 / 10, titleize(&slug))
    };
    let target = match overwrite {
        Some((id, _)) => format!("überschreibt {id}.ron"),
        None => format!(
            "neu: {}.ron",
            preview_id(chapter.value() as u8, order.value() as u16, &slug)
        ),
    };
    if let Ok(mut text) = info_texts.single_mut() {
        let hard = if draft.hard { "an" } else { "aus" };
        let fold = if forced_fold {
            "an (SBX)"
        } else if draft.fold {
            "an"
        } else {
            "aus"
        };
        set_text(
            &mut text,
            format!(
                "Kapitel: {} · Order: {} · hart: {hard} · einbacken: {fold}\n{target} · Name: {display_name}",
                chapter.value(),
                order.value()
            ),
        );
    }
    if let Ok(mut text) = briefing_texts.single_mut() {
        set_text(&mut text, briefing_display(&draft.briefing));
    }
}

/// Turns a typed level name into the id suffix: lowercase ASCII, every run of
/// other characters collapsed to a single `_`, ends trimmed. Empty → `"neu"`
/// (the historical default). Never yields `__` (the solution-variant separator)
/// or spaces, so the id stays a valid, unambiguous file stem.
fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut gap = false; // saw a separator char since the last alnum
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('_'); // one `_` between alnum runs; never leading/trailing
            }
            slug.push(c.to_ascii_lowercase());
            gap = false;
        } else {
            gap = true;
        }
    }
    if slug.is_empty() { "neu".to_string() } else { slug }
}

/// `kurvige_strecke` → `Kurvige Strecke`: split on `_`, capitalize each word's
/// first letter, join with spaces. The level's display name, mirroring the
/// existing `level.<id>.name` convention ("1.2 Kurvige Strecke").
fn titleize(slug: &str) -> String {
    slug.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// id stem the next save would use, BEFORE de-duplication (preview only).
fn preview_id(chapter: u8, order: u16, slug: &str) -> String {
    format!("k{chapter}_{:02}_{slug}", order / 10)
}

/// A real, free file id: the preview stem, with `_2`, `_3`, … appended if a
/// file already exists. Falls back to the bare stem if everything is taken.
fn unique_id(chapter: u8, order: u16, slug: &str) -> String {
    let base = preview_id(chapter, order, slug);
    let exists = |id: &str| std::path::Path::new("assets/levels").join(format!("{id}.ron")).exists();
    if !exists(&base) {
        return base;
    }
    (2..1000)
        .map(|n| format!("{base}_{n}"))
        .find(|cand| !exists(cand))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_makes_valid_id_suffixes() {
        assert_eq!(slugify("Kurvige Strecke"), "kurvige_strecke");
        assert_eq!(slugify("  Erste   Gleise  "), "erste_gleise");
        // Empty or punctuation-only → the historical default.
        assert_eq!(slugify(""), "neu");
        assert_eq!(slugify("!!!"), "neu");
        // Never a leading/trailing or doubled `_` (the variant separator).
        let s = slugify("__A b__");
        assert!(
            !s.starts_with('_') && !s.ends_with('_') && !s.contains("__"),
            "bad slug: {s}"
        );
    }

    #[test]
    fn titleize_capitalizes_words() {
        assert_eq!(super::titleize("kurvige_strecke"), "Kurvige Strecke");
        assert_eq!(super::titleize("neu"), "Neu");
        // Round-trips the slugify output: a save of "Kurvige Strecke" reads back
        // the same display words.
        assert_eq!(super::titleize(&slugify("Kurvige Strecke")), "Kurvige Strecke");
    }
}
