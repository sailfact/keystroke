//! The practice loop. Keyboard input drives the current lesson's
//! [`TypingSession`], and a finished lesson updates progress and rolls
//! straight into the next one, as on keybr.com.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use fastrand::Rng;

use crate::AppState;
use crate::lesson::LessonGenerator;
use crate::save::{self, SaveData};
use crate::typing::{KeyResult, TypingSession};

pub struct PracticePlugin;

impl Plugin for PracticePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<KeyFeedback>()
            .add_message::<LessonCompleted>()
            .init_resource::<LastLesson>()
            .init_resource::<LessonGenerator>()
            .insert_resource(LessonRng(Rng::new()))
            .add_systems(OnEnter(AppState::Practice), start_practice)
            .add_systems(OnExit(AppState::Practice), stop_practice)
            .add_systems(
                Update,
                (handle_typing, finish_lesson)
                    .chain()
                    .in_set(TypingSystems)
                    .run_if(in_state(AppState::Practice)),
            );
    }
}

/// Reads input and updates the lesson. Systems that show the lesson run
/// after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypingSystems;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackKind {
    Hit,
    Miss,
    Blocked,
    Backspace,
}

/// Sent for each keystroke the lesson reacted to.
#[derive(Message, Debug, Clone, Copy)]
pub struct KeyFeedback {
    pub kind: FeedbackKind,
    /// The physical key that was pressed.
    pub key: KeyCode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonSummary {
    pub wpm: f32,
    pub accuracy: f32,
    pub errors: u32,
    pub seconds: f32,
    pub score: u32,
    pub unlocked: Option<char>,
    pub best_wpm: bool,
    pub best_score: bool,
}

/// Sent when a lesson is finished, after progress is saved and the next
/// lesson has replaced it.
#[derive(Message, Debug, Clone, Copy)]
pub struct LessonCompleted {
    pub summary: LessonSummary,
}

/// The latest lesson's results, for the HUD.
#[derive(Resource, Default)]
pub struct LastLesson(pub Option<LessonSummary>);

#[derive(Resource)]
struct LessonRng(Rng);

/// A new lesson for the selected difficulty.
fn next_session(save: &SaveData, generator: &LessonGenerator, rng: &mut Rng) -> TypingSession {
    let rules = save.selected.rules();
    let profile = save.current();
    let text = generator.generate(
        profile.unlocked_letters(),
        profile.focus_key(rules.target_wpm),
        &rules,
        rng,
    );
    debug!("New lesson: {text}");
    TypingSession::new(&text, rules.mistakes)
}

fn start_practice(
    mut commands: Commands,
    save: Res<SaveData>,
    generator: Res<LessonGenerator>,
    mut rng: ResMut<LessonRng>,
    mut last: ResMut<LastLesson>,
) {
    commands.insert_resource(next_session(&save, &generator, &mut rng.0));
    last.0 = None;
}

fn stop_practice(mut commands: Commands) {
    commands.remove_resource::<TypingSession>();
}

fn handle_typing(
    mut inputs: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut session: ResMut<TypingSession>,
    mut feedback: MessageWriter<KeyFeedback>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // The first time this runs, the menu keypress that started practice is
    // still queued. It isn't meant for the lesson.
    if session.is_added() {
        inputs.clear();
        return;
    }
    let now = time.elapsed_secs_f64();
    let shortcut_held = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);

    for input in inputs.read() {
        if !input.state.is_pressed() {
            continue;
        }
        let kind = match &input.logical_key {
            Key::Escape => {
                next_state.set(AppState::Menu);
                return;
            }
            Key::Backspace => {
                if !session.backspace(now) {
                    continue;
                }
                FeedbackKind::Backspace
            }
            key => {
                // Holding a key down shouldn't type it again and again.
                if input.repeat || shortcut_held {
                    continue;
                }
                let Some(c) = typed_char(key) else {
                    continue;
                };
                match session.type_char(c, now) {
                    KeyResult::Hit => FeedbackKind::Hit,
                    KeyResult::Miss => FeedbackKind::Miss,
                    KeyResult::Blocked => FeedbackKind::Blocked,
                    KeyResult::Ignored => continue,
                }
            }
        };
        feedback.write(KeyFeedback {
            kind,
            key: input.key_code,
        });
    }
}

/// The character a key types, if it types exactly one.
fn typed_char(key: &Key) -> Option<char> {
    match key {
        Key::Space => Some(' '),
        Key::Character(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            (chars.next().is_none() && !c.is_control()).then_some(c)
        }
        _ => None,
    }
}

