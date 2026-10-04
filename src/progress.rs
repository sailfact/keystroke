//! Long-term progress: per-key speed, which keys are unlocked, and totals.
//!
//! The unlock rules follow keybr.com's guided lessons: start with six
//! letters, and add the next one once every unlocked letter has reached
//! [`TARGET_WPM`]. The weakest unlocked letter is the "focus" key, which the
//! lesson generator puts in every word.

use serde::{Deserialize, Serialize};

use crate::typing::{KeyTally, LessonStats, lesson_score, letter_slot};

/// Letters in the order they unlock, roughly by how common they are in
/// English words.
pub const LETTER_ORDER: [char; 26] = [
    'e', 'n', 'i', 't', 'r', 'l', 's', 'a', 'u', 'o', 'd', 'y', 'c', 'h', 'g', 'm', 'p', 'b', 'k',
    'v', 'w', 'f', 'z', 'x', 'q', 'j',
];

pub const STARTING_KEYS: usize = 6;

/// Every unlocked key must reach this speed before the next key unlocks.
/// keybr.com's default.
pub const TARGET_WPM: f32 = 35.0;

/// How far one lesson moves a key's average speed.
pub const SMOOTHING: f32 = 0.25;

/// Converts a time per character (ms) to words per minute.
pub fn ms_to_wpm(ms: f32) -> f32 {
    12_000.0 / ms
}

/// Converts words per minute to a time per character (ms).
pub fn wpm_to_ms(wpm: f32) -> f32 {
    12_000.0 / wpm
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyStats {
    /// Smoothed time to type the key, in milliseconds.
    pub avg_ms: Option<f32>,
    /// Fastest smoothed time reached so far.
    pub best_ms: Option<f32>,
    pub hits: u32,
    pub misses: u32,
}

impl KeyStats {
    /// Folds one lesson's mean time into the average.
    pub fn add_sample(&mut self, ms: f32) {
        let avg = match self.avg_ms {
            Some(prev) => prev + SMOOTHING * (ms - prev),
            None => ms,
        };
        self.avg_ms = Some(avg);
        self.best_ms = Some(self.best_ms.map_or(avg, |best| best.min(avg)));
    }

    pub fn wpm(&self) -> Option<f32> {
        self.avg_ms.map(ms_to_wpm)
    }

    /// 1.0 or more means the key has reached the target speed. Keys with no
    /// data are 0.
    pub fn confidence(&self) -> f32 {
        self.best_ms
            .map_or(0.0, |best| wpm_to_ms(TARGET_WPM) / best)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// How many letters of [`LETTER_ORDER`] are unlocked.
    pub unlocked: usize,
    /// Indexed `a` to `z`.
    pub keys: [KeyStats; 26],
    pub lessons: u32,
    pub total_score: u64,
    pub best_score: u32,
    pub best_wpm: f32,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            unlocked: STARTING_KEYS,
            keys: Default::default(),
            lessons: 0,
            total_score: 0,
            best_score: 0,
            best_wpm: 0.0,
        }
    }
}

/// What a finished lesson changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonOutcome {
    pub score: u32,
    pub unlocked: Option<char>,
    pub best_wpm: bool,
    pub best_score: bool,
}

impl Profile {
    pub fn unlocked_letters(&self) -> &[char] {
        &LETTER_ORDER[..self.unlocked.clamp(STARTING_KEYS, LETTER_ORDER.len())]
    }

    pub fn is_unlocked(&self, c: char) -> bool {
        self.unlocked_letters().contains(&c)
    }

    /// The letter that unlocks next, if any are left.
    pub fn next_letter(&self) -> Option<char> {
        LETTER_ORDER.get(self.unlocked_letters().len()).copied()
    }

    /// Stats for a lowercase letter.
    pub fn key(&self, c: char) -> &KeyStats {
        &self.keys[letter_slot(c).expect("key stats exist only for a-z")]
    }

    /// The weakest unlocked letter still below the target, or `None` once
    /// every unlocked letter is fast enough. Letters with no data come first.
    pub fn focus_key(&self) -> Option<char> {
        self.unlocked_letters()
            .iter()
            .map(|&c| (c, self.key(c).confidence()))
            .filter(|&(_, confidence)| confidence < 1.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(c, _)| c)
    }

    /// How many unlocked letters have reached the target speed.
    pub fn confident_keys(&self) -> usize {
        self.unlocked_letters()
            .iter()
            .filter(|&&c| self.key(c).confidence() >= 1.0)
            .count()
    }

