//! The on-screen keyboard: a US QWERTY layout, tinted by which finger
//! types each key. It lights up held keys, outlines the next key (and the
//! Shift to use with it), flashes wrong keys, marks the focus key, and dims
//! letters that aren't unlocked yet.

use bevy::prelude::*;

use super::theme;
use crate::practice::{FeedbackKind, KeyFeedback, TypingSystems};
use crate::save::SaveData;
use crate::typing::TypingSession;

/// Width and height of a standard key, and the gap between keys.
const UNIT: f32 = 50.0;
const GAP: f32 = 5.0;
/// How long a wrongly pressed key flashes.
const FLASH_SECS: f32 = 0.3;

const KEY_HELD: Color = Color::srgb(0.85, 0.88, 0.95);
const KEY_LOCKED: Color = Color::srgb(0.085, 0.095, 0.115);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finger {
    LeftPinky,
    LeftRing,
    LeftMiddle,
    LeftIndex,
    Thumb,
    RightIndex,
    RightMiddle,
    RightRing,
    RightPinky,
}

impl Finger {
    pub fn is_left(self) -> bool {
        matches!(
            self,
            Finger::LeftPinky | Finger::LeftRing | Finger::LeftMiddle | Finger::LeftIndex
        )
    }

    /// Each finger's keys share a tint, mirrored across both hands.
    fn tint(self) -> Color {
        match self {
            Finger::LeftPinky | Finger::RightPinky => Color::srgb(0.25, 0.16, 0.21),
            Finger::LeftRing | Finger::RightRing => Color::srgb(0.25, 0.2, 0.14),
            Finger::LeftMiddle | Finger::RightMiddle => Color::srgb(0.16, 0.23, 0.16),
            Finger::LeftIndex | Finger::RightIndex => Color::srgb(0.14, 0.2, 0.27),
            Finger::Thumb => Color::srgb(0.17, 0.18, 0.21),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyDef {
    pub code: KeyCode,
    /// The characters typed without and with Shift.
    pub chars: Option<(char, char)>,
    /// Label for keys that don't type a character.
    pub label: &'static str,
    /// Width in standard key widths.
    pub width: f32,
    pub finger: Finger,
}

const fn key(code: KeyCode, plain: char, shifted: char, finger: Finger) -> KeyDef {
    KeyDef {
        code,
        chars: Some((plain, shifted)),
        label: "",
        width: 1.0,
        finger,
    }
}

const fn wide(code: KeyCode, label: &'static str, width: f32, finger: Finger) -> KeyDef {
    KeyDef {
        code,
        chars: None,
        label,
        width,
        finger,
    }
}

const LP: Finger = Finger::LeftPinky;
const LR: Finger = Finger::LeftRing;
const LM: Finger = Finger::LeftMiddle;
const LI: Finger = Finger::LeftIndex;
const TH: Finger = Finger::Thumb;
const RI: Finger = Finger::RightIndex;
const RM: Finger = Finger::RightMiddle;
const RR: Finger = Finger::RightRing;
const RP: Finger = Finger::RightPinky;

/// US ANSI layout. Every row is 15 key widths wide.
pub const ROWS: [&[KeyDef]; 5] = [
    &[
        key(KeyCode::Backquote, '`', '~', LP),
        key(KeyCode::Digit1, '1', '!', LP),
        key(KeyCode::Digit2, '2', '@', LR),
        key(KeyCode::Digit3, '3', '#', LM),
        key(KeyCode::Digit4, '4', '$', LI),
        key(KeyCode::Digit5, '5', '%', LI),
        key(KeyCode::Digit6, '6', '^', RI),
        key(KeyCode::Digit7, '7', '&', RI),
        key(KeyCode::Digit8, '8', '*', RM),
        key(KeyCode::Digit9, '9', '(', RR),
        key(KeyCode::Digit0, '0', ')', RP),
        key(KeyCode::Minus, '-', '_', RP),
        key(KeyCode::Equal, '=', '+', RP),
        wide(KeyCode::Backspace, "Backspace", 2.0, RP),
    ],
    &[
        wide(KeyCode::Tab, "Tab", 1.5, LP),
        key(KeyCode::KeyQ, 'q', 'Q', LP),
        key(KeyCode::KeyW, 'w', 'W', LR),
        key(KeyCode::KeyE, 'e', 'E', LM),
        key(KeyCode::KeyR, 'r', 'R', LI),
        key(KeyCode::KeyT, 't', 'T', LI),
        key(KeyCode::KeyY, 'y', 'Y', RI),
        key(KeyCode::KeyU, 'u', 'U', RI),
        key(KeyCode::KeyI, 'i', 'I', RM),
        key(KeyCode::KeyO, 'o', 'O', RR),
        key(KeyCode::KeyP, 'p', 'P', RP),
        key(KeyCode::BracketLeft, '[', '{', RP),
        key(KeyCode::BracketRight, ']', '}', RP),
        KeyDef {
            width: 1.5,
            ..key(KeyCode::Backslash, '\\', '|', RP)
        },
    ],
    &[
        wide(KeyCode::CapsLock, "Caps", 1.75, LP),
        key(KeyCode::KeyA, 'a', 'A', LP),
        key(KeyCode::KeyS, 's', 'S', LR),
        key(KeyCode::KeyD, 'd', 'D', LM),
        key(KeyCode::KeyF, 'f', 'F', LI),
        key(KeyCode::KeyG, 'g', 'G', LI),
        key(KeyCode::KeyH, 'h', 'H', RI),
        key(KeyCode::KeyJ, 'j', 'J', RI),
        key(KeyCode::KeyK, 'k', 'K', RM),
        key(KeyCode::KeyL, 'l', 'L', RR),
        key(KeyCode::Semicolon, ';', ':', RP),
        key(KeyCode::Quote, '\'', '"', RP),
        wide(KeyCode::Enter, "Enter", 2.25, RP),
    ],
    &[
        wide(KeyCode::ShiftLeft, "Shift", 2.25, LP),
        key(KeyCode::KeyZ, 'z', 'Z', LP),
        key(KeyCode::KeyX, 'x', 'X', LR),
        key(KeyCode::KeyC, 'c', 'C', LM),
        key(KeyCode::KeyV, 'v', 'V', LI),
        key(KeyCode::KeyB, 'b', 'B', LI),
        key(KeyCode::KeyN, 'n', 'N', RI),
        key(KeyCode::KeyM, 'm', 'M', RI),
        key(KeyCode::Comma, ',', '<', RM),
        key(KeyCode::Period, '.', '>', RR),
        key(KeyCode::Slash, '/', '?', RP),
        wide(KeyCode::ShiftRight, "Shift", 2.75, RP),
    ],
    &[
        wide(KeyCode::ControlLeft, "Ctrl", 1.25, LP),
        wide(KeyCode::SuperLeft, "Super", 1.25, TH),
        wide(KeyCode::AltLeft, "Alt", 1.25, TH),
        KeyDef {
            chars: Some((' ', ' ')),
            ..wide(KeyCode::Space, "", 6.25, TH)
        },
        wide(KeyCode::AltRight, "Alt", 1.25, TH),
        wide(KeyCode::SuperRight, "Super", 1.25, TH),
        wide(KeyCode::ContextMenu, "Menu", 1.25, RP),
        wide(KeyCode::ControlRight, "Ctrl", 1.25, RP),
    ],
];

/// The key that types a character, and the Shift to hold with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyHint {
    pub key: KeyCode,
    pub shift: Option<KeyCode>,
}

/// Finds the key for `c`. Touch typists press Shift with the other hand
/// from the key, so capitals and symbols get the opposite Shift.
pub fn key_for_char(c: char) -> Option<KeyHint> {
    ROWS.iter().flat_map(|row| row.iter()).find_map(|def| {
        let (plain, shifted) = def.chars?;
        if c == plain {
            Some(KeyHint {
                key: def.code,
                shift: None,
            })
        } else if c == shifted {
            let shift = if def.finger.is_left() {
                KeyCode::ShiftRight
            } else {
                KeyCode::ShiftLeft
            };
            Some(KeyHint {
                key: def.code,
                shift: Some(shift),
            })
        } else {
            None
        }
    })
}

pub struct KeyboardPlugin;

impl Plugin for KeyboardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyFlashes>().add_systems(
            Update,
            (flash_wrong_keys, update_keys).chain().after(TypingSystems),
        );
    }
}

