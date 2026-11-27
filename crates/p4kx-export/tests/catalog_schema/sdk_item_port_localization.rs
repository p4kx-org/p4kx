// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::{Archive, Guid, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestInstance, TestRecord, TestValue};

use super::{TestResult, alignment, full_proof};

const LANGUAGE: Language = Language::English;
const OWNER_GUID: &str = "00000000-0000-0000-0000-000000001087";
const ALIAS: u32 = 2;
const UNRELATED: u32 = 3;
const UNREADABLE: u32 = 4;

#[test]
fn item_ports_load_requested_localization_after_owner_admission() -> TestResult<()> {
    let broken_ini = TestEntry::zstd(LANGUAGE.global_ini_path(), fixture_ini())
        .with_payload([0x28, 0xb5, 0x2f, 0xfd]);
    let (_directory, archive) = port_archive(broken_ini)?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[LANGUAGE]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    let absent_guid = Guid::from_stored([u8::MAX; 16]);
    for (selection, unreadable) in [
        (CatalogRecordSelection::Guid(absent_guid), &[][..]),
        (CatalogRecordSelection::RecordIndex(u32::MAX), &[][..]),
        (CatalogRecordSelection::RecordIndex(UNRELATED), &[][..]),
        (
            CatalogRecordSelection::RecordIndex(UNREADABLE),
            &[UNREADABLE][..],
        ),
    ] {
        parsed.with_item_ports(selection, None, |snapshot| {
            assert!(
                matches!(snapshot.availability, DatasetAvailability::Available),
                "the original port family exists when the owner selection yields no row",
            );
            assert!(
                snapshot.rows.is_empty(),
                "the selected scope has no readable owner"
            );
            assert_eq!(
                snapshot.requested_languages,
                [LANGUAGE],
                "keep the requested policy"
            );
            assert!(
                snapshot.available_languages.is_empty() && snapshot.localization_notes.is_empty(),
                "an empty scope must not read or index the malformed requested Entry",
            );
            assert_eq!(
                snapshot.source_inventory.use_count(),
                0,
                "there are no contributing source uses in the empty scope",
            );
            assert_eq!(
                snapshot.coverage.unreadable_bodies, unreadable,
                "physical absence remains distinct from an unreadable selected Record",
            );
        })?;
    }
    let selection = CatalogRecordSelection::Guid(OWNER_GUID.parse()?);
    let result = parsed.with_item_ports(selection, None, |_| ());
    assert!(
        matches!(result, Err(CatalogError::LoadTexts(_))),
        "a populated matching owner must still report its unreadable requested locale",
    );
    Ok(())
}

#[test]
fn item_ports_preserve_localization_and_physical_guid_aliases() -> TestResult<()> {
    let (_directory, archive) =
        port_archive(TestEntry::stored(LANGUAGE.global_ini_path(), fixture_ini()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[LANGUAGE]),
        None,
    )?
    .with_provenance_mode(full_proof::MODE);
    let selection = CatalogRecordSelection::Guid(OWNER_GUID.parse()?);
    session.with_item_ports(selection, None, |snapshot| {
        let [first, alias] = snapshot.rows.as_slice() else {
            return Err("selected GUID must preserve both physical port containers".into());
        };
        assert_eq!(
            [
                first.owner_record_index().value(),
                alias.owner_record_index().value()
            ],
            [Some(&0), Some(&ALIAS)],
            "the two aliases keep their physical Record order",
        );
        assert_eq!(
            snapshot.available_languages,
            [LANGUAGE],
            "load the requested valid locale"
        );
        let label = snapshot
            .public_names
            .iter()
            .find(|row| {
                row.key
                    .value()
                    .is_some_and(|key| key == alignment::PORT_KEY)
            })
            .ok_or("the selected port retains its authored public-name key")?;
        let english = label
            .translations
            .value()
            .and_then(|translations| translations.get(LANGUAGE.code()))
            .ok_or("the selected port key resolves in the requested language")?;
        let display = english.display_text.value().map(std::borrow::Cow::as_ref);
        assert_eq!(
            display,
            Some("Fixture port label"),
            "preserve the translated label"
        );
        assert!(
            snapshot
                .public_names
                .iter()
                .any(|row| { row.key.value().is_some_and(|key| key == "@LOC_EMPTY") }),
            "the other authored placeholder remains in the selected name table",
        );
        assert!(
            snapshot.source_inventory.use_count() > 0,
            "keep original port and locale evidence"
        );
        Ok(())
    })?
}

fn port_archive(locale: TestEntry) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("localized-item-ports.p4k");
    let database = alignment::database(alignment::Layout::Localized)
        .record(port_owner(alignment::OWNER, "alias"))
        .record(TestRecord::new(
            42,
            "DataForgeComponentParams",
            "DataForgeComponentParams.Unrelated",
            "unrelated.xml",
        ))
        .record(port_owner(43, "unreadable").with_instance_index(u16::MAX));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .entry(locale)
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn port_owner(guid: u128, name: &str) -> TestRecord {
    let container = TestInstance::new("SItemPortContainerComponentParams")
        .value("Ports", TestValue::Array(Vec::new()));
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.Fixture_{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{name}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(container)]),
    )
}

fn fixture_ini() -> String {
    format!(
        "{}=Fixture port label\n",
        alignment::PORT_KEY.trim_start_matches('@')
    )
}
