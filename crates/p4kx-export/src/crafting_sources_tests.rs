// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{DataForge, Guid, Record};

use super::collect;
use crate::crafting_types::RewardSource;

#[path = "crafting_source_controls.rs"]
mod controls;
#[path = "crafting_source_fixture.rs"]
mod fixture;
#[path = "crafting_source_scope_controls.rs"]
mod scope_controls;
#[path = "crafting_source_scope_fixture.rs"]
mod scope_fixture;
#[path = "crafting_source_support.rs"]
mod support;

use support::TestResult;

type RewardKey<'source> = (Guid, Option<Guid>, Option<&'source str>, Option<u32>);

#[test]
fn shallow_reward_sources_keep_original_order_duplicates_and_chances() -> TestResult {
    complete_tail(3)
}

#[test]
fn the_reward_tail_after_one_hundred_thirty_owned_nodes_is_retained() -> TestResult {
    complete_tail(130)
}

#[test]
fn the_reward_tail_after_two_hundred_fifty_seven_owned_nodes_is_retained() -> TestResult {
    complete_tail(257)
}

fn complete_tail(depth: u32) -> TestResult {
    let bytes = fixture::database(depth);
    let forge = DataForge::open(&bytes)?;
    let record = controls::original(&forge, depth)?;
    let tree = record.tree()?;
    controls::complete(&forge, &record, &tree, depth)?;
    let mut sources = Vec::new();
    collect(&tree, &fixture::pools(), &mut sources);
    let actual = observations(&sources, &record)?;
    let expected = [
        (
            fixture::pool_a(),
            None,
            Some("root"),
            Some(0.25_f32.to_bits()),
        ),
        (
            fixture::pool_a(),
            None,
            Some("root"),
            Some(0.25_f32.to_bits()),
        ),
        (
            fixture::pool_b(),
            None,
            Some("root"),
            Some(0.25_f32.to_bits()),
        ),
        (
            fixture::pool_b(),
            None,
            Some("root"),
            Some(0.75_f32.to_bits()),
        ),
    ];
    assert_eq!(
        actual, expected,
        "every deep tail slot precedes the later sibling without deduplication"
    );
    Ok(())
}

fn observations<'source>(
    sources: &[RewardSource<'source>],
    owner: &Record<'_>,
) -> TestResult<Vec<RewardKey<'source>>> {
    let path = owner.record_path()?;
    Ok(sources
        .iter()
        .map(|source| {
            assert_eq!(
                source.record,
                owner.guid(),
                "each occurrence keeps its actual owner"
            );
            assert_eq!(
                source.record_path, path,
                "each occurrence keeps its original path"
            );
            (
                source.pool,
                source.contract_id,
                source.contract_name,
                source.chance.map(f32::to_bits),
            )
        })
        .collect())
}

#[test]
fn self_record_and_local_weak_links_do_not_become_reward_sources() -> TestResult {
    let bytes = fixture::database(3);
    let forge = DataForge::open(&bytes)?;
    let record = controls::original(&forge, 3)?;
    let tree = record.tree()?;
    controls::complete(&forge, &record, &tree, 3)?;
    let mut sources = Vec::new();
    collect(&tree, &HashSet::from([record.guid()]), &mut sources);
    assert!(
        sources.is_empty(),
        "the genuine self Reference, root Weak and local Weak remain excluded"
    );
    Ok(())
}

#[test]
fn nearest_identity_scope_and_array_source_order_are_preserved() -> TestResult {
    let bytes = scope_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let record = scope_controls::original(&forge)?;
    let tree = record.tree()?;
    scope_controls::complete(&forge, &record, &tree)?;
    let mut sources = Vec::new();
    collect(&tree, &fixture::pools(), &mut sources);
    let actual = observations(&sources, &record)?;
    assert_eq!(
        actual,
        scope_expectation(),
        "the exact scope, stored order and unmodified chances survive"
    );
    Ok(())
}

fn scope_expectation() -> [RewardKey<'static>; 5] {
    let first_id = Guid::from_stored([0x55; 16]);
    let nested_id = Guid::from_stored([0x66; 16]);
    [
        (
            fixture::pool_a(),
            None,
            Some("nearest"),
            Some(0.5_f32.to_bits()),
        ),
        (
            fixture::pool_b(),
            Some(first_id),
            Some("bound"),
            Some(0.0_f32.to_bits()),
        ),
        (
            fixture::pool_a(),
            Some(nested_id),
            None,
            Some(1.5_f32.to_bits()),
        ),
        (
            fixture::pool_b(),
            Some(first_id),
            Some("bound"),
            Some(0.25_f32.to_bits()),
        ),
        (
            fixture::pool_a(),
            None,
            Some("root-fallback"),
            Some(0.75_f32.to_bits()),
        ),
    ]
}
