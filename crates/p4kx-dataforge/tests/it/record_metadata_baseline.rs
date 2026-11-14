// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, DataForgeError, Guid, JsonLayout, RecordJson, RecordXml};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

type TestResult = Result<(), Box<dyn Error>>;

const GUID: u128 = 0x1111_2222_3333_4444_5555_6666_7777_8888;
const GUID_TEXT: &str = "11112222-3333-4444-5555-666677778888";

fn source(version: u32, record: TestRecord) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(version)
        .structure(TestStruct::new("HeaderProbe"))
        .record(record)
        .build()
}

fn record(path: &str) -> TestRecord {
    TestRecord::new(GUID, "HeaderProbe", "HeaderProbe.Name", path)
}

#[test]
fn record_metadata_baseline_keeps_legacy_tag_and_name_collapsing() -> TestResult {
    for (version, row, record_tag) in [
        (6, record("synthetic/Case.XML").tag("RecordDomain"), None),
        (8, record("synthetic/Case.XML").without_tag(), None),
        (8, record("synthetic/Case.XML").tag(""), Some("")),
        (
            8,
            record("synthetic/Case.XML").tag("RecordDomain"),
            Some("RecordDomain"),
        ),
    ] {
        let bytes = source(version, row.without_name());
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original Record")?;
        assert_eq!(
            record.name()?,
            "",
            "the existing name accessor retains its sentinel contract"
        );
        assert_eq!(
            record.tag()?,
            record_tag,
            "the existing tag accessor retains its version/sentinel contract"
        );
        let tree = record.tree()?;
        assert_eq!(
            tree.name(),
            "",
            "the canonical name contract stays unchanged"
        );
        assert_eq!(
            tree.tag(),
            record_tag.filter(|tag| !tag.is_empty()),
            "the canonical tree keeps its existing empty-tag filter"
        );
    }
    Ok(())
}

#[test]
fn record_metadata_baseline_normalizes_only_a_trailing_xml_extension() -> TestResult {
    for (stored, normalized) in [
        ("Synthetic\\Case.XML", "Synthetic\\Case"),
        ("Synthetic/Mixed.xml", "Synthetic/Mixed"),
        ("Synthetic/Mixed", "Synthetic/Mixed"),
        ("Synthetic/Mixed.xml.extra", "Synthetic/Mixed.xml.extra"),
    ] {
        let bytes = source(8, record(stored));
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original Record")?;
        assert_eq!(
            record.record_path()?,
            normalized,
            "case and separator spelling remain original"
        );
        assert_eq!(
            record.tree()?.record_path(),
            normalized,
            "tree normalization follows the same format rule"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum MetadataWord {
    Name,
    FileName,
    Tag,
}

// V8 stores name, filename, tag and struct words before the GUID.
impl MetadataWord {
    const fn words_before_guid(self) -> usize {
        match self {
            Self::Name => 4,
            Self::FileName => 3,
            Self::Tag => 2,
        }
    }
    const fn invalid_offset(self) -> u32 {
        u32::MAX
            - match self {
                Self::Name => 2,
                Self::FileName => 1,
                Self::Tag => 3,
            }
    }
}

fn invalid_metadata(
    mut bytes: Vec<u8>,
    fields: &[MetadataWord],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let stored = GUID_TEXT.parse::<Guid>()?.stored();
    let mut positions = bytes
        .windows(stored.len())
        .enumerate()
        .filter_map(|(index, value)| (value == stored).then_some(index));
    let guid = positions.next().ok_or("unique synthetic Record GUID")?;
    if positions.next().is_some() {
        return Err("the synthetic Record GUID must be unique".into());
    }
    for field in fields {
        let start = guid
            .checked_sub(field.words_before_guid() * size_of::<u32>())
            .ok_or("v8 Record metadata word")?;
        bytes
            .get_mut(start..start + size_of::<u32>())
            .ok_or("v8 Record metadata word")?
            .copy_from_slice(&field.invalid_offset().to_le_bytes());
    }
    Ok(bytes)
}

#[test]
fn record_metadata_baseline_keeps_filename_then_name_then_tag_error_priority() -> TestResult {
    for fields in [
        vec![
            MetadataWord::FileName,
            MetadataWord::Name,
            MetadataWord::Tag,
        ],
        vec![MetadataWord::Name, MetadataWord::Tag],
        vec![MetadataWord::Tag],
    ] {
        let first = *fields.first().ok_or("first invalid field")?;
        let bytes = invalid_metadata(
            source(8, record("synthetic/Case.XML").tag("RecordDomain")),
            &fields,
        )?;
        let forge = DataForge::open(&bytes)?;
        let result = forge.record(0).ok_or("original Record")?.tree();
        assert!(
            matches!(result, Err(DataForgeError::InvalidString { offset, .. }) if offset == first.invalid_offset()),
            "the canonical read preserves its original metadata error order"
        );
    }
    Ok(())
}

#[test]
fn record_metadata_baseline_keeps_standalone_json_and_xml_bytes() -> TestResult {
    let bytes = source(8, record("synthetic/Case.XML").tag("RecordDomain"));
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original Record")?.tree()?;
    let (mut json, mut xml) = (Vec::new(), Vec::new());
    RecordJson::new(JsonLayout::Compact).write(&tree, &mut json)?;
    RecordXml::new().write(&tree, &mut xml)?;
    assert_eq!(
        String::from_utf8(json)?,
        format!(
            r#"{{"$type":"HeaderProbe","$guid":"{GUID_TEXT}","$path":"synthetic/Case","$name":"HeaderProbe.Name","$tag":"RecordDomain"}}"#
        ),
        "raw metadata enrichment does not extend the standalone JSON dialect"
    );
    assert_eq!(
        String::from_utf8(xml)?,
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<HeaderProbe xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" guid=\"{GUID_TEXT}\" path=\"synthetic/Case\" p4kx:name=\"HeaderProbe.Name\" p4kx:tag=\"RecordDomain\"/>\n"
        ),
        "raw metadata enrichment does not extend the standalone XML dialect"
    );
    Ok(())
}

#[test]
fn record_metadata_baseline_keeps_layout_admission_before_metadata_errors() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("HeaderProbe").declared_size(1))
        .record(record("synthetic/Case.XML"))
        .build();
    let bytes = invalid_metadata(bytes, &[MetadataWord::FileName])?;
    let forge = DataForge::open(&bytes)?;
    let result = forge.record(0).ok_or("original Record")?.tree();
    assert!(
        matches!(
            result,
            Err(DataForgeError::StructLayout {
                declared: 1,
                fields: 0,
                ..
            })
        ),
        "the existing layout rejection retains priority over invalid metadata"
    );
    Ok(())
}