#[derive(Component)]
struct Keycap(KeyDef);

#[derive(Component)]
struct KeycapLabel;

/// Shows a letter key's speed as a bar across the top of the key.
#[derive(Component)]
struct ConfidenceBar(char);

/// Keys that were pressed by mistake, with seconds left to flash.
#[derive(Resource, Default)]
struct KeyFlashes(Vec<(KeyCode, f32)>);

pub fn spawn_keyboard(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(GAP),
                padding: px(14).all(),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
        ))
        .with_children(|board| {
            for row in ROWS {
                board
                    .spawn(Node {
                        column_gap: px(GAP),
                        ..default()
                    })
                    .with_children(|keys| {
                        for &def in row {
                            spawn_key(keys, def);
                        }
                    });
            }
        });
}

fn spawn_key(parent: &mut ChildSpawnerCommands, def: KeyDef) {
    let width = def.width * UNIT + (def.width - 1.0) * GAP;
    let (text, size) = match def.chars {
        Some((plain, _)) if plain.is_ascii_lowercase() => {
            (plain.to_ascii_uppercase().to_string(), 18.0)
        }
        Some((' ', _)) => (String::new(), 14.0),
        Some((plain, shifted)) => (format!("{shifted}\n{plain}"), 13.0),
        None => (def.label.to_string(), 12.0),
    };
    parent
        .spawn((
            Keycap(def),
            Node {
                width: px(width),
                height: px(UNIT),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: px(2).all(),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(def.finger.tint()),
            BorderColor::all(Color::NONE),
        ))
        .with_children(|cap| {
            cap.spawn((
                KeycapLabel,
                Text::new(text),
                TextFont::from_font_size(size),
                TextColor(theme::TEXT),
                TextLayout::justify(Justify::Center),
            ));
            if let Some((letter, _)) = def.chars.filter(|(c, _)| c.is_ascii_lowercase()) {
                cap.spawn((
                    ConfidenceBar(letter),
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(4),
                        left: px(6),
                        height: px(3),
                        width: px(0),
                        border_radius: BorderRadius::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ));
            }
            if matches!(def.code, KeyCode::KeyF | KeyCode::KeyJ) {
                // The bumps that let touch typists find the home row.
                cap.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(5),
                        left: px((width - 4.0 - 14.0) / 2.0),
                        width: px(14),
                        height: px(3),
                        border_radius: BorderRadius::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(theme::TEXT_DIM),
                ));
            }
        });
}

