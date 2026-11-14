// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::Value;

use crate::origin_aggregate_support::{self as aggregate, TestResult};
use crate::origin_test_support::{self as support};

const SHADER: &[u8] = b"Version(1)\nProperty { Name(%A); Mask(0x10); }";

#[test]
fn plain_source_aggregate_includes_its_unknown_root_and_all_actual_source_members() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (_, output, index) =
        support::export(folder.path(), [TestEntry::stored("plain.ext", SHADER)])?;
    let parent = aggregate::field(&output, &index, "sources", 0, "")?;
    let mut children = Vec::new();
    for pointer in [
        "/root",
        "/identity",
        "/size",
        "/declared_integrity",
        "/verification",
        "/raw",
    ] {
        children.push(aggregate::field(&output, &index, "sources", 0, pointer)?);
    }
    assert_eq!(
        support::marker(&parent),
        None,
        "a proved plain Entry cannot erase its independently unknown whole-Archive hash"
    );
    assert_eq!(
        support::complete(&parent),
        Some(true),
        "the actual Source member set is known"
    );
    aggregate::assert_union(&parent, &children)?;
    let ordinal = support::ordinal(&output, &index, "shader", "plain.ext", "shader-definition")?;
    let record_source = support::field(&output, &index, "shader", ordinal, "/source")?;
    assert_eq!(
        support::marker(&record_source),
        None,
        "record Source has the same actual root input"
    );
    Ok(())
}

#[test]
fn retained_source_digest_descriptors_do_not_inherit_source_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (_, output, index) = support::export(
        folder.path(),
        [TestEntry::zstd("encrypted.ext", SHADER).encrypted()],
    )?;
    for pointer in [
        "/root/algorithm",
        "/root/scope",
        "/raw/digest/algorithm",
        "/raw/digest/scope",
    ] {
        let field = aggregate::field(&output, &index, "sources", 0, pointer)?;
        assert_eq!(
            support::marker(&field),
            Some(false),
            "fixed digest descriptor {pointer}"
        );
        assert_eq!(
            support::complete(&field),
            Some(true),
            "known independent value"
        );
        assert!(
            aggregate::references(&field)?.is_empty(),
            "no source bytes determine {pointer}"
        );
    }
    for (pointer, encryption) in [("/root/hex", None), ("/raw/digest/hex", Some(true))] {
        let field = aggregate::field(&output, &index, "sources", 0, pointer)?;
        assert_eq!(
            support::marker(&field),
            encryption,
            "original hash input {pointer}"
        );
        assert!(
            !aggregate::references(&field)?.is_empty(),
            "original bytes remain bound"
        );
    }
    Ok(())
}

#[test]
fn directory_source_without_raw_does_not_acquire_the_containing_entry_payload() -> TestResult {
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(TestEntry::stored("nested.ext", SHADER))
        .build();
    let (_, output, index) = support::export(
        folder.path(),
        [TestEntry::zstd("container.socpak", nested).encrypted()],
    )?;
    let ordinal = support::ordinal(
        &output,
        &index,
        "archive",
        "container.socpak",
        "archive-layout",
    )?;
    let source = support::field(&output, &index, "archive", ordinal, "/source")?;
    let raw = support::field(&output, &index, "archive", ordinal, "/source/raw")?;
    let data = support::field(&output, &index, "archive", ordinal, "/data")?;
    assert_eq!(
        support::marker(&data),
        Some(true),
        "decrypted directory layout used containing payload"
    );
    assert_eq!(
        support::marker(&source),
        None,
        "Source describes root hash and public parent metadata without a retained raw payload"
    );
    assert_eq!(
        support::marker(&raw),
        Some(false),
        "null raw availability is an independent control"
    );
    assert!(
        aggregate::references(&raw)?.is_empty(),
        "no unobserved raw payload input is invented"
    );
    let observations = support::rows(&output, &index, "origin-uses.jsonl")?;
    let data_sources = use_sources(&data, &observations)?;
    for reference in aggregate::references(&source)? {
        let contributor = observations
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(reference.as_str()))
            .ok_or("actual Source use")?;
        let own_payload = contributor.get("scope").and_then(Value::as_str) == Some("payload")
            && contributor
                .get("source")
                .and_then(Value::as_str)
                .is_some_and(|source| data_sources.contains(source));
        assert!(
            !own_payload,
            "directory-only Source must not borrow its layout's payload taint"
        );
    }
    Ok(())
}

fn use_sources(
    field: &Value,
    observations: &[Value],
) -> Result<std::collections::BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut sources = std::collections::BTreeSet::new();
    for reference in aggregate::references(field)? {
        let contributor = observations
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(reference.as_str()))
            .ok_or("original contributing use")?;
        sources.insert(
            contributor
                .get("source")
                .and_then(Value::as_str)
                .ok_or("original physical source reference")?
                .to_owned(),
        );
    }
    Ok(sources)
}

#[test]
fn container_identity_list_uses_ancestor_metadata_instead_of_the_child_entry_metadata() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(TestEntry::stored("nested.ext", SHADER))
        .build();
    let (_, output, index) = support::export(
        folder.path(),
        [TestEntry::zstd("container.socpak", nested).encrypted()],
    )?;
    let ordinal = support::ordinal(&output, &index, "shader", "nested.ext", "shader-definition")?;
    let list = support::field(
        &output,
        &index,
        "shader",
        ordinal,
        "/source/identity/containers",
    )?;
    let container = support::field(
        &output,
        &index,
        "shader",
        ordinal,
        "/source/identity/containers/0",
    )?;
    let path = support::field(
        &output,
        &index,
        "shader",
        ordinal,
        "/source/identity/containers/0/path",
    )?;
    assert_eq!(
        support::marker(&path),
        Some(false),
        "outer container's name comes from the root directory"
    );
    assert_eq!(
        support::marker(&list),
        Some(false),
        "the descendant's encrypted directory cannot taint the outer container identity"
    );
    aggregate::assert_union(&list, std::slice::from_ref(&container))?;
    let mut members = vec![path];
    for field in ["entry_index", "raw_path_hex"] {
        members.push(support::field(
            &output,
            &index,
            "shader",
            ordinal,
            &format!("/source/identity/containers/0/{field}"),
        )?);
    }
    aggregate::assert_union(&container, &members)?;
    Ok(())
}
