//! Reproducible pseudo-random numbers.
//!
//! New IDs, and the card `srl next` picks, come from a seed that the
//! caller gives, so that the core takes its inputs explicitly and its
//! tests can repeat them. The generator
//! here is a small, fixed algorithm, so the same seed gives the same numbers
//! on every platform and in every version.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let first = numbers(7).take(5).collect::<Vec<_>>();
        assert_eq!(first, numbers(7).take(5).collect::<Vec<_>>());
        assert_ne!(first, numbers(8).take(5).collect::<Vec<_>>());
    }

    #[test]
    fn numbers_are_fixed() {
        // The first splitmix64 output for seed 0, as published.
        assert_eq!(numbers(0).next(), Some(0xE220_A839_7B1D_CDAF));
    }
}
