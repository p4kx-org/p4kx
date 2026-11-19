// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, SourceInventory, SourceLocation, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
};
use serde_json::{Value, json};

use super::{FlareElement, FlareField, FlareLibrary, FlareNodeRole, FlareValue};
use crate::catalog_queries::flares::{FlareLibrarySelection, read_flare_libraries};
use crate::catalog_table::DatasetAvailability;
use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};

#[path = "flare_library_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const OLD: &str = "Data/Libs/Flares/older.xml";
const NEW: &str = "Data/Libs/Flares/newer.XML";
const TEXTURE: &str = "Data/textures/fixture.dds";
const ITEM_ID: &str = "{bd885042-3319-4bf1-841a-be7487f50a40}";

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let archive_path = folder.path().join("Data.p4k");
    let old = "<LensFlareLibrary Name='older'><FlareItem><Params><LensGhost \
        Texture='textures/fixture.dds'/></Params></FlareItem></LensFlareLibrary>";
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(OLD, old))
        .entry(TestEntry::stored(NEW, replacement()).encrypted())
        .entry(TestEntry::stored(TEXTURE, "directory target").encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/FlaresExtra/unselected.xml",
            old,
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open_with_key(archive_path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn replacement() -> Vec<u8> {
    let item = TestElement::new("FlareItem")
        .attribute("Id", ITEM_ID)
        .attribute("Name", "repeat")
        .attribute("Enable", "1");
    TestElement::new("LensFlareLibrary")
        .attribute("Name", "newer")
        .child(
            item.clone().child(
                TestElement::new("FlareItem")
                    .attribute("Type", "nested")
                    .child(TestElement::new("FutureEffect").attribute("Future", "retained"))
                    .child(parameters()),
            ),
        )
        .child(item)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn parameters() -> TestElement {
    TestElement::new("Params")
        .child(
            TestElement::new("Common")
                .attribute("Brightness", "4.125")
                .attribute("Size", "NaN")
                .attribute("Position", "01.0,-2.5")
                .attribute("Tint", ""),
        )
        .child(TestElement::new("LensGhost").attribute("Texture", "textures/fixture.dds"))
        .child(TestElement::new("MultiGhost").attribute("Texture", "textures/missing.dds"))
}

#[test]
fn narrow_flare_query_preserves_repeated_items_and_nested_unknown_markup() -> TestResult {
    let (_folder, archive) = fixture()?;
    let all = read_flare_libraries(&archive, FlareLibrarySelection::All, None)?;
    assert_eq!(all.rows.len(), 2, "both physical libraries are retained");
    let old = all.rows.first().ok_or("ordinary XML library")?;
    let new = all.rows.get(1).ok_or("replacement CryXml library")?;
    assert_plain_payload_and_encrypted_texture(old, &all.source_inventory)?;
    assert_repeated_items_and_unknown_markup(new)?;
    let selected = read_flare_libraries(
        &archive,
        FlareLibrarySelection::ArchivePath(r"data\LIBS\Flares\newer.xml"),
        None,
    )?;
    assert_eq!(
        selected.rows.len(),
        1,
        "the exact normalized path narrows the read"
    );
    assert!(matches!(
        selected.availability,
        DatasetAvailability::Available
    ));
    assert_eq!(selected.rows[0].elements().len(), new.elements().len());
    Ok(())
}

fn assert_plain_payload_and_encrypted_texture(
    old: &FlareLibrary,
    inventory: &SourceInventory,
) -> TestResult {
    let old_item = old.elements().get(1).ok_or("older item")?;
    assert_eq!(
        field(old_item, "Id")?.value().state(),
        SourceValueState::Absent
    );
    assert_eq!(
        field(old_item, "Id")?
            .value()
            .origin()
            .contains_encrypted_source(),
        Some(false)
    );
    let old_texture = field(node(old.elements(), "LensGhost")?, "Texture")?;
    assert_eq!(
        old_texture.value().origin().contains_encrypted_source(),
        Some(false),
    );
    let texture = old_texture.texture().ok_or("directory texture link")?;
    assert_eq!(
        texture.state().origin().contains_encrypted_source(),
        Some(false),
        "the lookup only consumes directory metadata"
    );
    let target = texture
        .targets()
        .first()
        .and_then(p4kx_extraction::SourcedValue::value)
        .ok_or("physical texture target")?;
    let source = inventory
        .sources()
        .iter()
        .find(|source| source.id() == target.input_ref())
        .ok_or("retained texture Entry descriptor")?;
    assert_eq!(
        serde_json::to_value(source)?["storedEncryption"],
        json!("encrypted"),
        "the directory separately describes the encrypted target payload"
    );
    Ok(())
}

fn assert_repeated_items_and_unknown_markup(new: &FlareLibrary) -> TestResult {
    let nodes = new.elements();
    let items: Vec<_> = nodes
        .iter()
        .filter(|node| node.role().value() == Some(&FlareNodeRole::Item))
        .collect();
    assert_eq!(
        items.len(),
        2,
        "repeated authored GUIDs keep both occurrences"
    );
    for item in items {
        assert!(matches!(
            field(item, "Id")?.value().value(),
            Some(FlareValue::Guid(_))
        ));
        assert_eq!(
            field(item, "Id")?
                .value()
                .origin()
                .contains_encrypted_source(),
            Some(true)
        );
    }
    let future = node(nodes, "FutureEffect")?;
    assert_eq!(future.role().value(), Some(&FlareNodeRole::Other));
    assert_eq!(future.authored().parent().value(), Some(&Some(2)));
    assert!(future.fields().is_empty());
    let declaration = future
        .authored()
        .declarations()
        .first()
        .ok_or("unknown attribute")?;
    assert_eq!(
        declaration.name().value().map(String::as_str),
        Some("Future")
    );
    assert_eq!(
        declaration.value().value().value().map(String::as_str),
        Some("retained")
    );
    Ok(())
}

#[test]
fn flare_parameters_preserve_finite_empty_invalid_and_directory_link_outcomes() -> TestResult {
    let (_folder, archive) = fixture()?;
    let selected = read_flare_libraries(&archive, FlareLibrarySelection::ArchivePath(NEW), None)?;
    let elements = selected.rows.first().ok_or("CryXml library")?.elements();
    let common = node(elements, "Common")?;
    assert_eq!(
        field(common, "Brightness")?.value().value(),
        Some(&FlareValue::Number(4.125))
    );
    assert_eq!(
        field(common, "Size")?.value().state(),
        SourceValueState::Invalid
    );
    assert_eq!(
        field(common, "Position")?.value().value(),
        Some(&FlareValue::Pair([1.0, -2.5]))
    );
    assert_eq!(
        field(common, "Tint")?.value().value(),
        Some(&FlareValue::Empty)
    );
    assert_eq!(
        field(common, "Orbitangle")?.value().state(),
        SourceValueState::Absent
    );
    for (name, disposition, count) in [("LensGhost", "resolved", 1), ("MultiGhost", "missing", 0)] {
        let texture = field(node(elements, name)?, "Texture")?;
        let projected = serde_json::to_value(texture)?;
        assert_eq!(projected["texture"]["state"]["value"], disposition);
        assert_eq!(
            projected["texture"]["targets"].as_array().map(Vec::len),
            Some(count)
        );
    }
    Ok(())
}

#[test]
fn authored_flare_rows_match_the_full_catalog_table() -> TestResult {
    let (_folder, archive) = fixture()?;
    let options = CatalogOptions::new(UNIX_EPOCH);
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let selected = read_flare_libraries(&archive, FlareLibrarySelection::All, None)?;
    let rows = catalog
        .pointer("/definitions/flareLibraries/rows")
        .and_then(Value::as_array)
        .ok_or("canonical flare library rows")?;
    let scoped = serde_json::to_value(&selected.rows)?;
    assert_eq!(rows.len(), selected.rows.len());
    assert_eq!(strip_refs(Value::Array(rows.clone())), strip_refs(scoped));
    Ok(())
}

#[test]
fn required_flare_failures_are_counted_and_refuse_normal_publication() -> TestResult {
    const BAD_CRC: &str = "Data/Libs/Flares/bad-crc.xml";
    const MALFORMED: &str = "Data/Libs/Flares/malformed.xml";
    const OTHER_ROOT: &str = "Data/Libs/Flares/unrelated.xml";
    const LIBRARY: &[u8] = b"<LensFlareLibrary><FlareItem/></LensFlareLibrary>";

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(OLD, LIBRARY))
        .entry(TestEntry::stored(BAD_CRC, LIBRARY).with_crc32c(1))
        .entry(TestEntry::stored(MALFORMED, b"<LensFlareLibrary"))
        .entry(TestEntry::stored(OTHER_ROOT, b"<Unrelated/>"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    for (path, failed_reads, failed_projections) in [(BAD_CRC, 1, 0), (MALFORMED, 0, 1)] {
        let selected =
            read_flare_libraries(&archive, FlareLibrarySelection::ArchivePath(path), None)?;
        assert!(matches!(
            selected.availability,
            DatasetAvailability::Unavailable
        ));
        assert!(selected.rows.is_empty());
        assert_eq!(selected.entries.failed_required_reads, failed_reads);
        assert_eq!(selected.entries.failed_projections, failed_projections);
    }
    let skipped = read_flare_libraries(
        &archive,
        FlareLibrarySelection::ArchivePath(OTHER_ROOT),
        None,
    )?;
    assert!(matches!(skipped.availability, DatasetAvailability::Absent));
    assert_eq!(skipped.entries.intentionally_skipped, 1);

    let mut options = CatalogOptions::new(UNIX_EPOCH);
    assert!(matches!(
        build_catalog(&archive, &options),
        Err(CatalogError::IncompleteExport(_))
    ));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let health = catalog
        .pointer("/exportHealth/kinds")
        .and_then(Value::as_array)
        .ok_or("catalog health kinds")?
        .iter()
        .find(|kind| kind["kind"] == "flareLibraries")
        .ok_or("flare library health")?;
    assert_eq!(health["sourceMode"], json!("archiveEntries"));
    assert_eq!(health["writtenRows"], json!(1));
    assert_eq!(health["sourceEntries"]["failedRequiredReads"], json!(1));
    assert_eq!(health["sourceEntries"]["failedProjections"], json!(1));
    Ok(())
}

#[test]
fn missing_flare_entry_index_only_uses_directory_count() -> TestResult {
    let (_folder, archive) = fixture()?;
    let snapshot =
        read_flare_libraries(&archive, FlareLibrarySelection::EntryIndex(u32::MAX), None)?;
    assert!(matches!(snapshot.availability, DatasetAvailability::Absent));
    assert!(snapshot.rows.is_empty());
    let [id] = snapshot.directory.source_uses_refs() else {
        return Err("one direct directory proof for an out-of-range Entry".into());
    };
    let proof = snapshot
        .source_inventory
        .uses()
        .iter()
        .find(|usage| usage.id() == id)
        .ok_or("directory proof in the scoped inventory")?;
    assert!(matches!(
        proof.location(),
        SourceLocation::Metadata { field } if field == "entryCount"
    ));
    Ok(())
}

fn strip_refs(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            fields.remove("sourceUsesRefs");
            fields.remove("inputRef");
            for field in fields.values_mut() {
                *field = strip_refs(std::mem::take(field));
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = strip_refs(std::mem::take(item));
            }
        }
        _ => {}
    }
    value
}

fn node<'library>(
    elements: &'library [FlareElement],
    name: &str,
) -> TestResult<&'library FlareElement> {
    Ok(elements
        .iter()
        .find(|node| {
            node.authored()
                .name()
                .value()
                .is_some_and(|value| value == name)
        })
        .ok_or("expected authored element")?)
}

fn field<'library>(element: &'library FlareElement, key: &str) -> TestResult<&'library FlareField> {
    Ok(element
        .fields()
        .iter()
        .find(|field| field.key() == key)
        .ok_or("expected typed field")?)
}
