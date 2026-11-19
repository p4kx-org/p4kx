// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    LocationEntityDeclarationSnapshot, LocationEntityKind,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Guid, RecordTree, TreeBuilder, Value,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type TagSlots = Vec<Option<Guid>>;
type PriorTags = BTreeMap<Guid, TagSlots>;
type SelectedDeclarations<'source> = BTreeMap<u32, (&'source str, TagSlots)>;
const SOURCE: &str = "LocationEntityDeclaration";
const STATIC: &str = "LocationEntityType_StaticEntity";
const DELIVERY: &str = "LocationEntityType_DeliveryTarget";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_location_entity_declarations_match_original_data() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let prior = real_archive_b_path()
        .map(|path| older_tags(path.as_path()))
        .transpose()?;
    let selected = select_declarations(&forge, &watch, prior.as_ref())?;
    let query = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = query.parsed(None)?;
    for (index, (type_name, tags)) in selected {
        parsed.with_location_entity_declarations(
            CatalogRecordSelection::RecordIndex(index),
            None,
            None,
            |snapshot| verify_projection(&snapshot, type_name, &tags),
        )??;
    }
    watch.check()?;
    Ok(())
}

fn select_declarations<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    prior: Option<&PriorTags>,
) -> TestResult<SelectedDeclarations<'source>> {
    let mut selected = BTreeMap::new();
    let mut changed = false;
    let mut trees = TreeBuilder::new();
    for record in forge
        .records()
        .filter(|record| record.type_name() == Ok(SOURCE))
    {
        watch.poll()?;
        let tree = trees.build(&record)?;
        let Some(Value::Struct(ty)) = tree.field("type") else {
            continue;
        };
        let ty = ty.type_name();
        let tags = original_tags(&tree)?;
        let differs = prior
            .and_then(|prior| prior.get(&record.guid()))
            .is_some_and(|prior| prior != &tags);
        let index = record.index();
        if matches!(ty, STATIC | DELIVERY) && !selected.values().any(|(kind, _)| *kind == ty) {
            selected.insert(index, (ty, tags.clone()));
        }
        if differs && !changed {
            selected.insert(index, (ty, tags));
            changed = true;
        }
        if selected.values().any(|(kind, _)| *kind == STATIC)
            && selected.values().any(|(kind, _)| *kind == DELIVERY)
            && (changed || prior.is_none())
        {
            break;
        }
    }
    assert!(
        selected.values().any(|(kind, _)| *kind == STATIC),
        "static witness"
    );
    assert!(
        selected.values().any(|(kind, _)| *kind == DELIVERY),
        "delivery witness"
    );
    assert!(
        prior.is_none() || changed,
        "changed source witness for the second Archive"
    );
    Ok(selected)
}

fn verify_projection(
    snapshot: &LocationEntityDeclarationSnapshot<'_>,
    type_name: &str,
    tags: &[Option<Guid>],
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("expected one projected physical row".into());
    };
    assert!(
        snapshot.coverage.incomplete_fields.is_empty(),
        "complete typed source"
    );
    let source = row.entity_type().value().ok_or("original strong pointer")?;
    assert!(
        source.source_type().value() == Some(&type_name),
        "source concrete type"
    );
    match type_name {
        STATIC => assert!(
            matches!(source.kind(), LocationEntityKind::StaticEntity),
            "the original static subtype selects the static projection"
        ),
        DELIVERY => assert!(
            matches!(source.kind(), LocationEntityKind::DeliveryTarget { .. }),
            "the original delivery subtype selects the delivery projection"
        ),
        _ => {}
    }
    let block = row
        .entity_tags()
        .value()
        .ok_or("original entity tag block")?;
    let actual = block.tags().value().ok_or("original tag list")?;
    assert_eq!(actual.len(), tags.len(), "tag array length");
    assert!(
        actual
            .iter()
            .zip(tags.iter().copied())
            .all(|(actual, expected)| {
                actual.value().and_then(|link| link.guid().value().copied()) == expected
            }),
        "ordered original tag targets"
    );
    Ok(())
}

fn original_tags(tree: &RecordTree<'_>) -> TestResult<TagSlots> {
    let Some(Value::Struct(block)) = tree.field("entityTags") else {
        return Err("original entity tags".into());
    };
    let Some(Value::Array(tags)) = block.field("tags") else {
        return Err("original tag array".into());
    };
    tags.iter()
        .map(|value| match value {
            Value::Reference(link) => Ok(Some(link.guid())),
            Value::Null => Ok(None),
            _ => Err("unexpected original tag slot".into()),
        })
        .collect()
}

fn older_tags(path: &std::path::Path) -> TestResult<PriorTags> {
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge: DataForge<'_> = entry.open()?;
    let mut trees = TreeBuilder::new();
    let mut rows = BTreeMap::new();
    for record in forge
        .records()
        .filter(|record| record.type_name() == Ok(SOURCE))
    {
        watch.poll()?;
        rows.insert(record.guid(), original_tags(&trees.build(&record)?)?);
    }
    watch.check()?;
    Ok(rows)
}
