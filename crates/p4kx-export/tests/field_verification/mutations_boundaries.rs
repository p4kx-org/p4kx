// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::fixtures::{self, GUID, OTHER, PublishedFixture};
use super::path::{Path, Step};
use super::published_model::{MAX_JSON_BYTES, MAX_METADATA_BYTES};

#[test]
fn sparse_oversized_payloads_and_metadata_are_rejected_before_retaining_them() -> super::Result<()>
{
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(fixture.root.join("synthetic.json"))?
        .set_len(MAX_JSON_BYTES + 1)?;
    assert!(
        fixture.load().is_err(),
        "a selected file over the per-file byte bound fails before JSON allocation"
    );
    std::fs::OpenOptions::new()
        .write(true)
        .open(fixture.root.join(super::published_model::METADATA))?
        .set_len(MAX_METADATA_BYTES + 1)?;
    assert!(
        fixture.load().is_err(),
        "metadata is bounded before JSON parsing"
    );
    Ok(())
}

#[test]
fn path_traversal_malformed_join_identity_and_duplicate_metadata_are_failures() -> super::Result<()>
{
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    fixture
        .snapshot
        .files
        .first_mut()
        .ok_or("file mapping absent")?
        .path = "../outside.json".to_owned();
    fixture.save()?;
    assert!(
        fixture.load().is_err(),
        "selected paths cannot escape the offline root"
    );
    let malformed = PublishedFixture::new(vec![fixtures::row("not-a-guid", json!(1))])?;
    let catalog = fixtures::catalog(vec![fixtures::item(GUID, json!(1))]);
    assert!(
        super::published::verify(
            &malformed.load()?,
            &catalog,
            &fixtures::build(),
            &[fixtures::PUBLICATION],
            &|| false
        )
        .is_err(),
        "malformed source GUIDs never disappear as successful missing joins"
    );
    let mut duplicate = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    duplicate.snapshot.fields.push(
        duplicate
            .snapshot
            .fields
            .first()
            .ok_or("field mapping absent")?
            .clone(),
    );
    duplicate.save()?;
    assert!(
        duplicate.load().is_err(),
        "duplicate comparison mappings cannot inflate agreement counts"
    );
    Ok(())
}

#[test]
fn a_selected_guid_missing_on_both_sides_is_not_an_optional_missing_field() -> super::Result<()> {
    let mut fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    fixture.snapshot.selection = vec![GUID.to_owned(), OTHER.to_owned()];
    fixture.save()?;
    let report = super::published::verify(
        &fixture.load()?,
        &fixtures::catalog(vec![fixtures::item(GUID, json!(1))]),
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        report.missing_both_guids, 1,
        "the absent selected identity has its own join diagnostic"
    );
    assert!(
        !report.succeeds(),
        "one equal field cannot replace another selected identity"
    );
    Ok(())
}

#[test]
fn local_walks_and_selector_work_remain_cancellable_and_bounded() -> super::Result<()> {
    let fixture = PublishedFixture::new(vec![fixtures::row(GUID, json!(1))])?;
    let root = p4kx_testkit::offline_seam::direct_root(&fixture.root)?;
    let calls = std::cell::Cell::new(0_u32);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 1
    };
    assert!(
        p4kx_testkit::offline_seam::inventory(
            &root,
            p4kx_testkit::offline_seam::Limits {
                path_bytes: 4096,
                entries: 10
            },
            None,
            cancelled
        )
        .is_err(),
        "cancellation is checked during enumeration rather than only at entry"
    );
    let path = Path(vec![
        Step::Field {
            name: "rows".to_owned(),
        },
        Step::Each,
    ]);
    let source = json!({ "rows": [1,2,3] });
    assert!(
        path.select(&source, &|| true).is_err(),
        "selector loops honor cancellation"
    );
    assert!(
        Path(vec![Step::Each; 65])
            .select(&source, &|| false)
            .is_err(),
        "selector depth is bounded"
    );
    assert_eq!(
        Path(Vec::new()).select(&source, &|| false)?.len(),
        1,
        "an explicit empty selector denotes the root"
    );
    Ok(())
}

#[test]
fn duplicate_schema_owners_and_shadowed_field_names_are_never_last_wins() -> super::Result<()> {
    use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};
    let duplicate = DataForgeBuilder::new()
        .structure(TestStruct::new("Duplicate").field("a", TestType::Single))
        .structure(TestStruct::new("Duplicate").field("b", TestType::Single))
        .build();
    let forge = p4kx_extraction::DataForge::open(&duplicate)?;
    assert!(
        super::schema::Schema::read(&forge, &["Duplicate".to_owned()], &|| false).is_err(),
        "duplicate schema owner names cannot replace an earlier declaration"
    );
    let shadowed = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("same", TestType::Single))
        .structure(
            TestStruct::new("Child")
                .parent("Base")
                .field("same", TestType::Single),
        )
        .build();
    let forge = p4kx_extraction::DataForge::open(&shadowed)?;
    assert!(
        super::schema::Schema::read(&forge, &["Child".to_owned()], &|| false).is_err(),
        "ambiguous inherited field names cannot select an arbitrary owning declaration"
    );
    Ok(())
}

#[test]
fn a_missing_selected_row_is_not_hidden_by_an_optional_field_absent_on_both_sides()
-> super::Result<()> {
    let fixture = PublishedFixture::new(vec![
        fixtures::row(GUID, json!(null)),
        fixtures::row(OTHER, json!(1)),
    ])?;
    let report = super::published::verify(
        &fixture.load()?,
        &fixtures::catalog(vec![fixtures::item(OTHER, json!(1))]),
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )?;
    assert_eq!(
        report.reference_only_guids, 1,
        "the selected source identity is missing from the typed export"
    );
    assert_eq!(
        report
            .fields
            .values()
            .map(|summary| summary.count(super::compare::Class::ReferenceOnly))
            .sum::<u64>(),
        1,
        "row absence remains a missing-identity result even when its optional field is absent"
    );
    assert!(
        !report.succeeds(),
        "another equal row cannot conceal the missing selected source"
    );
    Ok(())
}
