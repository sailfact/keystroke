//! The practice screen: live speed, accuracy and score, the key strip that
//! shows unlock progress, the lesson text, and the last lesson's results.

use bevy::prelude::*;

use super::passage::passage_bundle;
use super::{ScreenRoot, label, theme, thousands};
use crate::AppState;
use crate::practice::{LastLesson, LessonCompleted, TypingSystems};
use crate::progress::LETTER_ORDER;
use crate::save::SaveData;
use crate::typing::{TypingSession, lesson_score};

/// How long the "new key" toast stays up.
const TOAST_SECS: f32 = 3.0;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Practice), spawn_practice_screen)
            .add_systems(
                Update,
                (
                    update_stats,
                    update_progress,
                    update_key_strip,
                    update_status,
                    update_hint,
                    update_last_lesson,
                    show_unlock_toast,
                    fade_toasts,
                )
                    .after(TypingSystems)
                    .run_if(in_state(AppState::Practice)),
            );
    }
}

#[derive(Component, Clone, Copy)]
enum Stat {
    Speed,
    Accuracy,
    Score,
    Time,
}

#[derive(Component)]
struct StripKey(char);

#[derive(Component)]
struct ProgressFill;

#[derive(Component)]
struct StatusLine;

#[derive(Component)]
struct HintLine;

#[derive(Component)]
struct LastLessonLine;

#[derive(Component)]
struct PracticeColumn;

#[derive(Component)]
struct Toast {
    remaining: f32,
}

#[derive(Component)]
struct ToastBox;

