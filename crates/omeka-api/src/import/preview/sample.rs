//! Seeded samples: the same seed draws the same members, another seed
//! others, and no member is favoured by its place in the list.

/// The `n` members with the lowest score under `seed`, in key order. Each
/// member's key must be distinct, such as a record id or a flag's index.
pub(super) fn sample<T>(members: impl Iterator<Item = (u64, T)>, seed: u64, n: usize) -> Vec<T> {
    let mut kept: Vec<(u64, u64, T)> = Vec::with_capacity(n + 1);
    for (key, member) in members {
        let score = mix(seed ^ mix(key));
        if kept.len() == n && kept.last().is_some_and(|(last, _, _)| score >= *last) {
            continue;
        }
        let at = kept.partition_point(|(held, _, _)| *held < score);
        kept.insert(at, (score, key, member));
        kept.truncate(n);
    }
    kept.sort_by_key(|(_, key, _)| *key);
    kept.into_iter().map(|(_, _, member)| member).collect()
}

/// SplitMix64's finaliser: a fixed, well-spread map of `u64` to `u64`.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::sample;

    #[test]
    fn a_seed_draws_the_same_members_and_another_seed_others() {
        let members = || (0..1000_u64).map(|key| (key, key));
        let first = sample(members(), 7, 3);
        assert_eq!(first.len(), 3);
        assert_eq!(first, sample(members(), 7, 3), "the same seed");
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]), "key order");
        let others: Vec<Vec<u64>> = (8..12).map(|seed| sample(members(), seed, 3)).collect();
        assert!(others.iter().any(|drawn| *drawn != first), "{others:?}");
        assert!(first != [0, 1, 2], "not the first members");
        assert_eq!(sample((0..2_u64).map(|key| (key, key)), 7, 3), [0, 1]);
    }
}
