//! Lesson text: pseudo-random words built only from the unlocked letters,
//! with the focus letter in every word. Real words are used when enough of
//! them fit, and invented ones fill the gaps (as keybr.com does).

pub mod model;

use bevy::prelude::Resource;
use fastrand::Rng;

use crate::difficulty::Rules;
use model::{LetterSet, MIN_WORD_LEN, PhoneticModel};

/// Letters per lesson, not counting spaces or punctuation.
pub const LESSON_LETTERS: usize = 100;

/// When fewer real words than this fit the unlocked letters, invented words
/// fill the pool.
const MIN_POOL: usize = 15;

/// Punctuation that Hard mode adds, with relative weights.
pub const PUNCTUATION: [(char, u32); 9] = [
    (',', 30),
    ('.', 25),
    ('!', 8),
    ('?', 8),
    ('-', 7),
    ('\'', 6),
    ('"', 6),
    (';', 5),
    (':', 5),
];

/// Common English words, written for this project. They train the phonetic
/// model and double as the real-word dictionary.
const WORDS: &str = include_str!("words_en.txt");

#[derive(Resource)]
pub struct LessonGenerator {
    model: PhoneticModel,
    dictionary: Vec<&'static str>,
}

impl Default for LessonGenerator {
    fn default() -> Self {
        let dictionary: Vec<&'static str> = WORDS.split_whitespace().collect();
        let model = PhoneticModel::train(dictionary.iter().copied());
        Self { model, dictionary }
    }
}

impl LessonGenerator {
    /// Builds a lesson from `letters`, with `focus` in every word.
    pub fn generate(
        &self,
        letters: &[char],
        focus: Option<char>,
        rules: &Rules,
        rng: &mut Rng,
    ) -> String {
        let pool = self.word_pool(letters, focus, rng);
        let mut words = Vec::new();
        let mut letter_count = 0;
        let mut previous = None;
        while letter_count < LESSON_LETTERS {
            let mut pick = rng.usize(..pool.len());
            if pool.len() > 1 && Some(pick) == previous {
                // Shift to any other word so the same word never repeats.
                pick = (pick + 1 + rng.usize(..pool.len() - 1)) % pool.len();
            }
            previous = Some(pick);
            letter_count += pool[pick].len();
            let (word, extra_letters) = decorate(&pool[pick], &pool, rules, rng);
            letter_count += extra_letters;
            words.push(word);
        }
        words.join(" ")
    }

    fn word_pool(&self, letters: &[char], focus: Option<char>, rng: &mut Rng) -> Vec<String> {
        let allowed = LetterSet::from_letters(letters);
        let mut pool: Vec<String> = self
            .dictionary
            .iter()
            .filter(|word| {
                word.len() >= MIN_WORD_LEN
                    && allowed.contains_word(word)
                    && focus.is_none_or(|f| word.contains(f))
            })
            .map(|word| word.to_string())
            .collect();

        for _ in 0..MIN_POOL * 10 {
            if pool.len() >= MIN_POOL {
                break;
            }
            if let Some(word) = self.model.word(allowed, focus, rng)
                && !pool.contains(&word)
            {
                pool.push(word);
            }
        }
        // Last resort, in case the model can't make enough words.
        while pool.len() < MIN_POOL {
            pool.push(random_word(letters, focus, rng));
        }
        pool
    }
}

/// Applies Hard mode's capitals and punctuation to a word. Returns the
/// word and any letters it gained (a hyphen joins a second word).
fn decorate(word: &str, pool: &[String], rules: &Rules, rng: &mut Rng) -> (String, usize) {
    let mut word = word.to_string();
    let mut extra_letters = 0;
    if rules.capitals > 0.0 && rng.f32() < rules.capitals {
        word[..1].make_ascii_uppercase();
    }
    if rules.punctuation > 0.0 && rng.f32() < rules.punctuation {
        match pick_punctuation(rng) {
            '\'' => word = format!("'{word}'"),
            '"' => word = format!("\"{word}\""),
            '-' => {
                let other = &pool[rng.usize(..pool.len())];
                extra_letters = other.len();
                word = format!("{word}-{other}");
            }
            mark => word.push(mark),
        }
    }
    (word, extra_letters)
}

fn pick_punctuation(rng: &mut Rng) -> char {
    let total: u32 = PUNCTUATION.iter().map(|&(_, weight)| weight).sum();
    let mut roll = rng.u32(..total);
    for (mark, weight) in PUNCTUATION {
        if roll < weight {
            return mark;
        }
        roll -= weight;
    }
    unreachable!("roll is below the total weight")
}

