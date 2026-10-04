//! The lesson text: one span per character, coloured by typing state, with
//! a highlighted cursor that flashes red on a mistake.

use bevy::prelude::*;

use super::theme;
use crate::AppState;
use crate::practice::{FeedbackKind, KeyFeedback, LessonCompleted, TypingSystems};
use crate::typing::{CharState, TypingSession};

const FONT_SIZE: f32 = 30.0;
/// How long the cursor stays red after a mistake.
const FLASH_SECS: f32 = 0.25;

pub struct PassagePlugin;

impl Plugin for PassagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorFlash>().add_systems(
            Update,
            (rebuild_passage, flash_on_mistake, color_passage)
                .chain()
                .after(TypingSystems)
                .run_if(in_state(AppState::Practice)),
        );
    }
}

/// The text entity that holds the lesson's character spans.
#[derive(Component)]
pub struct Passage;

#[derive(Component)]
struct PassageChar(usize);

/// Seconds left on the cursor's mistake flash.
#[derive(Resource, Default)]
struct CursorFlash(f32);

/// An empty passage. Its spans are added once the lesson exists.
pub fn passage_bundle() -> impl Bundle {
    (
        Passage,
        Text::default(),
        TextLayout::justify(Justify::Left),
        Node {
            max_width: px(1000),
            ..default()
        },
    )
}

fn rebuild_passage(
    mut commands: Commands,
    session: Res<TypingSession>,
    mut completed: MessageReader<LessonCompleted>,
    passage: Single<Entity, With<Passage>>,
) {
    let next_lesson = completed.read().count() > 0;
    if !session.is_added() && !next_lesson {
        return;
    }
    commands
        .entity(*passage)
        .despawn_related::<Children>()
        .with_children(|parent| {
            for (i, &c) in session.text().iter().enumerate() {
                parent.spawn((
                    PassageChar(i),
                    TextSpan::new(c.to_string()),
                    TextFont::from_font_size(FONT_SIZE),
                    TextColor(theme::TEXT_FAINT),
                    TextBackgroundColor(Color::NONE),
                    Underline,
                    UnderlineColor(Color::NONE),
                ));
            }
        });
}

fn flash_on_mistake(
    time: Res<Time>,
    mut flash: ResMut<CursorFlash>,
    mut feedback: MessageReader<KeyFeedback>,
) {
    flash.0 = (flash.0 - time.delta_secs()).max(0.0);
    let mistakes = feedback
        .read()
        .filter(|f| f.kind == FeedbackKind::Miss)
        .count();
    if mistakes > 0 {
        flash.0 = FLASH_SECS;
    }
}

fn color_passage(
    session: Res<TypingSession>,
    flash: Res<CursorFlash>,
    mut spans: Query<(
        &PassageChar,
        &mut TextColor,
        &mut TextBackgroundColor,
        &mut UnderlineColor,
    )>,
) {
    let cursor = (!session.is_finished()).then(|| session.cursor());
    let flashing = flash.0 > 0.0;
    for (PassageChar(i), mut color, mut background, mut underline) in &mut spans {
        let Some(&state) = session.states().get(*i) else {
            continue;
        };
        let (mut fg, mut bg, mut line) = match state {
            CharState::Pending => (theme::TEXT_FAINT, Color::NONE, Color::NONE),
            CharState::Correct => (theme::TEXT, Color::NONE, Color::NONE),
            CharState::Recovered => (theme::WARN, Color::NONE, Color::NONE),
        };
        if cursor == Some(*i) {
            fg = theme::TEXT;
            (bg, line) = if flashing {
                (theme::BAD_BACKGROUND, theme::BAD)
            } else {
                (theme::CURSOR_BACKGROUND, theme::ACCENT)
            };
        }
        color.set_if_neq(TextColor(fg));
        background.set_if_neq(TextBackgroundColor(bg));
        underline.set_if_neq(UnderlineColor(line));
    }
}
