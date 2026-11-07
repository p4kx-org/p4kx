// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{Identity, MaterialProvider, MediaRecord};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::json;

use super::material_lookup_support::{field, reference, status, target_index};
use super::material_support::{self as support, TestResult};

#[test]
fn two_materials_close_a_cycle_with_one_parsed_record_per_source_in_either_order() -> TestResult {
    for reverse in [false, true] {
        let folder = tempfile::tempdir()?;
        let a = TestEntry::stored("Data/a.mtl", br#"<Material><MatLayers><Layer Path="b" Submtl="Hull"/><Layer Path="b" Submtl="Other"/></MatLayers></Material>"#);
        let b = TestEntry::stored(
            "Data/b.mtl",
            br#"<Material><MatLayers><Layer Path="a" Submtl="Name"/></MatLayers></Material>"#,
        );
        let entries = if reverse { [b, a] } else { [a, b] };
        let archive = support::archive(folder.path(), entries)?;
        let output = folder.path().join("out");
        let index = support::published(p4kx_media::export_with(
            &archive,
            &output,
            &support::options(),
            &support::registry(MaterialProvider::default())?,
        )?)?;
        let records = support::records(&output, &index)?;
        assert_eq!(
            records.len(),
            2,
            "a cycle does not recursively reopen or reparse libraries"
        );
        assert_eq!(
            index.source_entries.written, 2,
            "physical library and cycle identities stay separate"
        );
        verify_cycle(&records)?;
    }
    Ok(())
}

#[test]
fn nested_lookups_use_the_immediate_container_inventory_and_physical_chain() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = br#"<Material><MatLayers><Layer Path="b"/></MatLayers></Material>"#;
    let first = TestContainer::new().entry(TestEntry::stored("Data/a.mtl", source));
    let second = TestContainer::new()
        .entry(TestEntry::stored("Data/a.mtl", source))
        .entry(TestEntry::stored("Data/b.mtl", b"<Material/>"));
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
            TestEntry::stored("first.pak", first.build()),
            TestEntry::stored("second.pak", second.build()),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    verify_nested(&records)?;
    Ok(())
}

fn verify_cycle(records: &[MediaRecord]) -> TestResult {
    let mut edges = Vec::new();
    for record in records {
        let Identity::Entry { entry, .. } = &record.source.identity else {
            return Err("source identity".into());
        };
        let reference = reference(record, 0)?;
        status(reference, "resolved")?;
        let target = target_index(reference)?;
        assert_ne!(
            target,
            u64::try_from(entry.entry_index)?,
            "physical library and cycle identities stay separate"
        );
        edges.push((u64::try_from(entry.entry_index)?, target));
    }
    edges.sort_unstable();
    assert_eq!(
        edges,
        [(0, 1), (1, 0)],
        "both actual parsed declarations close the physical cycle"
    );
    let a = records
        .iter()
        .find(|record| {
            matches!(&record.source.identity,
        Identity::Entry { entry, .. } if entry.path.ends_with("a.mtl"))
        })
        .ok_or("a library")?;
    assert_eq!(
        field(&a.data, "/payload/references/0/submaterial")?,
        &json!("Hull"),
        "physical library and cycle identities stay separate"
    );
    assert_eq!(
        field(&a.data, "/payload/references/1/submaterial")?,
        &json!("Other"),
        "physical library and cycle identities stay separate"
    );
    status(reference(a, 1)?, "resolved")?;
    Ok(())
}

fn verify_nested(records: &[MediaRecord]) -> TestResult {
    let mut checked = 0;
    for record in records {
        let Identity::Entry { containers, entry } = &record.source.identity else {
            continue;
        };
        let Some(container) = containers.first() else {
            continue;
        };
        if entry.entry_index != 0 {
            continue;
        }
        let reference = reference(record, 0)?;
        if container.entry_index == 1 {
            status(reference, "missing")?;
        } else {
            assert_eq!(
                container.entry_index, 2,
                "nested targets stay within their owning container"
            );
            status(reference, "resolved")?;
            assert_eq!(
                target_index(reference)?,
                1,
                "nested targets stay within their owning container"
            );
            assert_eq!(
                field(
                    reference,
                    "/resolution/target/source/containers/0/entry_index"
                )?,
                &json!(2),
                "nested targets stay within their owning container"
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked, 2,
        "a root namesake cannot satisfy a nested owning-source reference"
    );
    Ok(())
}