/// A word of random letters, used only if the phonetic model comes up short.
fn random_word(letters: &[char], focus: Option<char>, rng: &mut Rng) -> String {
    let len = rng.usize(MIN_WORD_LEN..=6);
    let mut word: Vec<char> = (0..len)
        .map(|_| letters[rng.usize(..letters.len())])
        .collect();
    if let Some(f) = focus {
        word[rng.usize(..len)] = f;
    }
    word.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::model::MAX_WORD_LEN;
    use super::*;
    use crate::difficulty::Difficulty;
    use crate::progress::{LETTER_ORDER, STARTING_KEYS};

    #[test]
    fn same_seed_gives_same_text() {
        let generator = LessonGenerator::default();
        let rules = Difficulty::Hard.rules();
        let letters = &LETTER_ORDER[..10];
        let a = generator.generate(letters, Some('a'), &rules, &mut Rng::with_seed(7));
        let b = generator.generate(letters, Some('a'), &rules, &mut Rng::with_seed(7));
        assert_eq!(a, b);
    }

    #[test]
    fn easy_and_medium_use_only_unlocked_letters() {
        let generator = LessonGenerator::default();
        for n in [STARTING_KEYS, 10, 26] {
            let letters = &LETTER_ORDER[..n];
            for difficulty in [Difficulty::Easy, Difficulty::Medium] {
                for seed in 0..20 {
                    let text = generator.generate(
                        letters,
                        None,
                        &difficulty.rules(),
                        &mut Rng::with_seed(seed),
                    );
                    assert!(
                        text.chars().all(|c| c == ' ' || letters.contains(&c)),
                        "{text}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_word_contains_the_focus_letter() {
        let generator = LessonGenerator::default();
        let letters = &LETTER_ORDER[..STARTING_KEYS];
        let rules = Difficulty::Medium.rules();
        for &focus in letters {
            for seed in 0..10 {
                let text =
                    generator.generate(letters, Some(focus), &rules, &mut Rng::with_seed(seed));
                for word in text.split(' ') {
                    assert!(word.contains(focus), "{word:?} has no {focus:?}");
                }
            }
        }
    }

    #[test]
    fn hard_adds_only_capitals_and_known_punctuation() {
        let generator = LessonGenerator::default();
        let letters = &LETTER_ORDER[..12];
        let rules = Difficulty::Hard.rules();
        let (mut capitals, mut punctuation) = (false, false);
        for seed in 0..20 {
            let text = generator.generate(letters, Some('d'), &rules, &mut Rng::with_seed(seed));
            for c in text.chars() {
                if c.is_ascii_uppercase() {
                    capitals = true;
                    assert!(letters.contains(&c.to_ascii_lowercase()), "{c:?}");
                } else if PUNCTUATION.iter().any(|&(mark, _)| mark == c) {
                    punctuation = true;
                } else {
                    assert!(c == ' ' || letters.contains(&c), "{c:?} in {text}");
                }
            }
        }
        assert!(capitals && punctuation);
    }

    #[test]
    fn lessons_are_long_enough_and_cleanly_spaced() {
        let generator = LessonGenerator::default();
        for difficulty in Difficulty::ALL {
            for seed in 0..20 {
                let text = generator.generate(
                    &LETTER_ORDER,
                    None,
                    &difficulty.rules(),
                    &mut Rng::with_seed(seed),
                );
                let letters = text.chars().filter(char::is_ascii_alphabetic).count();
                assert!(letters >= LESSON_LETTERS, "{letters} letters: {text}");
                assert!(!text.starts_with(' ') && !text.ends_with(' '));
                assert!(!text.contains("  "));
            }
        }
    }

    #[test]
    fn pseudo_words_respect_letters_and_length() {
        let model = PhoneticModel::train(WORDS.split_whitespace());
        let allowed = LetterSet::from_letters(&LETTER_ORDER[..STARTING_KEYS]);
        let mut rng = Rng::with_seed(1);
        let mut made = 0;
        for _ in 0..200 {
            if let Some(word) = model.word(allowed, Some('r'), &mut rng) {
                made += 1;
                assert!(
                    (MIN_WORD_LEN..=MAX_WORD_LEN).contains(&word.len()),
                    "{word}"
                );
                assert!(allowed.contains_word(&word) && word.contains('r'), "{word}");
            }
        }
        assert!(made > 150, "only {made} of 200 attempts made a word");
    }

    #[test]
    fn word_list_is_valid() {
        let words: Vec<&str> = WORDS.split_whitespace().collect();
        assert!(words.len() >= 1000, "only {} words", words.len());
        let mut seen = HashSet::new();
        for word in words {
            assert!(word.chars().all(|c| c.is_ascii_lowercase()), "{word:?}");
            assert!(seen.insert(word), "duplicate {word:?}");
        }
    }
}
