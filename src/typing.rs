//! The typing model for one lesson: cursor movement, mistakes, and the
//! measurements taken along the way (speed, accuracy, per-key timing).

use bevy::prelude::Resource;

use crate::difficulty::MistakeMode;

/// Gaps longer than this (in seconds) are pauses, not typing speed, so they
/// never become per-key timing samples.
pub const MAX_SAMPLE_GAP: f64 = 2.0;

/// Shortest duration used for speed, so the first keystrokes of a lesson
/// can't produce absurd numbers.
const MIN_ELAPSED: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharState {
    Pending,
    /// Typed right on the first try.
    Correct,
    /// Typed right after at least one miss.
    Recovered,
    /// Typed wrong and not fixed yet. Only happens in
    /// [`MistakeMode::FixWithBackspace`].
    Wrong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    Hit,
    Miss,
    /// Refused because the current word still has a mistake to fix. Counts
    /// against accuracy.
    Blocked,
    /// The lesson is already finished.
    Ignored,
}

/// One letter's results for a lesson.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct KeyTally {
    /// Sum of the timing samples, in milliseconds.
    pub total_ms: f32,
    pub samples: u32,
    /// Positions typed right on the first try.
    pub hits: u32,
    /// Positions missed at least once.
    pub misses: u32,
}

