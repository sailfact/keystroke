//! A letter-level Markov chain trained on English words. It invents
//! pronounceable pseudo-words from whatever letters are unlocked.

use fastrand::Rng;

/// Symbol 0 is a word boundary; 1 to 26 are the letters `a` to `z`.
const BOUNDARY: usize = 0;
const SYMBOLS: usize = 27;

pub const MIN_WORD_LEN: usize = 3;
pub const MAX_WORD_LEN: usize = 10;

/// Ending a word gets this much more likely with every letter, which keeps
/// words short.
const END_BOOST: f32 = 1.3;
/// Makes the focus letter more likely, so fewer words get rejected for
/// missing it.
const FOCUS_BOOST: f32 = 3.0;
const ATTEMPTS: usize = 50;

/// A set of lowercase letters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LetterSet(u32);

impl LetterSet {
    pub fn from_letters(letters: &[char]) -> Self {
        let mut set = Self::default();
        for &c in letters {
            if c.is_ascii_lowercase() {
                set.0 |= 1 << (c as u8 - b'a');
            }
        }
        set
    }

    pub fn contains(self, c: char) -> bool {
        c.is_ascii_lowercase() && self.0 & (1 << (c as u8 - b'a')) != 0
    }

    pub fn contains_word(self, word: &str) -> bool {
        word.chars().all(|c| self.contains(c))
    }
}

pub struct PhoneticModel {
    /// How often each symbol follows each pair of symbols, indexed by
    /// [`index`].
    counts: Vec<u32>,
}

fn index(two_back: usize, one_back: usize, next: usize) -> usize {
    (two_back * SYMBOLS + one_back) * SYMBOLS + next
}

fn symbol(c: char) -> usize {
    (c as u8 - b'a') as usize + 1
}

fn letter(symbol: usize) -> char {
    (b'a' + (symbol - 1) as u8) as char
}

impl PhoneticModel {
    /// Trains on lowercase words. Anything else is skipped.
    pub fn train<'a>(words: impl IntoIterator<Item = &'a str>) -> Self {
        let mut counts = vec![0; SYMBOLS.pow(3)];
        for word in words {
            if !word.chars().all(|c| c.is_ascii_lowercase()) {
                continue;
            }
            let (mut two_back, mut one_back) = (BOUNDARY, BOUNDARY);
            for c in word.chars() {
                let next = symbol(c);
                counts[index(two_back, one_back, next)] += 1;
                (two_back, one_back) = (one_back, next);
            }
            counts[index(two_back, one_back, BOUNDARY)] += 1;
        }
        Self { counts }
    }

    /// Invents a word from the `allowed` letters that contains `focus`, if
    /// one is given. Returns `None` when nothing suitable turns up in a
    /// reasonable number of tries.
    pub fn word(&self, allowed: LetterSet, focus: Option<char>, rng: &mut Rng) -> Option<String> {
        (0..ATTEMPTS).find_map(|_| {
            self.attempt(allowed, focus, rng)
                .filter(|word| focus.is_none_or(|f| word.contains(f)))
        })
    }

    fn attempt(&self, allowed: LetterSet, focus: Option<char>, rng: &mut Rng) -> Option<String> {
        let mut word = String::new();
        let (mut two_back, mut one_back) = (BOUNDARY, BOUNDARY);
        let mut weights = [0.0; SYMBOLS];
        loop {
            let len = word.len();
            for (next, weight) in weights.iter_mut().enumerate() {
                let count = self.counts[index(two_back, one_back, next)] as f32;
                *weight = if next == BOUNDARY {
                    if len < MIN_WORD_LEN {
                        0.0
                    } else {
                        count * END_BOOST.powi(len as i32)
                    }
                } else {
                    let c = letter(next);
                    if !allowed.contains(c) {
                        0.0
                    } else if focus == Some(c) {
                        count * FOCUS_BOOST
                    } else {
                        count
                    }
                };
            }

            // No way to continue from here: give up on this attempt.
            let next = weighted_pick(&weights, rng)?;
            if next == BOUNDARY {
                return Some(word);
            }
            if len >= MAX_WORD_LEN {
                return None;
            }
            word.push(letter(next));
            (two_back, one_back) = (one_back, next);
        }
    }
}

/// Picks an index with probability proportional to its weight, or `None` if
/// every weight is zero.
fn weighted_pick(weights: &[f32], rng: &mut Rng) -> Option<usize> {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut remaining = rng.f32() * total;
    let mut last = None;
    for (i, &weight) in weights.iter().enumerate() {
        if weight <= 0.0 {
            continue;
        }
        if remaining < weight {
            return Some(i);
        }
        remaining -= weight;
        last = Some(i);
    }
    // Only reachable through rounding: fall back to the last candidate.
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_set_membership() {
        let set = LetterSet::from_letters(&['a', 'c']);
        assert!(set.contains('a') && set.contains('c'));
        assert!(!set.contains('b') && !set.contains('A') && !set.contains(' '));
        assert!(!set.contains_word("cab") && set.contains_word("acca"));
    }

    #[test]
    fn follows_training_data() {
        // With a single training word, the only possible output is that word.
        let model = PhoneticModel::train(["tent"]);
        let allowed = LetterSet::from_letters(&['e', 'n', 't']);
        let mut rng = Rng::with_seed(3);
        assert_eq!(model.word(allowed, None, &mut rng).as_deref(), Some("tent"));
        assert_eq!(model.word(allowed, Some('x'), &mut rng), None);
    }

    #[test]
    fn weighted_pick_skips_zero_weights() {
        let mut rng = Rng::with_seed(9);
        for _ in 0..100 {
            assert_eq!(weighted_pick(&[0.0, 2.0, 0.0], &mut rng), Some(1));
        }
        assert_eq!(weighted_pick(&[0.0, 0.0], &mut rng), None);
    }
}