    /// Records a finished lesson and unlocks the next letter if every
    /// unlocked letter is now fast enough. At most one letter unlocks per
    /// lesson, and unlocked letters stay unlocked.
    pub fn apply_lesson(&mut self, stats: &LessonStats, tally: &[KeyTally; 26]) -> LessonOutcome {
        for (key, lesson) in self.keys.iter_mut().zip(tally) {
            key.hits += lesson.hits;
            key.misses += lesson.misses;
            if let Some(ms) = lesson.mean_ms() {
                key.add_sample(ms);
            }
        }

        let score = lesson_score(stats.wpm, stats.accuracy, self.unlocked_letters().len());
        self.lessons += 1;
        self.total_score += u64::from(score);
        let best_score = score > self.best_score;
        if best_score {
            self.best_score = score;
        }
        let best_wpm = stats.wpm > self.best_wpm;
        if best_wpm {
            self.best_wpm = stats.wpm;
        }

        let unlocked = self
            .next_letter()
            .filter(|_| self.confident_keys() == self.unlocked_letters().len());
        if unlocked.is_some() {
            self.unlocked = self.unlocked_letters().len() + 1;
        }

        LessonOutcome {
            score,
            unlocked,
            best_wpm,
            best_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(wpm: f32) -> LessonStats {
        LessonStats {
            wpm,
            accuracy: 1.0,
            errors: 0,
            seconds: 30.0,
        }
    }

    /// A tally where each listed letter took `ms` per keystroke.
    fn tally(letters: &[char], ms: f32) -> [KeyTally; 26] {
        let mut tally = [KeyTally::default(); 26];
        for &c in letters {
            tally[letter_slot(c).unwrap()] = KeyTally {
                total_ms: ms * 4.0,
                samples: 4,
                hits: 4,
                misses: 0,
            };
        }
        tally
    }

    #[test]
    fn first_sample_seeds_the_average() {
        let mut key = KeyStats::default();
        key.add_sample(400.0);
        assert_eq!(key.avg_ms, Some(400.0));
        key.add_sample(200.0);
        assert_eq!(key.avg_ms, Some(350.0));
    }

    #[test]
    fn best_time_never_gets_worse() {
        let mut key = KeyStats::default();
        key.add_sample(300.0);
        key.add_sample(500.0);
        assert_eq!(key.avg_ms, Some(350.0));
        assert_eq!(key.best_ms, Some(300.0));
    }

    #[test]
    fn confidence_compares_best_time_to_target() {
        let mut key = KeyStats::default();
        assert_eq!(key.confidence(), 0.0);
        key.add_sample(wpm_to_ms(TARGET_WPM) / 2.0);
        assert!((key.confidence() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn unlock_needs_every_key_at_target() {
        let mut profile = Profile::default();
        let fast = wpm_to_ms(TARGET_WPM) * 0.8;
        let start = &LETTER_ORDER[..STARTING_KEYS];

        let outcome = profile.apply_lesson(&stats(40.0), &tally(&start[..5], fast));
        assert_eq!(outcome.unlocked, None);
        assert_eq!(profile.unlocked, STARTING_KEYS);

        let outcome = profile.apply_lesson(&stats(40.0), &tally(start, fast));
        assert_eq!(outcome.unlocked, Some('s'));
        assert_eq!(profile.unlocked, STARTING_KEYS + 1);
        assert!(profile.is_unlocked('s'));
    }

    #[test]
    fn unlocks_one_key_per_lesson() {
        let mut profile = Profile::default();
        let fast = tally(&LETTER_ORDER, wpm_to_ms(TARGET_WPM) * 0.5);
        for n in 1..=3 {
            profile.apply_lesson(&stats(60.0), &fast);
            assert_eq!(profile.unlocked, STARTING_KEYS + n);
        }
    }

    #[test]
    fn stops_unlocking_after_the_last_letter() {
        let mut profile = Profile {
            unlocked: LETTER_ORDER.len(),
            ..Profile::default()
        };
        let fast = tally(&LETTER_ORDER, wpm_to_ms(TARGET_WPM) * 0.5);
        let outcome = profile.apply_lesson(&stats(60.0), &fast);
        assert_eq!(outcome.unlocked, None);
        assert_eq!(profile.next_letter(), None);
    }

    #[test]
    fn focus_is_the_weakest_key_and_untested_keys_come_first() {
        let mut profile = Profile::default();
        assert_eq!(profile.focus_key(), Some('e'));

        let target_ms = wpm_to_ms(TARGET_WPM);
        let mut lesson = tally(&['e', 'n', 'i', 't', 'l'], target_ms * 0.9);
        lesson[letter_slot('t').unwrap()].total_ms = target_ms * 1.5 * 4.0;
        profile.apply_lesson(&stats(30.0), &lesson);
        // 'r' has no data yet, so it beats the slow 't'.
        assert_eq!(profile.focus_key(), Some('r'));

        profile.apply_lesson(&stats(30.0), &tally(&['r'], target_ms * 0.9));
        assert_eq!(profile.focus_key(), Some('t'));
    }

    #[test]
    fn no_focus_once_every_key_is_fast() {
        let mut profile = Profile::default();
        let start = &LETTER_ORDER[..STARTING_KEYS];
        profile.keys = Default::default();
        for &c in start {
            profile.keys[letter_slot(c).unwrap()].add_sample(100.0);
        }
        assert_eq!(profile.focus_key(), None);
        assert_eq!(profile.confident_keys(), STARTING_KEYS);
    }

    #[test]
    fn lesson_totals_and_bests() {
        let mut profile = Profile::default();
        let empty = [KeyTally::default(); 26];
        let first = profile.apply_lesson(&stats(30.0), &empty);
        assert!(first.best_wpm && first.best_score);
        assert_eq!(first.score, lesson_score(30.0, 1.0, STARTING_KEYS));

        let second = profile.apply_lesson(&stats(20.0), &empty);
        assert!(!second.best_wpm && !second.best_score);
        assert_eq!(profile.lessons, 2);
        assert_eq!(
            profile.total_score,
            u64::from(first.score) + u64::from(second.score)
        );
        assert_eq!(profile.best_wpm, 30.0);
    }

    #[test]
    fn letter_order_is_the_whole_alphabet() {
        let mut sorted = LETTER_ORDER;
        sorted.sort();
        assert_eq!(
            sorted.iter().collect::<String>(),
            "abcdefghijklmnopqrstuvwxyz"
        );
    }
}
