// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Exclusion;
use p4kx_media::ExportOptions;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::required_input_support as input;
use super::support::{self, TestResult};

#[test]
fn misleading_dds_names_do_not_inherit_a_header_exclusion() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"P"),
            TestEntry::stored("owner.dds.a", b"A"),
            TestEntry::stored("owner.dds.01", b"zero-prefixed suffix"),
            TestEntry::stored("owner.dds.fake", b"nonmember"),
            TestEntry::stored("owner.dds.folder/child.bin", b"folder name"),
            TestEntry::stored("owner.dds.1.fake", b"extra suffix"),
        ],
    )?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["*.dds"])?,
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    assert_eq!(
        visits,
        vec![3, 4, 5, 6],
        "only valid DDS members inherit header exclusion"
    );
    assert_eq!(
        input::sources(&output, &index)?.len(),
        4,
        "misleading names remain independent sources"
    );
    assert_eq!(
        index.source_entries.skipped, 3,
        "header and real part/alpha stay excluded"
    );
    Ok(())
}

#[test]
fn qualified_header_exclusion_uses_the_same_member_grammar_inside_a_container() -> TestResult {
    let folder = tempfile::tempdir()?;
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("owner.dds", b"H"))
        .entry(TestEntry::stored("owner.dds.1", b"corrupt excluded part").with_crc32c(1))
        .entry(TestEntry::stored("owner.dds.01", b"ordinary source"))
        .build();
    let archive =
        support::archive_with(folder.path(), [TestEntry::stored("bundle.socpak", inner)])?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["bundle.socpak!*.dds"])?,
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    assert_eq!(
        visits,
        vec![0, 2],
        "container and independent child preserve physical callback order"
    );
    assert_eq!(
        input::sources(&output, &index)?.len(),
        2,
        "qualified excluded family is not retained"
    );
    assert_eq!(
        index.source_entries.enumerated, 4,
        "outer and three physical children are counted"
    );
    Ok(())
}

#[test]
fn qualified_individual_part_exclusion_allows_only_its_required_private_read() -> TestResult {
    let folder = tempfile::tempdir()?;
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("owner.dds", b"H"))
        .entry(TestEntry::stored("owner.dds.1", b"needed"))
        .build();
    let archive =
        support::archive_with(folder.path(), [TestEntry::stored("bundle.socpak", inner)])?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["bundle.socpak!*.dds.1"])?,
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    assert_eq!(visits, vec![0, 0], "required part has no child callback");
    let row = input::row(&output, &index)?;
    assert_eq!(
        row.pointer("/inputs/0/omission"),
        Some(&json!("explicit-exclusion"))
    );
    assert_eq!(row.pointer("/inputs/0/source/raw"), Some(&json!(null)));
    assert_eq!(
        row.pointer("/inputs/0/source/identity/containers/0/path"),
        Some(&json!("bundle.socpak")),
        "receipt preserves the actual owning container",
    );
    assert_eq!(
        input::sources(&output, &index)?.len(),
        2,
        "only selected container and header retain bytes"
    );
    input::no_private_spool(&output, &index)?;
    Ok(())
}
