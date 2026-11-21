// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceEvidenceError;

use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};

use super::fixture;
use super::{OwnedValueEvent, OwnedValueVisit, OwnedValueWalk};

#[test]
fn neutral_visits_keep_original_owner_depth_paths_and_occurrence_order() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, _| {
        let record = forge.record(0).ok_or("actual Record")?;
        let tree = record.tree()?;
        let mut walk = OwnedValueWalk::new(&record, &tree, None)?;
        let expected = [
            "dup",
            "dup",
            "container",
            "container.zero",
            "container.odd.name",
            "slots",
            "slots[0]",
            "slots[0].marker",
            "slots[1]",
            "slots[2]",
            "slots[3]",
            "slots[3].marker",
            "tail",
        ];
        let mut visits = 0;
        while let Some(event) = walk.next_event()? {
            let OwnedValueEvent::Value(visit) = event else {
                continue;
            };
            assert_eq!(visit.ordinal(), u64::try_from(visits)?);
            assert_eq!(
                visit.logical_path(),
                *expected.get(visits).ok_or("actual path oracle")?
            );
            check_owner(&visit);
            visits += 1;
        }
        assert_eq!(
            visits,
            expected.len(),
            "same complete source inventory as the bound Loot walk"
        );
        Ok(())
    })
}

fn check_owner(visit: &OwnedValueVisit<'_, '_, '_>) {
    match visit.ordinal() {
        3 | 4 => {
            assert_eq!(visit.owner_type(), "Nested");
            assert_eq!(visit.parent_type(), Some("Owner"));
            assert_eq!(visit.struct_depth(), 1);
        }
        7 | 11 => {
            assert_eq!(visit.owner_type(), "Leaf");
            assert_eq!(visit.parent_type(), Some("Owner"));
            assert_eq!(visit.struct_depth(), 1);
        }
        _ => {
            assert_eq!(visit.owner_type(), "Owner");
            assert_eq!(visit.parent_type(), None);
            assert_eq!(visit.struct_depth(), 0);
        }
    }
}

#[test]
fn neutral_walk_requires_actual_record_tree_pairing_without_source_registry() -> TestResult {
    test_support::with_input(&fixture::database(), |forge, _| {
        let record = forge.record(0).ok_or("actual Record")?;
        let other = forge.record(1).ok_or("other actual Record")?;
        let tree = record.tree()?;
        assert!(
            OwnedValueWalk::new(&record, &tree, None).is_ok(),
            "positive original pair"
        );
        assert!(
            matches!(
                OwnedValueWalk::new(&other, &tree, None),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "equal body fields cannot replace actual Record ownership"
        );
        Ok(())
    })
}
