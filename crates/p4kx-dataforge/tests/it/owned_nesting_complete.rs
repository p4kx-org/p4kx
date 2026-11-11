// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, FieldStatistics, Number, TreeBuilder};

use crate::owned_source_controls as controls;
use crate::owned_source_fixture::{self as fixture, TestResult};

#[test]
fn valid_named_owned_chains_retain_all_nodes_and_original_weak_positions() -> TestResult {
    for depth in [65, 257] {
        let bytes = fixture::database(1, depth);
        let forge = DataForge::open(&bytes)?;
        let record = controls::original(&forge, 1, depth)?;
        let tree = TreeBuilder::new().build(&record)?;
        controls::complete(&tree, 1, depth)?;
        controls::tail(&tree, &record, 1, depth)?;
        let cloned = tree.clone();
        assert!(
            cloned.is_from(&record),
            "a whole original clone preserves its actual read owner"
        );
        assert_eq!(tree, cloned, "canonical clone keeps all value semantics");
        controls::complete(&cloned, 1, depth)?;
        drop(tree);
        controls::tail(&cloned, &record, 1, depth)?;
        drop(cloned);
    }
    Ok(())
}

#[test]
fn statistics_retain_the_last_valid_owned_node_beyond_sixty_four() -> TestResult {
    let bytes = fixture::database(1, 65);
    let forge = DataForge::open(&bytes)?;
    controls::original(&forge, 1, 65)?;
    let statistics = FieldStatistics::of_all(&forge)?;
    assert_eq!(
        statistics.records(),
        1,
        "one original physical Record is examined"
    );
    assert_eq!(
        statistics.unreadable_records(),
        0,
        "the valid source is not a failed Record"
    );
    assert_eq!(
        statistics.unreadable_values(),
        0,
        "the final valid node is not a skipped value"
    );
    let mut path = "roots/Node".to_owned();
    for _ in 1..65 {
        path.push_str("/next/Node");
    }
    path.push_str("/mark");
    let mark = statistics
        .get(&path)
        .ok_or("the original last owned scalar's statistics")?;
    assert_eq!(
        mark.occurrences(),
        1,
        "the exact final declaration is counted once"
    );
    assert_eq!(
        mark.max(),
        Some(Number::Integer(65)),
        "the actual final original value is retained"
    );
    Ok(())
}
