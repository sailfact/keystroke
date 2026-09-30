//! The menu: pick a difficulty (each has its own progress), toggle sound,
//! or reset a difficulty's progress.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::{ScreenRoot, label, theme, thousands};
use crate::AppState;
use crate::difficulty::Difficulty;
use crate::progress::{LETTER_ORDER, Profile};
use crate::save::{self, SaveData};

/// Seconds to press R a second time to confirm a reset.
const RESET_WINDOW: f64 = 3.0;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResetArmed>()
            .add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(
                Update,
                (menu_keys, menu_clicks, refresh_menu)
                    .chain()
                    .run_if(in_state(AppState::Menu)),
            );
    }
}

#[derive(Component)]
struct Card(Difficulty);

#[derive(Component)]
struct CardStats(Difficulty);

#[derive(Component)]
struct Footer;

/// When R was pressed once, waiting for a second press to confirm.
#[derive(Resource, Default)]
struct ResetArmed(Option<f64>);

fn spawn_menu(mut commands: Commands, root: Single<Entity, With<ScreenRoot>>) {
    commands.entity(*root).with_children(|screen| {
        screen
            .spawn((
                DespawnOnExit(AppState::Menu),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(16),
                    margin: UiRect::top(px(8)),
                    ..default()
                },
            ))
            .with_children(|menu| {
                menu.spawn(label("KEYSTROKE", 64.0, theme::TEXT));
                menu.spawn(label(
                    "Touch-typing practice. Start with six keys and unlock the rest by getting faster.",
                    18.0,
                    theme::TEXT_DIM,
                ));
                menu.spawn(Node {
                    column_gap: px(18),
                    margin: UiRect::vertical(px(12)),
                    ..default()
                })
                .with_children(|cards| {
                    for difficulty in Difficulty::ALL {
                        cards
                            .spawn((
                                Card(difficulty),
                                Button,
                                Node {
                                    width: px(300),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(12),
                                    padding: px(20).all(),
                                    border: px(2).all(),
                                    border_radius: BorderRadius::all(px(12)),
                                    ..default()
                                },
                                BackgroundColor(theme::PANEL),
                                BorderColor::all(theme::BORDER),
                            ))
                            .with_children(|card| {
                                card.spawn(label(
                                    format!("{}  {}", difficulty.index() + 1, difficulty.label()),
                                    28.0,
                                    theme::TEXT,
                                ));
                                card.spawn(label(difficulty.blurb(), 15.0, theme::TEXT_DIM));
                                card.spawn((CardStats(difficulty), label("", 15.0, theme::TEXT)));
                            });
                    }
                });
                menu.spawn((Footer, label("", 16.0, theme::TEXT_DIM)));
            });
    });
}

/// Reads logical keys, so shortcuts follow the key labels on any layout.
fn menu_keys(
    mut inputs: MessageReader<KeyboardInput>,
    state: Res<State<AppState>>,
    time: Res<Time>,
    mut save: ResMut<SaveData>,
    mut reset: ResMut<ResetArmed>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Keys typed during practice are still queued the first time this runs
    // after returning to the menu. They aren't meant for the menu.
    if state.is_changed() {
        inputs.clear();
        return;
    }
    let now = time.elapsed_secs_f64();
    if reset.0.is_some_and(|armed| now - armed > RESET_WINDOW) {
        reset.0 = None;
    }

    for input in inputs.read() {
        if !input.state.is_pressed() || input.repeat {
            continue;
        }
        let selected = save.selected;
        let choice = match &input.logical_key {
            Key::ArrowLeft => Some(selected.prev()),
            Key::ArrowRight => Some(selected.next()),
            Key::Character(c) => match c.as_str() {
                "1" => Some(Difficulty::Easy),
                "2" => Some(Difficulty::Medium),
                "3" => Some(Difficulty::Hard),
                _ => None,
            },
            _ => None,
        };
        if let Some(choice) = choice.filter(|&choice| choice != selected) {
            save.selected = choice;
            reset.0 = None;
            continue;
        }

        match &input.logical_key {
            Key::Enter | Key::Space => {
                save::persist(&save);
                next_state.set(AppState::Practice);
                return;
            }
            Key::Character(c) if c.eq_ignore_ascii_case("m") => {
                save.sound_on = !save.sound_on;
                save::persist(&save);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                if reset.0.is_some() {
                    *save.profile_mut(selected) = Profile::default();
                    save::persist(&save);
                    reset.0 = None;
                } else {
                    reset.0 = Some(now);
                }
            }
            _ => {}
        }
    }
}

fn menu_clicks(
    cards: Query<(&Interaction, &Card), Changed<Interaction>>,
    mut save: ResMut<SaveData>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, Card(difficulty)) in &cards {
        if *interaction == Interaction::Pressed {
            save.selected = *difficulty;
            save::persist(&save);
            next_state.set(AppState::Practice);
        }
    }
}

fn refresh_menu(
    save: Res<SaveData>,
    reset: Res<ResetArmed>,
    mut cards: Query<(&Card, &Interaction, &mut BackgroundColor, &mut BorderColor)>,
    mut stats: Query<(&CardStats, &mut Text), Without<Footer>>,
    mut footer: Single<&mut Text, With<Footer>>,
) {
    for (Card(difficulty), interaction, mut background, mut border) in &mut cards {
        let selected = *difficulty == save.selected;
        let hovered = *interaction != Interaction::None;
        let fill = if selected || hovered {
            theme::PANEL_LIGHT
        } else {
            theme::PANEL
        };
        let edge = if selected {
            theme::ACCENT
        } else {
            theme::BORDER
        };
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(BorderColor::all(edge));
    }

    for (CardStats(difficulty), mut text) in &mut stats {
        let profile = save.profile(*difficulty);
        let value = format!(
            "Keys: {} of {}\nLessons: {}\nBest speed: {:.0} WPM\nTotal score: {}",
            profile.unlocked_letters().len(),
            LETTER_ORDER.len(),
            profile.lessons,
            profile.best_wpm,
            thousands(profile.total_score),
        );
        if text.0 != value {
            text.0 = value;
        }
    }

    let value = if reset.0.is_some() {
        format!(
            "Press R again to reset your {} progress",
            save.selected.label()
        )
    } else {
        let sound = if save.sound_on { "on" } else { "off" };
        format!(
            "Left/Right or 1-3: choose   |   Enter: start   |   M: sound {sound}   |   R twice: reset progress"
        )
    };
    if footer.0 != value {
        footer.0 = value;
    }
}
