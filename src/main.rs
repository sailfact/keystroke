//! Keystroke: a touch-typing game in the style of keybr.com, built with Bevy.
//! Lessons use only the keys you've unlocked, and new keys unlock as you get
//! faster.

mod audio;
mod difficulty;
mod lesson;
mod practice;
mod progress;
mod save;
mod typing;
mod ui;

use bevy::prelude::*;
use bevy::window::WindowResizeConstraints;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Bevy enters the initial state before `Startup` runs, so the game
    /// starts here and moves to the menu once the layout exists.
    #[default]
    Loading,
    Menu,
    Practice,
}

fn show_menu(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Menu);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Keystroke".into(),
                resolution: (1280, 800).into(),
                resize_constraints: WindowResizeConstraints {
                    min_width: 640.0,
                    min_height: 400.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(Startup, show_menu)
        .add_plugins((
            save::SavePlugin,
            practice::PracticePlugin,
            audio::SfxPlugin,
            ui::UiPlugin,
        ))
        .run();
}
