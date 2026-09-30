//! Difficulty levels and the rules each one plays by.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
}

/// What happens when the player presses the wrong key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MistakeMode {
    /// The cursor waits on the character until the right key is pressed.
    StopOnError,
    /// The cursor moves on and the character is marked wrong. Mistakes must be
    /// fixed with Backspace before the cursor can leave the word.
    FixWithBackspace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    /// Every unlocked key must reach this speed before the next key unlocks.
    pub target_wpm: f32,
    pub mistakes: MistakeMode,
    /// Chance that a word is capitalised.
    pub capitals: f32,
    /// Chance that a word gets punctuation.
    pub punctuation: f32,
    pub score_multiplier: f32,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard];

    pub fn rules(self) -> Rules {
        match self {
            Difficulty::Easy => Rules {
                target_wpm: 25.0,
                mistakes: MistakeMode::StopOnError,
                capitals: 0.0,
                punctuation: 0.0,
                score_multiplier: 1.0,
            },
            Difficulty::Medium => Rules {
                target_wpm: 35.0,
                mistakes: MistakeMode::StopOnError,
                capitals: 0.0,
                punctuation: 0.0,
                score_multiplier: 2.0,
            },
            Difficulty::Hard => Rules {
                target_wpm: 50.0,
                mistakes: MistakeMode::FixWithBackspace,
                capitals: 0.2,
                punctuation: 0.25,
                score_multiplier: 3.0,
            },
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Easy => {
                "New keys unlock at 25 WPM.\nLowercase words.\nThe cursor waits for the right key."
            }
            Difficulty::Medium => {
                "New keys unlock at 35 WPM.\nLowercase words.\nThe cursor waits for the right key."
            }
            Difficulty::Hard => {
                "New keys unlock at 50 WPM.\nCapitals and punctuation.\nFix mistakes with Backspace."
            }
        }
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + 2) % 3]
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % 3]
    }
}