fn spawn_practice_screen(
    mut commands: Commands,
    root: Single<Entity, With<ScreenRoot>>,
    save: Res<SaveData>,
) {
    let difficulty = save.selected;
    let heading = format!(
        "{}  |  keys unlock at {} WPM",
        difficulty.label().to_uppercase(),
        difficulty.rules().target_wpm
    );
    commands.entity(*root).with_children(|screen| {
        screen
            .spawn((
                PracticeColumn,
                DespawnOnExit(AppState::Practice),
                Node {
                    width: percent(100),
                    max_width: px(1100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(14),
                    ..default()
                },
            ))
            .with_children(|column| {
                column
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::End,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(label(heading, 18.0, theme::TEXT_DIM));
                        row.spawn(Node {
                            column_gap: px(32),
                            ..default()
                        })
                        .with_children(|stats| {
                            for (stat, name) in [
                                (Stat::Speed, "SPEED"),
                                (Stat::Accuracy, "ACCURACY"),
                                (Stat::Score, "SCORE"),
                                (Stat::Time, "TIME"),
                            ] {
                                stats
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::End,
                                        ..default()
                                    })
                                    .with_children(|cell| {
                                        cell.spawn(label(name, 12.0, theme::TEXT_DIM));
                                        cell.spawn((stat, label("-", 28.0, theme::TEXT)));
                                    });
                            }
                        });
                    });

                column
                    .spawn(Node {
                        column_gap: px(6),
                        ..default()
                    })
                    .with_children(|strip| {
                        for c in LETTER_ORDER {
                            strip
                                .spawn((
                                    StripKey(c),
                                    Node {
                                        width: px(32),
                                        height: px(34),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: px(2).all(),
                                        border_radius: BorderRadius::all(px(6)),
                                        ..default()
                                    },
                                    BackgroundColor(theme::PANEL),
                                    BorderColor::all(Color::NONE),
                                ))
                                .with_child(label(c.to_string(), 18.0, theme::TEXT));
                        }
                    });
                column.spawn((StatusLine, label("", 16.0, theme::TEXT_DIM)));

                column
                    .spawn((
                        Node {
                            width: percent(100),
                            height: px(6),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_child((
                        ProgressFill,
                        Node {
                            width: percent(0),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(theme::ACCENT),
                    ));

                column
                    .spawn((
                        Node {
                            width: percent(100),
                            min_height: px(170),
                            padding: px(24).all(),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(px(12)),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_child(passage_bundle());

                column
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((LastLessonLine, label("", 16.0, theme::TEXT_DIM)));
                        row.spawn((HintLine, label("", 16.0, theme::TEXT_DIM)));
                    });
            });
    });
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

fn update_stats(
    session: Res<TypingSession>,
    save: Res<SaveData>,
    time: Res<Time>,
    mut stats: Query<(&Stat, &mut Text)>,
) {
    let now = time.elapsed_secs_f64();
    let wpm = session.wpm(now);
    let accuracy = session.accuracy();
    let score = lesson_score(
        wpm,
        accuracy,
        save.current().unlocked_letters().len(),
        save.selected.rules().score_multiplier,
    );
    for (stat, mut text) in &mut stats {
        let value = if !session.is_started() {
            "-".to_string()
        } else {
            match stat {
                Stat::Speed => format!("{wpm:.0} wpm"),
                Stat::Accuracy => format!("{:.1}%", accuracy * 100.0),
                Stat::Score => thousands(u64::from(score)),
                Stat::Time => {
                    let secs = session.elapsed(now) as u64;
                    format!("{}:{:02}", secs / 60, secs % 60)
                }
            }
        };
        set_text(&mut text, value);
    }
}

fn update_progress(session: Res<TypingSession>, mut fill: Single<&mut Node, With<ProgressFill>>) {
    let width = percent(session.progress() * 100.0);
    if fill.width != width {
        fill.width = width;
    }
}

fn update_key_strip(
    save: Res<SaveData>,
    mut keys: Query<(&StripKey, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut labels: Query<&mut TextColor>,
) {
    let target = save.selected.rules().target_wpm;
    let profile = save.current();
    let focus = profile.focus_key(target);
    for (StripKey(c), mut background, mut border, children) in &mut keys {
        let (fill, text) = if profile.is_unlocked(*c) {
            let confidence = profile.key(*c).confidence(target);
            (theme::confidence(confidence, 0.35), theme::TEXT)
        } else {
            (theme::PANEL, theme::TEXT_FAINT)
        };
        let edge = if focus == Some(*c) {
            theme::FOCUS
        } else if profile.next_letter() == Some(*c) {
            theme::BORDER
        } else {
            Color::NONE
        };
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(BorderColor::all(edge));
        for child in children.iter() {
            if let Ok(mut color) = labels.get_mut(child) {
                color.set_if_neq(TextColor(text));
            }
        }
    }
}

fn update_status(save: Res<SaveData>, mut line: Single<&mut Text, With<StatusLine>>) {
    let target = save.selected.rules().target_wpm;
    let profile = save.current();
    let focus = match profile.focus_key(target) {
        Some(c) => match profile.key(c).wpm() {
            Some(wpm) => format!("Focus: {c}  {wpm:.0} of {target:.0} WPM"),
            None => format!("Focus: {c}  (new key)"),
        },
        None => "Every key is at target speed".to_string(),
    };
    let next = match profile.next_letter() {
        Some(c) => format!(
            "Next key: {c}  unlocks when all {} keys reach {target:.0} WPM ({} there)",
            profile.unlocked_letters().len(),
            profile.confident_keys(target),
        ),
        None => "All keys unlocked".to_string(),
    };
    set_text(&mut line, format!("{focus}     |     {next}"));
}

fn update_hint(
    session: Res<TypingSession>,
    mut hint: Single<(&mut Text, &mut TextColor), With<HintLine>>,
) {
    let (text, color) = &mut *hint;
    if session.word_has_error() {
        set_text(text, "Fix the mistake with Backspace".to_string());
        color.set_if_neq(TextColor(theme::BAD));
    } else {
        set_text(text, "Esc: menu".to_string());
        color.set_if_neq(TextColor(theme::TEXT_DIM));
    }
}

fn update_last_lesson(
    last: Res<LastLesson>,
    session: Res<TypingSession>,
    mut line: Single<(&mut Text, &mut TextColor), With<LastLessonLine>>,
) {
    let (text, color) = &mut *line;
    match last.0 {
        None => {
            let hint = if session.is_started() {
                ""
            } else {
                "Start typing. The timer begins with your first key."
            };
            set_text(text, hint.to_string());
            color.set_if_neq(TextColor(theme::TEXT_DIM));
        }
        Some(lesson) => {
            let best = match (lesson.best_wpm, lesson.best_score) {
                (true, true) => "  New best speed and score!",
                (true, false) => "  New best speed!",
                (false, true) => "  New best score!",
                (false, false) => "",
            };
            let errors = match lesson.errors {
                1 => "1 error".to_string(),
                n => format!("{n} errors"),
            };
            set_text(
                text,
                format!(
                    "Last lesson: {:.0} WPM  |  {:.1}%  |  {errors}  |  +{} points{best}",
                    lesson.wpm,
                    lesson.accuracy * 100.0,
                    thousands(u64::from(lesson.score)),
                ),
            );
            let tone = if best.is_empty() {
                theme::TEXT
            } else {
                theme::GOOD
            };
            color.set_if_neq(TextColor(tone));
        }
    }
}

/// Shows the toast as an overlay just below the practice column, so it
/// covers neither the new lesson nor the last lesson's results.
fn show_unlock_toast(
    mut commands: Commands,
    mut completed: MessageReader<LessonCompleted>,
    column: Single<Entity, With<PracticeColumn>>,
) {
    for unlocked in completed.read().filter_map(|c| c.summary.unlocked) {
        commands.entity(*column).with_child((
            Toast {
                remaining: TOAST_SECS,
            },
            GlobalZIndex(10),
            Node {
                position_type: PositionType::Absolute,
                top: percent(100),
                left: px(0),
                right: px(0),
                margin: UiRect::top(px(16)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            children![(
                ToastBox,
                Node {
                    padding: UiRect::axes(px(28), px(12)),
                    border: px(2).all(),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(theme::PANEL_LIGHT),
                BorderColor::all(theme::FOCUS),
                children![label(
                    format!("New key unlocked: {}", unlocked.to_ascii_uppercase()),
                    34.0,
                    theme::FOCUS,
                )],
            )],
        ));
    }
}

fn fade_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut Toast)>,
    children: Query<&Children>,
    mut boxes: Query<(&mut BackgroundColor, &mut BorderColor), With<ToastBox>>,
    mut texts: Query<&mut TextColor>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.remaining -= time.delta_secs();
        if toast.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // Fade out over the last half second.
        let alpha = (toast.remaining / 0.5).min(1.0);
        for part in children.iter_descendants(entity) {
            if let Ok((mut background, mut border)) = boxes.get_mut(part) {
                background.0 = theme::PANEL_LIGHT.with_alpha(alpha);
                *border = BorderColor::all(theme::FOCUS.with_alpha(alpha));
            }
            if let Ok(mut text) = texts.get_mut(part) {
                text.0 = theme::FOCUS.with_alpha(alpha);
            }
        }
    }
}
