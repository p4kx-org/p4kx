// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::Exclusion;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::input_budget::InputBudget;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::source_selection::SourceSelection;
use crate::{
    MediaError, MediaKind, OriginalIdentifier, ReferenceResolution, ReferenceUnavailable,
    Subresource,
};

#[test]
fn a_genuine_token_from_another_root_is_refused_before_input_admission() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |outer, writer, _, _| {
            support::with_fixture(
                [
                    TestEntry::stored("owner.dds", b"other owner"),
                    TestEntry::stored("owner.dds.1", b"needed"),
                ],
                Inventory::Intact,
                &Exclusion::default(),
                |inner, inner_writer, _, _| {
                    let token = support::lookup(inner, "owner.dds.1", inner_writer)?;
                    assert_ne!(
                        outer.source.root, inner.source.root,
                        "independent actual root digests"
                    );
                    let called = Cell::new(false);
                    let failure = outer
                        .with_required_input(
                            &token,
                            &Subresource::new("synthetic.foreign", vec![0]),
                            writer,
                            |_input, _writer| {
                                called.set(true);
                                Ok(())
                            },
                        )
                        .err()
                        .ok_or("foreign root input accepted")?;
                    assert!(
                        matches!(failure, MediaError::Contract(_)),
                        "typed scope refusal"
                    );
                    assert!(!called.get(), "no foreign input callback");
                    assert_eq!(
                        outer.directory.input_state()?.budget.sources.get(),
                        1,
                        "refusal precedes input admission"
                    );
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn a_genuine_token_from_another_container_is_refused_before_input_admission() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("owner.dds.1", b"inner"))
        .build();
    support::with_fixture(
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"outer"),
            TestEntry::stored("bundle.socpak", inner),
        ],
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let archive = context.directory.index.archive;
            let entry = archive.entries().get(2).ok_or("container")?;
            let container = archive.open_container(entry)?;
            let chain = [crate::source::identity(archive, entry)];
            let keys = Cell::new(0);
            let index = DirectoryIndex::new(&container, &keys);
            let budget = InputBudget::default();
            let inputs = DirectoryInputs::try_new(container.entries().len(), &budget, writer)?;
            let lookup = DirectoryLookup::new_with_inputs(
                &index,
                &context.source.root,
                &chain,
                SourceSelection {
                    include: context.directory.selection.include,
                    exclude: context.directory.selection.exclude,
                },
                &inputs,
            );
            let query = lookup.lookup_input(
                OriginalIdentifier::new_with_path("owner.dds.1"),
                "owner.dds.1",
                Some(MediaKind::Texture),
                writer,
            )?;
            let token = query.input.ok_or("actual nested token")?;
            let failure = context
                .with_required_input(
                    &token,
                    &Subresource::new("synthetic.foreign", vec![1]),
                    writer,
                    |_input, _writer| Ok(()),
                )
                .err()
                .ok_or("foreign container input accepted")?;
            assert!(matches!(failure, MediaError::Contract(_)));
            assert_eq!(
                context.directory.input_state()?.budget.sources.get(),
                1,
                "scope refusal precedes source work"
            );
            Ok(())
        },
    )
}

#[test]
fn a_lossy_display_match_has_no_read_token_or_admission() -> TestResult {
    support::with_fixture(
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("unused.bin", b"bad")
                .with_path_bytes(b"bad\xff.bin")
                .with_crc32c(1),
        ],
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let query = context.directory.lookup_input(
                OriginalIdentifier::new_with_path("bad\u{fffd}.bin"),
                "bad\u{fffd}.bin",
                Some(MediaKind::Audio),
                writer,
            )?;
            assert!(
                query.input.is_none(),
                "lossy text cannot prove an input token"
            );
            assert!(
                matches!(
                    query.reference.resolution(),
                    ReferenceResolution::Unavailable {
                        reason: ReferenceUnavailable::UnsupportedLayout,
                        ..
                    }
                ),
                "raw path loss precedes unrequested-kind classification"
            );
            assert_eq!(
                context.directory.input_state()?.budget.sources.get(),
                1,
                "bad payload was never admitted or read"
            );
            Ok(())
        },
    )
}

#[test]
fn a_recovered_inventory_cannot_prove_a_present_or_missing_input() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Recovered,
        &Exclusion::default(),
        |context, writer, _, _| {
            assert!(
                context
                    .directory
                    .index
                    .archive
                    .issues()
                    .iter()
                    .any(|(_, issue)| issue.is_archive_wide())
            );
            for path in ["owner.dds.1", "not-present.bin"] {
                let query = context.directory.lookup_input(
                    OriginalIdentifier::new_with_path(path),
                    path,
                    Some(MediaKind::Texture),
                    writer,
                )?;
                assert!(
                    query.input.is_none(),
                    "incomplete inventory lends no input token"
                );
                assert!(
                    matches!(
                        query.reference.resolution(),
                        ReferenceResolution::Unavailable {
                            reason: ReferenceUnavailable::IncompleteInventory,
                            ..
                        }
                    ),
                    "neither unique nor missing can be claimed"
                );
            }
            assert_eq!(context.directory.input_state()?.budget.sources.get(), 1);
            Ok(())
        },
    )
}
