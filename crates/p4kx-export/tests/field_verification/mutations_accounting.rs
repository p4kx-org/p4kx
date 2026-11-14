// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{Archive, DataForge, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::accounting;
use super::fixtures::{self, GUID};
use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::Path;

fn survey(
    changed_target: bool,
    populated_empty: bool,
) -> super::Result<BTreeMap<FieldKey, accounting::Counts>> {
    let bytes = fixtures::probe_database(changed_target, populated_empty);
    let forge = DataForge::open(&bytes)?;
    super::coverage::run(
        &forge,
        &fixtures::probe_inventory(),
        &projected(),
        BTreeSet::new(),
        BTreeMap::new(),
        None,
        &|| false,
    )
}

fn projected() -> Value {
    json!({"items": [{"guid": GUID, "stats": {"probe": {"value": 7, "amount": 2, "weights": [0,3], "future": null}}}]})
}

#[test]
fn declared_present_populated_and_primitive_array_elements_have_independent_counts()
-> super::Result<()> {
    let counts = survey(false, false)?;
    let container = counts
        .get(&FieldKey::new("SyntheticParams", "weights"))
        .ok_or("array field absent")?;
    assert!(
        container.declared,
        "the source schema declares the array even before reading instances"
    );
    assert!(
        container.array,
        "array shape is recorded independently of element population"
    );
    assert_eq!(container.present, 1, "the array field is present once");
    let mut key = FieldKey::new("SyntheticParams", "weights");
    key.array_element = true;
    let elements = counts.get(&key).ok_or("primitive array elements absent")?;
    assert_eq!(
        elements.present, 2,
        "every primitive array element is surveyed"
    );
    assert_eq!(
        elements.populated, 2,
        "numeric zero is populated source data"
    );
    assert_eq!(
        elements.exported, 2,
        "primitive elements map to all projected elements"
    );
    let future = counts
        .get(&FieldKey::new("SyntheticParams", "future"))
        .ok_or("future field absent")?;
    assert!(
        future.declared,
        "the empty resource family is schema-declared"
    );
    assert_eq!(
        future.present, 1,
        "the null pointer is still a present field"
    );
    assert_eq!(
        future.populated, 0,
        "declared absence is not observed data loss"
    );
    let missing = counts
        .get(&FieldKey::new("SyntheticParams", "missing"))
        .ok_or("missing link absent")?;
    assert_eq!(
        missing.missing_targets, 1,
        "real missing targets have their own source category"
    );
    assert_eq!(
        missing.read_errors, 0,
        "missing targets are not parser/layout failures"
    );
    Ok(())
}

#[test]
fn a_declared_polymorphic_target_change_does_not_imply_concrete_value_loss() -> super::Result<()> {
    let counts = survey(true, false)?;
    let target = counts
        .get(&FieldKey::new("SyntheticParams", "variant"))
        .ok_or("variant absent")?;
    assert!(
        target.target == accounting::Target::Changed,
        "the declared base target changed independently of observed instances"
    );
    let concrete = counts
        .get(&FieldKey::new("SyntheticConcrete", "value"))
        .ok_or("concrete value absent")?;
    assert_eq!(
        concrete.populated, 1,
        "the familiar concrete instance remains readable"
    );
    assert_eq!(
        concrete.exported, 1,
        "supported concrete data remains represented"
    );
    assert_eq!(
        concrete.unhandled, 0,
        "a base-target change is not falsely labelled observed value loss"
    );
    Ok(())
}

#[test]
fn a_previously_empty_family_becoming_populated_requires_review() -> super::Result<()> {
    let counts = survey(false, true)?;
    assert!(
        counts
            .get(&FieldKey::new("SyntheticParams", "future"))
            .is_some_and(|count| count.population == accounting::Population::NewlyPopulated),
        "the baseline detects a schema-backed family acquiring concrete data"
    );
    assert!(
        accounting::regressions(&counts) > 0,
        "new population cannot silently preserve a green baseline"
    );
    Ok(())
}