impl KeyTally {
    pub fn mean_ms(&self) -> Option<f32> {
        (self.samples > 0).then(|| self.total_ms / self.samples as f32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonStats {
    pub wpm: f32,
    pub accuracy: f32,
    pub errors: u32,
    pub seconds: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct TypingSession {
    text: Vec<char>,
    states: Vec<CharState>,
    /// Whether each position has been missed at least once.
    missed: Vec<bool>,
    cursor: usize,
    mode: MistakeMode,
    keystrokes: u32,
    correct_keystrokes: u32,
    started_at: Option<f64>,
    finished_at: Option<f64>,
    /// Time of the last keystroke that moved the cursor.
    last_key_at: Option<f64>,
    /// Whether the character before the cursor was typed cleanly, with no
    /// miss or Backspace since. Only then is the next letter's timing kept.
    prev_clean: bool,
    /// Per-letter timing samples: (total ms, count).
    timing: [(f32, u32); 26],
}

impl TypingSession {
    pub fn new(text: &str, mode: MistakeMode) -> Self {
        let text: Vec<char> = text.chars().collect();
        let len = text.len();
        Self {
            text,
            states: vec![CharState::Pending; len],
            missed: vec![false; len],
            cursor: 0,
            mode,
            keystrokes: 0,
            correct_keystrokes: 0,
            started_at: None,
            finished_at: None,
            last_key_at: None,
            prev_clean: false,
            timing: [(0.0, 0); 26],
        }
    }

    pub fn text(&self) -> &[char] {
        &self.text
    }

    pub fn states(&self) -> &[CharState] {
        &self.states
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_started(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.finished_at.is_some()
    }

    /// The character the player should type next.
    pub fn expected(&self) -> Option<char> {
        if self.is_finished() {
            None
        } else {
            self.text.get(self.cursor).copied()
        }
    }

    /// Whether the word being typed contains a mistake that still needs
    /// Backspace. Always false in [`MistakeMode::StopOnError`].
    pub fn word_has_error(&self) -> bool {
        for i in (0..self.cursor).rev() {
            match self.states[i] {
                CharState::Wrong => return true,
                _ if self.text[i] == ' ' => return false,
                _ => {}
            }
        }
        false
    }

    pub fn type_char(&mut self, c: char, now: f64) -> KeyResult {
        if self.is_finished() {
            return KeyResult::Ignored;
        }
        self.started_at.get_or_insert(now);
        self.keystrokes += 1;

        // Past the end is only reachable when a mistake is left to fix.
        let Some(&expected) = self.text.get(self.cursor) else {
            return KeyResult::Blocked;
        };
        if self.mode == MistakeMode::FixWithBackspace && expected == ' ' && self.word_has_error() {
            return KeyResult::Blocked;
        }

        if c == expected {
            self.hit(expected, now);
            KeyResult::Hit
        } else {
            self.miss(now);
            KeyResult::Miss
        }
    }

    fn hit(&mut self, expected: char, now: f64) {
        let i = self.cursor;
        let clean = !self.missed[i];
        self.correct_keystrokes += 1;
        self.states[i] = if clean {
            CharState::Correct
        } else {
            CharState::Recovered
        };

        if clean
            && self.prev_clean
            && let (Some(slot), Some(last)) = (letter_slot(expected), self.last_key_at)
        {
            let gap = now - last;
            if gap <= MAX_SAMPLE_GAP {
                self.timing[slot].0 += (gap * 1000.0) as f32;
                self.timing[slot].1 += 1;
            }
        }

        self.prev_clean = clean;
        self.last_key_at = Some(now);
        self.cursor += 1;
        if self.cursor == self.text.len() && !self.states.contains(&CharState::Wrong) {
            self.finished_at = Some(now);
        }
    }

    fn miss(&mut self, now: f64) {
        let i = self.cursor;
        self.missed[i] = true;
        self.prev_clean = false;
        if self.mode == MistakeMode::FixWithBackspace {
            self.states[i] = CharState::Wrong;
            self.last_key_at = Some(now);
            self.cursor += 1;
        }
    }

    /// Deletes the last typed character. Returns false when Backspace does
    /// nothing, which is always the case in [`MistakeMode::StopOnError`].
    pub fn backspace(&mut self, now: f64) -> bool {
        if self.mode != MistakeMode::FixWithBackspace || self.is_finished() || self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.states[self.cursor] = CharState::Pending;
        self.prev_clean = false;
        self.last_key_at = Some(now);
        true
    }

    /// Seconds from the first keystroke to `now`, or to the finish.
    pub fn elapsed(&self, now: f64) -> f64 {
        match self.started_at {
            Some(start) => (self.finished_at.unwrap_or(now) - start).max(0.0),
            None => 0.0,
        }
    }

    pub fn correct_chars(&self) -> usize {
        self.states
            .iter()
            .filter(|s| matches!(s, CharState::Correct | CharState::Recovered))
            .count()
    }

    /// Words per minute, counting five correct characters as a word.
    pub fn wpm(&self, now: f64) -> f32 {
        if !self.is_started() {
            return 0.0;
        }
        let minutes = self.elapsed(now).max(MIN_ELAPSED) / 60.0;
        (self.correct_chars() as f64 / 5.0 / minutes) as f32
    }

    /// Share of character keystrokes that were right. Backspace isn't a
    /// character keystroke.
    pub fn accuracy(&self) -> f32 {
        if self.keystrokes == 0 {
            1.0
        } else {
            self.correct_keystrokes as f32 / self.keystrokes as f32
        }
    }

    pub fn errors(&self) -> u32 {
        self.keystrokes - self.correct_keystrokes
    }

    pub fn progress(&self) -> f32 {
        if self.text.is_empty() {
            1.0
        } else {
            self.cursor as f32 / self.text.len() as f32
        }
    }

    pub fn stats(&self, now: f64) -> LessonStats {
        LessonStats {
            wpm: self.wpm(now),
            accuracy: self.accuracy(),
            errors: self.errors(),
            seconds: self.elapsed(now) as f32,
        }
    }

    /// Per-letter results, indexed `a` to `z`.
    pub fn tally(&self) -> [KeyTally; 26] {
        let mut tally = [KeyTally::default(); 26];
        for (slot, &(total_ms, samples)) in self.timing.iter().enumerate() {
            tally[slot].total_ms = total_ms;
            tally[slot].samples = samples;
        }
        for i in 0..self.text.len() {
            let Some(slot) = letter_slot(self.text[i]) else {
                continue;
            };
            if self.missed[i] {
                tally[slot].misses += 1;
            } else if self.states[i] == CharState::Correct {
                tally[slot].hits += 1;
            }
        }
        tally
    }
}

/// Index of a lowercase letter, `a` = 0. Capitals and punctuation have no
/// slot and are never tracked per key.
pub fn letter_slot(c: char) -> Option<usize> {
    c.is_ascii_lowercase().then(|| (c as u8 - b'a') as usize)
}

/// Score for a lesson. Accuracy is squared so mistakes cost more than slow
/// typing, and more unlocked keys make every lesson worth more.
pub fn lesson_score(wpm: f32, accuracy: f32, keys: usize, multiplier: f32) -> u32 {
    (wpm * accuracy * accuracy * keys as f32 * multiplier)
        .round()
        .max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(text: &str) -> TypingSession {
        TypingSession::new(text, MistakeMode::StopOnError)
    }

    fn fix(text: &str) -> TypingSession {
        TypingSession::new(text, MistakeMode::FixWithBackspace)
    }

    #[test]
    fn stop_mode_hit_advances() {
        let mut s = stop("ab");
        assert_eq!(s.type_char('a', 0.0), KeyResult::Hit);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.states()[0], CharState::Correct);
        assert_eq!(s.expected(), Some('b'));
    }

    #[test]
    fn stop_mode_miss_holds_cursor() {
        let mut s = stop("ab");
        assert_eq!(s.type_char('x', 0.0), KeyResult::Miss);
        assert_eq!(s.cursor(), 0);
        assert_eq!(s.states()[0], CharState::Pending);
        assert_eq!(s.type_char('a', 0.1), KeyResult::Hit);
        assert_eq!(s.states()[0], CharState::Recovered);
    }

    #[test]
    fn stop_mode_ignores_backspace() {
        let mut s = stop("ab");
        s.type_char('a', 0.0);
        assert!(!s.backspace(0.1));
        assert_eq!(s.cursor(), 1);
    }

    #[test]
    fn stop_mode_finishes_on_last_char() {
        let mut s = stop("ab");
        s.type_char('a', 0.0);
        s.type_char('b', 0.2);
        assert!(s.is_finished());
        assert_eq!(s.expected(), None);
        assert_eq!(s.type_char('c', 0.3), KeyResult::Ignored);
    }

    #[test]
    fn fix_mode_miss_advances_and_marks_wrong() {
        let mut s = fix("abc");
        assert_eq!(s.type_char('x', 0.0), KeyResult::Miss);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.states()[0], CharState::Wrong);
        assert!(s.word_has_error());
    }

    #[test]
    fn fix_mode_backspace_then_fix_marks_recovered() {
        let mut s = fix("ab");
        s.type_char('x', 0.0);
        assert!(s.backspace(0.1));
        assert_eq!(s.cursor(), 0);
        assert_eq!(s.states()[0], CharState::Pending);
        assert_eq!(s.type_char('a', 0.2), KeyResult::Hit);
        assert_eq!(s.states()[0], CharState::Recovered);
        assert!(!s.word_has_error());
    }

    #[test]
    fn fix_mode_blocks_space_while_word_has_error() {
        let mut s = fix("ab cd");
        s.type_char('a', 0.0);
        s.type_char('x', 0.1);
        assert_eq!(s.type_char(' ', 0.2), KeyResult::Blocked);
        assert_eq!(s.cursor(), 2);
        s.backspace(0.3);
        s.type_char('b', 0.4);
        assert_eq!(s.type_char(' ', 0.5), KeyResult::Hit);
        assert_eq!(s.cursor(), 3);
    }

    #[test]
    fn fix_mode_wrong_space_counts_as_word_error() {
        let mut s = fix("ab cd");
        s.type_char('a', 0.0);
        s.type_char('b', 0.1);
        s.type_char('x', 0.2);
        assert_eq!(s.states()[2], CharState::Wrong);
        s.type_char('c', 0.3);
        s.type_char('d', 0.4);
        assert!(s.word_has_error());
    }

    #[test]
    fn fix_mode_completion_requires_no_wrong_chars() {
        let mut s = fix("ab");
        s.type_char('a', 0.0);
        s.type_char('x', 0.1);
        assert!(!s.is_finished());
        assert_eq!(s.type_char('b', 0.2), KeyResult::Blocked);
        s.backspace(0.3);
        s.type_char('b', 0.4);
        assert!(s.is_finished());
    }

    #[test]
    fn accuracy_counts_every_character_keystroke() {
        let mut s = stop("ab");
        s.type_char('a', 0.0);
        s.type_char('x', 0.1);
        s.type_char('y', 0.2);
        s.type_char('b', 0.3);
        assert_eq!(s.errors(), 2);
        assert!((s.accuracy() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wpm_uses_time_since_first_keystroke() {
        let text = "hello world";
        let mut s = stop(text);
        for (i, c) in text.chars().enumerate() {
            s.type_char(c, 10.0 + i as f64 * 0.6);
        }
        // 11 characters over 6 seconds: 2.2 words in 0.1 minutes.
        assert!((s.elapsed(100.0) - 6.0).abs() < 1e-9);
        assert!((s.wpm(100.0) - 22.0).abs() < 1e-3);
    }

    #[test]
    fn timing_samples_skip_misses_and_pauses() {
        let mut s = stop("abcde");
        s.type_char('a', 0.0); // first character: no sample
        s.type_char('b', 0.2); // 200 ms sample
        s.type_char('x', 0.3); // miss on c
        s.type_char('c', 0.4); // recovered: no sample
        s.type_char('d', 0.6); // previous char wasn't clean: no sample
        s.type_char('e', 3.0); // pause over 2 s: no sample

        let tally = s.tally();
        let slot = |c| letter_slot(c).unwrap();
        assert_eq!(tally[slot('b')].samples, 1);
        assert!((tally[slot('b')].mean_ms().unwrap() - 200.0).abs() < 1e-3);
        assert_eq!(tally[slot('c')].samples, 0);
        assert_eq!(tally[slot('c')].misses, 1);
        assert_eq!(tally[slot('d')].samples, 0);
        assert_eq!(tally[slot('d')].hits, 1);
        assert_eq!(tally[slot('e')].samples, 0);
    }

    #[test]
    fn capitals_and_punctuation_are_not_tracked() {
        let mut s = stop("Ab,");
        s.type_char('A', 0.0);
        s.type_char('b', 0.1);
        s.type_char(',', 0.2);
        let tally = s.tally();
        assert_eq!(tally[0].hits, 0);
        assert_eq!(tally[1].hits, 1);
    }

    #[test]
    fn score_formula() {
        assert_eq!(lesson_score(40.0, 0.9, 10, 2.0), 648);
        assert_eq!(lesson_score(0.0, 1.0, 26, 3.0), 0);
    }
}