fn finish_lesson(
    mut session: ResMut<TypingSession>,
    mut save: ResMut<SaveData>,
    generator: Res<LessonGenerator>,
    mut rng: ResMut<LessonRng>,
    time: Res<Time>,
    mut last: ResMut<LastLesson>,
    mut completed: MessageWriter<LessonCompleted>,
) {
    if !session.is_finished() {
        return;
    }
    let difficulty = save.selected;
    let rules = difficulty.rules();
    let stats = session.stats(time.elapsed_secs_f64());
    let outcome = save
        .profile_mut(difficulty)
        .apply_lesson(&stats, &session.tally(), &rules);
    save::persist(&save);

    let summary = LessonSummary {
        wpm: stats.wpm,
        accuracy: stats.accuracy,
        errors: stats.errors,
        seconds: stats.seconds,
        score: outcome.score,
        unlocked: outcome.unlocked,
        best_wpm: outcome.best_wpm,
        best_score: outcome.best_score,
    };
    last.0 = Some(summary);
    *session = next_session(&save, &generator, &mut rng.0);
    completed.write(LessonCompleted { summary });
}

#[cfg(test)]
mod tests {
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::difficulty::MistakeMode;

    /// Feedback sent since the test last looked.
    #[derive(Resource, Default)]
    struct Received(Vec<FeedbackKind>);

    fn record(mut feedback: MessageReader<KeyFeedback>, mut received: ResMut<Received>) {
        received.0.extend(feedback.read().map(|f| f.kind));
    }

    /// A headless app running only the input system on a fixed lesson,
    /// before its first update.
    fn headless(text: &str, mode: MistakeMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
            .init_state::<AppState>()
            .add_message::<KeyFeedback>()
            .init_resource::<Received>()
            .insert_resource(TypingSession::new(text, mode))
            .add_systems(Update, (handle_typing, record).chain());
        app
    }

    fn app(text: &str, mode: MistakeMode) -> App {
        let mut app = headless(text, mode);
        app.update();
        app
    }

    fn press(app: &mut App, logical_key: Key, key_code: KeyCode) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn feedback(app: &mut App) -> Vec<FeedbackKind> {
        std::mem::take(&mut app.world_mut().resource_mut::<Received>().0)
    }

    fn cursor(app: &App) -> usize {
        app.world().resource::<TypingSession>().cursor()
    }

    #[test]
    fn keys_drive_the_session() {
        let mut app = app("ab", MistakeMode::StopOnError);
        press(&mut app, Key::Character("a".into()), KeyCode::KeyA);
        assert_eq!(cursor(&app), 1);
        assert_eq!(feedback(&mut app), [FeedbackKind::Hit]);

        press(&mut app, Key::Character("x".into()), KeyCode::KeyX);
        assert_eq!(cursor(&app), 1);
        assert_eq!(feedback(&mut app), [FeedbackKind::Miss]);
    }

    #[test]
    fn space_and_backspace_keys() {
        let mut app = app("a b", MistakeMode::FixWithBackspace);
        press(&mut app, Key::Character("a".into()), KeyCode::KeyA);
        press(&mut app, Key::Space, KeyCode::Space);
        assert_eq!(cursor(&app), 2);
        assert_eq!(feedback(&mut app), [FeedbackKind::Hit, FeedbackKind::Hit]);
        press(&mut app, Key::Backspace, KeyCode::Backspace);
        assert_eq!(cursor(&app), 1);
        assert_eq!(feedback(&mut app), [FeedbackKind::Backspace]);
    }

    #[test]
    fn keys_queued_before_the_lesson_are_dropped() {
        let mut app = headless("ab", MistakeMode::StopOnError);
        press(&mut app, Key::Character("a".into()), KeyCode::KeyA);
        assert_eq!(cursor(&app), 0);
        assert!(feedback(&mut app).is_empty());
    }

    #[test]
    fn key_repeats_are_ignored() {
        let mut app = app("aa", MistakeMode::StopOnError);
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character("a".into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: true,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(cursor(&app), 0);
    }

    #[test]
    fn escape_returns_to_the_menu() {
        let mut app = app("ab", MistakeMode::StopOnError);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Practice);
        app.update();
        press(&mut app, Key::Escape, KeyCode::Escape);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
    }

    #[test]
    fn typed_char_mapping() {
        assert_eq!(typed_char(&Key::Space), Some(' '));
        assert_eq!(typed_char(&Key::Character("A".into())), Some('A'));
        assert_eq!(typed_char(&Key::Character("ab".into())), None);
        assert_eq!(typed_char(&Key::Enter), None);
    }
}