#[test]
fn a_successful_public_mapper_cannot_hide_a_populated_nested_block() -> super::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixtures::healthy_probe_database(),
        ))
        .entry(TestEntry::stored(
            "Data/Localization/english/global.ini",
            p4kx_testkit::GlobalIniBuilder::new().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let built = p4kx_export::build_catalog(
        &archive,
        &p4kx_export::CatalogOptions::new(std::time::UNIX_EPOCH),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let loaded = DataForgeEntry::read(&archive)?;
    let counts = super::coverage::run(
        &loaded.open()?,
        &fixtures::probe_inventory(),
        &catalog,
        built
            .summary()
            .failures()
            .iter()
            .map(|failure| failure.guid().parse())
            .collect::<Result<_, _>>()?,
        BTreeMap::new(),
        None,
        &|| false,
    )?;
    let silent = counts
        .get(&FieldKey::new("SyntheticNested", "amount"))
        .ok_or("nested data absent")?;
    assert_eq!(
        silent.populated, 1,
        "canonical reading exposes the nested source value"
    );
    assert_eq!(
        silent.exported, 0,
        "the typed catalog omits the unfamiliar stats block"
    );
    assert!(
        silent.unhandled > 0,
        "a successful exit is not proof of field retention"
    );
    Ok(())
}

#[test]
fn generic_readability_and_a_real_typed_mapper_rejection_are_counted_separately()
-> super::Result<()> {
    let tagged = super::tagged_fixture::read()?;
    assert!(
        tagged.rejected.contains(&tagged.guid),
        "the public typed mapper rejects the populated unknown variant"
    );
    let key = FieldKey::new("MissionProperty", "value");
    let inventory = Inventory {
        schema_version: 1,
        roots: vec!["MissionProperty".to_owned()],
        fields: vec![Mapping {
            source: key.clone(),
            declared_target: Some("MissionPropertyValue".to_owned()),
            previously_empty: false,
            disposition: Disposition::Exported {
                group: "contractTemplates".to_owned(),
                path: Path::fields(&["objectiveTokens"]),
            },
        }],
    };
    let counts = accounting::count_fields(
        &tagged.schema,
        &tagged.observations,
        &inventory,
        &accounting::Projection {
            groups: BTreeMap::new(),
            rejected: tagged.rejected,
            exclusions: BTreeMap::new(),
        },
        &|| false,
    )?;
    let observed = counts.get(&key).ok_or("tagged value observation absent")?;
    assert!(
        observed.populated > 0,
        "generic reading succeeds on a populated tagged value"
    );
    assert!(
        observed.rejected > 0,
        "typed rejection is a distinct projection result"
    );
    assert_eq!(
        observed.read_errors, 0,
        "mapper rejection is not a generic parser error"
    );
    Ok(())
}

#[test]
fn unreviewed_declarations_exclusions_and_inventory_errors_are_visible() -> super::Result<()> {
    let bytes = fixtures::probe_database(false, false);
    let forge = DataForge::open(&bytes)?;
    let mut inventory = fixtures::probe_inventory();
    inventory
        .fields
        .retain(|mapping| mapping.source != FieldKey::new("SyntheticFuture", "amount"));
    let counts = super::coverage::run(
        &forge,
        &inventory,
        &projected(),
        BTreeSet::new(),
        BTreeMap::new(),
        None,
        &|| false,
    )?;
    assert!(
        counts
            .get(&FieldKey::new("SyntheticFuture", "amount"))
            .is_some_and(|count| count.review == accounting::Review::Unreviewed),
        "even an unobserved declared field must have a reviewed disposition"
    );
    let excluded = BTreeMap::from([(
        super::joins::guid(GUID)?,
        "explicit source selection exclusion".to_owned(),
    )]);
    let counts = super::coverage::run(
        &forge,
        &inventory,
        &projected(),
        BTreeSet::new(),
        excluded,
        None,
        &|| false,
    )?;
    assert!(
        counts
            .get(&FieldKey::new("SyntheticConcrete", "value"))
            .is_some_and(|count| count.excluded > 0 && count.missing_export_guid == 0),
        "selection exclusion is not a missing export-GUID join"
    );
    inventory
        .fields
        .push(inventory.fields.first().ok_or("mapping absent")?.clone());
    assert!(
        inventory.reviewed().is_err(),
        "duplicate mapping identities are rejected"
    );
    Ok(())
}
