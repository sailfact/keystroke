//! Screens and widgets. The root layout is a screen area above the
//! on-screen keyboard, which stays visible in every state.

pub mod hud;
pub mod keyboard;
pub mod menu;
pub mod passage;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(theme::BACKGROUND))
            .add_systems(Startup, spawn_layout)
            .add_systems(Update, fit_to_window)
            .add_plugins((
                keyboard::KeyboardPlugin,
                menu::MenuPlugin,
                hud::HudPlugin,
                passage::PassagePlugin,
            ));
    }
}

/// Screens put their content under this entity.
#[derive(Component)]
pub struct ScreenRoot;

/// The layout is designed for this window size and scaled to fit others.
const DESIGN_SIZE: Vec2 = Vec2::new(1280.0, 800.0);

fn spawn_layout(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: px(24).all(),
            row_gap: px(20),
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                ScreenRoot,
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            keyboard::spawn_keyboard(root);
        });
}

/// Scales the whole UI so the layout fits the window, however the window
/// manager sized it.
fn fit_to_window(window: Single<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let size = window.size();
    let fit = (size.x / DESIGN_SIZE.x)
        .min(size.y / DESIGN_SIZE.y)
        .clamp(0.5, 2.0);
    if (scale.0 - fit).abs() > 0.001 {
        scale.0 = fit;
    }
}

/// A text node.
pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

/// Formats a whole number with thousands separators.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub mod theme {
    use bevy::prelude::Color;

    pub const BACKGROUND: Color = Color::srgb(0.067, 0.075, 0.094);
    pub const PANEL: Color = Color::srgb(0.106, 0.118, 0.145);
    pub const PANEL_LIGHT: Color = Color::srgb(0.15, 0.165, 0.2);
    pub const BORDER: Color = Color::srgb(0.22, 0.24, 0.29);
    pub const TEXT: Color = Color::srgb(0.9, 0.91, 0.93);
    pub const TEXT_DIM: Color = Color::srgb(0.5, 0.53, 0.59);
    pub const TEXT_FAINT: Color = Color::srgb(0.3, 0.32, 0.37);
    pub const ACCENT: Color = Color::srgb(0.38, 0.68, 1.0);
    pub const FOCUS: Color = Color::srgb(1.0, 0.63, 0.28);
    pub const GOOD: Color = Color::srgb(0.38, 0.82, 0.5);
    pub const WARN: Color = Color::srgb(0.93, 0.7, 0.3);
    pub const BAD: Color = Color::srgb(0.96, 0.36, 0.36);
    pub const BAD_BACKGROUND: Color = Color::srgba(0.96, 0.36, 0.36, 0.3);
    pub const CURSOR_BACKGROUND: Color = Color::srgba(0.38, 0.68, 1.0, 0.3);

    const BAD_RGB: [f32; 3] = [0.96, 0.36, 0.36];
    const WARN_RGB: [f32; 3] = [0.93, 0.7, 0.3];
    const GOOD_RGB: [f32; 3] = [0.38, 0.82, 0.5];

    /// Red at 0, amber at 0.5, green at 1 or more.
    pub fn confidence(confidence: f32, alpha: f32) -> Color {
        let t = confidence.clamp(0.0, 1.0);
        let (from, to, t) = if t < 0.5 {
            (BAD_RGB, WARN_RGB, t * 2.0)
        } else {
            (WARN_RGB, GOOD_RGB, (t - 0.5) * 2.0)
        };
        let mix = |i: usize| from[i] + (to[i] - from[i]) * t;
        Color::srgba(mix(0), mix(1), mix(2), alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::thousands;

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