fn flash_wrong_keys(
    time: Res<Time>,
    mut flashes: ResMut<KeyFlashes>,
    mut feedback: MessageReader<KeyFeedback>,
) {
    let dt = time.delta_secs();
    flashes.0.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    for f in feedback.read() {
        if matches!(f.kind, FeedbackKind::Miss | FeedbackKind::Blocked) {
            flashes.0.push((f.key, FLASH_SECS));
        }
    }
}

fn update_keys(
    input: Res<ButtonInput<KeyCode>>,
    flashes: Res<KeyFlashes>,
    save: Res<SaveData>,
    session: Option<Res<TypingSession>>,
    mut caps: Query<(&Keycap, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut labels: Query<&mut TextColor, With<KeycapLabel>>,
    mut bars: Query<(&ConfidenceBar, &mut Node, &mut BackgroundColor), Without<Keycap>>,
) {
    let target = save.selected.rules().target_wpm;
    let profile = save.current();
    let focus = profile.focus_key(target);
    // With a mistake left to fix, Backspace is the key to press.
    let hint = session.as_ref().and_then(|s| {
        if s.word_has_error() {
            Some(KeyHint {
                key: KeyCode::Backspace,
                shift: None,
            })
        } else {
            s.expected().and_then(key_for_char)
        }
    });

    for (Keycap(def), mut background, mut border, children) in &mut caps {
        let letter = def
            .chars
            .map(|(plain, _)| plain)
            .filter(char::is_ascii_lowercase);
        let locked = letter.is_some_and(|c| !profile.is_unlocked(c));
        let flashing = flashes.0.iter().any(|&(code, _)| code == def.code);
        let held = input.pressed(def.code);
        let hinted = hint.is_some_and(|h| h.key == def.code || h.shift == Some(def.code));

        let (fill, text) = if flashing {
            (theme::BAD, theme::BACKGROUND)
        } else if held {
            (KEY_HELD, theme::BACKGROUND)
        } else if locked {
            (KEY_LOCKED, theme::TEXT_FAINT)
        } else {
            (def.finger.tint(), theme::TEXT)
        };
        let edge = if hinted {
            theme::ACCENT
        } else if letter.is_some() && letter == focus {
            theme::FOCUS
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

    for (ConfidenceBar(letter), mut node, mut background) in &mut bars {
        // No bar until the key has been timed at least once.
        let stats = profile.key(*letter);
        let (width, color) = if profile.is_unlocked(*letter) && stats.best_ms.is_some() {
            let confidence = stats.confidence(target);
            let width = confidence.clamp(0.05, 1.0) * (UNIT - 16.0);
            (width, theme::confidence(confidence, 1.0))
        } else {
            (0.0, Color::NONE)
        };
        if node.width != px(width) {
            node.width = px(width);
        }
        background.set_if_neq(BackgroundColor(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::PUNCTUATION;

    #[test]
    fn every_row_is_fifteen_units_wide() {
        for row in ROWS {
            let width: f32 = row.iter().map(|def| def.width).sum();
            assert!((width - 15.0).abs() < 1e-6, "row is {width} wide");
        }
    }

    #[test]
    fn every_lesson_character_has_a_key() {
        let lowercase = 'a'..='z';
        let uppercase = 'A'..='Z';
        let punctuation = PUNCTUATION.iter().map(|&(mark, _)| mark);
        for c in lowercase.chain(uppercase).chain(punctuation).chain([' ']) {
            assert!(key_for_char(c).is_some(), "no key for {c:?}");
        }
    }

    #[test]
    fn shift_is_on_the_other_hand() {
        let shift = |c| key_for_char(c).unwrap().shift;
        assert_eq!(shift('a'), None);
        assert_eq!(shift('A'), Some(KeyCode::ShiftRight));
        assert_eq!(shift('P'), Some(KeyCode::ShiftLeft));
        assert_eq!(shift('!'), Some(KeyCode::ShiftRight));
        assert_eq!(shift('"'), Some(KeyCode::ShiftLeft));
        assert_eq!(shift(':'), Some(KeyCode::ShiftLeft));
        assert_eq!(key_for_char(' ').unwrap().key, KeyCode::Space);
    }
}
