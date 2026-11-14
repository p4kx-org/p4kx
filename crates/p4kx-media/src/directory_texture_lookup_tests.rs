// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Exclusion, TextureMemberRelation};
use p4kx_testkit::TestEntry;

use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{
    CandidateInventory, CandidateState, Identity, MediaError, NotSelectedReason,
    ReferenceUnavailable,
};

#[test]
fn the_public_family_visitor_keeps_selection_identity_and_does_not_read_targets() -> TestResult {
    support::with_fixture(
        selection_entries(),
        Inventory::Intact,
        &Exclusion::parse(["*owner.dds.3"])?,
        |context, writer, _, _| {
            let budget = context.directory.input_state()?.budget;
            let remaining = budget.remaining_bytes(writer.limits());
            let mut seen = Vec::new();
            let inventory = context.directory.visit_texture_family(
                "owner.dds",
                writer,
                |member, reference, _| {
                    let Identity::Entry { entry, .. } = reference.target.source else {
                        return Err(MediaError::Contract("physical member identity".into()));
                    };
                    assert_eq!(entry.entry_index, member.entry.index());
                    assert!(matches!(
                        member.relation,
                        TextureMemberRelation::Matched { .. }
                    ));
                    seen.push((
                        entry.entry_index,
                        matches!(
                            reference.state,
                            CandidateState::NotSelected {
                                reason: NotSelectedReason::Filter
                            }
                        ),
                    ));
                    Ok(())
                },
            )?;
            assert!(matches!(inventory, CandidateInventory::Complete));
            assert_eq!(
                seen,
                [
                    (0, false),
                    (1, false),
                    (2, false),
                    (3, true),
                    (4, false),
                    (5, false),
                    (6, false)
                ]
            );
            assert_eq!(
                budget.sources.get(),
                1,
                "only the selected fixture owner was retained"
            );
            assert_eq!(
                budget.remaining_bytes(writer.limits()),
                remaining,
                "inventory does not read payloads"
            );
            Ok(())
        },
    )
}

#[test]
fn the_public_visitor_keeps_raw_loss_and_recovered_inventory_unproved() -> TestResult {
    for inventory in [Inventory::Intact, Inventory::Recovered] {
        support::with_fixture(
            [
                TestEntry::stored("owner.dds", b"H"),
                TestEntry::stored("x", b"opaque").with_path_bytes(b"bad\xff.dds.3"),
            ],
            inventory,
            &Exclusion::parse(["*bad*"])?,
            |context, writer, _, _| {
                let mut seen = 0;
                let completeness = context.directory.visit_texture_family(
                    "bad\u{fffd}.dds",
                    writer,
                    |member, reference, _| {
                        seen += 1;
                        assert_eq!(member.entry.index(), 1);
                        assert!(matches!(
                            member.relation,
                            TextureMemberRelation::UnsupportedRawPath
                        ));
                        assert!(matches!(
                            reference.state,
                            CandidateState::Unavailable {
                                reason: ReferenceUnavailable::UnsupportedLayout
                            }
                        ));
                        let Identity::Entry { entry, .. } = reference.target.source else {
                            return Err(MediaError::Contract("raw physical identity".into()));
                        };
                        assert_eq!(entry.raw_path_hex.as_deref(), Some("626164ff2e6464732e33"));
                        Ok(())
                    },
                )?;
                assert_eq!(seen, 1);
                let mut absent_seen = 0;
                let absent = context.directory.visit_texture_family(
                    "not-present.dds",
                    writer,
                    |_, _, _| {
                        absent_seen += 1;
                        Ok(())
                    },
                )?;
                assert_eq!(absent_seen, 0);
                assert_inventory(inventory, completeness);
                assert_inventory(inventory, absent);
                assert_eq!(context.directory.input_state()?.budget.sources.get(), 1);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn a_visitor_refusal_is_returned_without_visiting_later_members() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let mut seen = 0;
            let result = context
                .directory
                .visit_texture_family("owner.dds", writer, |_, _, _| {
                    seen += 1;
                    Err(MediaError::Contract("visitor refusal".into()))
                });
            assert!(
                matches!(result, Err(MediaError::Contract(detail)) if detail == "visitor refusal")
            );
            assert_eq!(seen, 1);
            assert_eq!(context.directory.input_state()?.budget.sources.get(), 1);
            Ok(())
        },
    )
}

fn selection_entries() -> [TestEntry; 7] {
    [
        TestEntry::stored("owner.dds", b"H"),
        TestEntry::stored("OWNER.DDS", b"duplicate"),
        TestEntry::stored("owner.dds.1", b"one"),
        TestEntry::stored("owner.dds.3", b"excluded"),
        TestEntry::stored("owner.dds.33", b"later"),
        TestEntry::stored("owner.dds.a", b"alpha"),
        TestEntry::stored("owner.dds.4a", b"alpha part"),
    ]
}

fn assert_inventory(inventory: Inventory, observed: CandidateInventory) {
    assert!(matches!(
        (inventory, observed),
        (Inventory::Intact, CandidateInventory::Complete)
            | (
                Inventory::Recovered,
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory)
            )
    ));
}
