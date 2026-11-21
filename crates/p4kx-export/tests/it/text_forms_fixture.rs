// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{
    Archive, DataForge, IniEncoding, Language, LocalizationEntry, Value as SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestValue};
use serde_json::Value;

pub(crate) use crate::commodity_v3_fixture::{TestResult, at};
pub(crate) use crate::manufacturer_v3_fixture::{text, with_keys};

#[derive(Clone, Copy)]
pub(crate) enum Origin {
    Plain,
    Encrypted,
    Unknown,
}

pub(crate) fn original_keys(
    database: &DataForgeBuilder,
    name: &str,
    description: &str,
    count: usize,
) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let mut actual = 0;
    for record in forge.records() {
        assert_eq!(
            usize::try_from(record.index())?,
            actual,
            "the original physical owner index matches its declared Record occurrence"
        );
        let tree = record.tree()?;
        let Some(SourceValue::Struct(localization)) = tree.root().field("Localization") else {
            return Err("actual original Manufacturer localization".into());
        };
        assert_eq!(
            localization.field("Name"),
            Some(&SourceValue::Locale(name)),
            "the public name key is the actual original Locale field"
        );
        assert_eq!(
            localization.field("Description"),
            Some(&SourceValue::Locale(description)),
            "the protected description key is the actual original Locale field"
        );
        actual += 1;
    }
    assert_eq!(
        actual, count,
        "the source contains the intended physical owners"
    );
    Ok(())
}

pub(crate) fn repeated(key: &str) -> DataForgeBuilder {
    with_keys(key, "@LOC_EMPTY").record(
        crate::manufacturer_v3_fixture::record(
            0x21,
            "Second",
            "libs/foundry/records/scitemmanufacturer/second.xml",
        )
        .value("Code", TestValue::text("SECOND"))
        .value(
            "Localization",
            TestValue::Struct(
                TestInstance::new("ManufacturerLocalization")
                    .value("Name", TestValue::text(key))
                    .value("Description", TestValue::text("@LOC_EMPTY")),
            ),
        ),
    )
}

pub(crate) fn encoded(text: &str, encoding: IniEncoding) -> TestResult<Vec<u8>> {
    match encoding {
        IniEncoding::Utf8 => Ok(text.as_bytes().to_vec()),
        IniEncoding::Utf8Bom => {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend_from_slice(text.as_bytes());
            Ok(bytes)
        }
        IniEncoding::Utf16Le | IniEncoding::Utf16Be => {
            let little = encoding == IniEncoding::Utf16Le;
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            Ok(bytes)
        }
        _ => Err("unsupported synthetic encoding".into()),
    }
}

pub(crate) fn locale(language: Language, bytes: Vec<u8>, origin: Origin) -> TestEntry {
    let entry = TestEntry::stored(language.global_ini_path(), bytes);
    match origin {
        Origin::Plain => entry,
        Origin::Encrypted => entry.encrypted(),
        Origin::Unknown => entry.encrypted().with_encryption_field(7),
    }
}

pub(crate) fn open(
    database: &DataForgeBuilder,
    locales: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("text-forms.p4k");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database.build()))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ));
    for entry in locales {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}

pub(crate) fn original_locale(
    archive: &Archive,
    language: Language,
    key: &str,
    expected: Option<&str>,
    encoding: IniEncoding,
    lossy: bool,
) -> TestResult {
    let entry = LocalizationEntry::read(archive, language)?;
    assert_eq!(entry.text().encoding(), encoding, "actual reader encoding");
    assert_eq!(
        entry.text().is_lossy(),
        lossy,
        "actual reader decoding result"
    );
    assert_eq!(
        entry.parse()?.resolve(key),
        expected,
        "actual winning decoded RHS"
    );
    assert!(
        entry.origin().is_some(),
        "the source is the actual Archive Entry"
    );
    Ok(())
}

pub(crate) fn export(archive: &Archive) -> TestResult<(BuiltCatalog, Value)> {
    let mut options = crate::commodity_v3_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}
