// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{AnimationError, AnimationHeader, CorruptAnimation, DbaHeaders};

use super::{Family, TestResult, Totals, require};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Counts {
    clips: u64,
    keys: u64,
}

pub(super) fn verify(
    family: Family,
    name: &str,
    file: &ChunkFile<&[u8]>,
    totals: &mut Totals,
    clips_before: u64,
    keys_before: u64,
) -> TestResult {
    let inventory = match family {
        Family::Caf => AnimationHeader::read_caf(name, file).map(|header| Counts {
            clips: 1,
            keys: header.keys as u64,
        }),
        Family::Dba => DbaHeaders::read(name, file)
            .map(|headers| dba_counts(headers, &mut totals.header_budget_refusals)),
        Family::Aim => return Err("AIM does not have decoded headers".into()),
    };
    let observed = Counts {
        clips: totals.clips - clips_before,
        keys: totals.declared - keys_before,
    };
    reconcile(
        family,
        inventory,
        observed,
        &mut totals.header_budget_refusals,
    )
}

fn dba_counts(headers: DbaHeaders, budget_refusals: &mut u64) -> Counts {
    let clips = (headers.animations.len() + headers.skipped.len()) as u64;
    let mut keys = headers
        .animations
        .iter()
        .map(|header| header.keys as u64)
        .sum();
    for skipped in headers.skipped {
        if let CorruptAnimation::TooManyKeys { keys: declared, .. } = skipped.reason {
            keys += declared;
            *budget_refusals += 1;
        }
    }
    Counts { clips, keys }
}

fn reconcile(
    family: Family,
    inventory: Result<Counts, AnimationError>,
    observed: Counts,
    budget_refusals: &mut u64,
) -> TestResult {
    let expected = match inventory {
        Ok(counts) => counts,
        Err(AnimationError::Corrupt {
            source: CorruptAnimation::TooManyKeys { keys, .. },
            ..
        }) => {
            *budget_refusals += 1;
            if !matches!(family, Family::Caf) {
                return Ok(());
            }
            Counts { clips: 1, keys }
        }
        Err(error) => return Err(error.into()),
    };
    require(
        expected == observed,
        "complete media clip and key totals agree with the independent header inventory",
    )
}

#[test]
fn caf_header_budget_refusal_still_requires_exact_inventory() {
    use p4kx_extraction::model::MAX_ANIMATION_KEYS;

    let keys = MAX_ANIMATION_KEYS + 1;
    for (observed, agrees) in [
        (Counts { clips: 1, keys }, true),
        (Counts { clips: 2, keys }, false),
        (
            Counts {
                clips: 1,
                keys: keys - 1,
            },
            false,
        ),
    ] {
        let inventory = Err(AnimationError::Corrupt {
            file: "fixture.caf".to_owned(),
            chunk: "CafData",
            source: CorruptAnimation::TooManyKeys {
                keys,
                limit: MAX_ANIMATION_KEYS,
            },
        });
        let mut budget_refusals = 0;
        let result = reconcile(Family::Caf, inventory, observed, &mut budget_refusals);
        assert_eq!(result.is_ok(), agrees, "exact CAF clip and key totals");
        assert_eq!(budget_refusals, 1, "budget refusal remains visible");
    }
}
