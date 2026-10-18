//! Reproducible pseudo-random numbers.
//!
//! `srl` needs randomness that can be repeated: new IDs come from one seed,
//! and a drill's cards must stay the same for a whole study day (see
//! `docs/drills.md`, rule 5). The generators here are small, fixed
//! algorithms, so the same seed gives the same numbers on every platform and
//! in every version.

/// An endless sequence of numbers made from `seed`, with the splitmix64
/// generator.
pub(crate) fn numbers(seed: u64) -> impl Iterator<Item = u64> {
    std::iter::successors(Some(seed), |state| {
        Some(state.wrapping_add(0x9E37_79B9_7F4A_7C15))
    })
    .skip(1)
    .map(|state| {
        let mixed = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    })
}

/// A seed made from text, with 64-bit FNV-1a.
pub(crate) fn seed(text: &str) -> u64 {
    text.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

/// `items` in an order made from `seed` (a Fisher-Yates shuffle).
pub(crate) fn shuffle<T>(mut items: Vec<T>, seed: u64) -> Vec<T> {
    let count = items.len();
    for (index, number) in (1..count).rev().zip(numbers(seed)) {
        let bound = u64::try_from(index + 1).expect("a list length should fit in a u64");
        let other =
            usize::try_from(number % bound).expect("an index below a length should fit in a usize");
        items.swap(index, other);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_order() {
        let items = (0..10).collect::<Vec<_>>();
        assert_eq!(shuffle(items.clone(), 7), shuffle(items.clone(), 7));
        assert_ne!(shuffle(items.clone(), 7), shuffle(items, 8));
    }

    #[test]
    fn a_shuffle_keeps_every_item() {
        let mut shuffled = shuffle((0..10).collect::<Vec<_>>(), seed("a7f3k2:2026-10-26"));
        shuffled.sort_unstable();
        assert_eq!(shuffled, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn seeds_are_fixed() {
        // FNV-1a of the empty text and of "a", as published.
        assert_eq!(seed(""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(seed("a"), 0xAF63_DC4C_8601_EC8C);
    }
}
