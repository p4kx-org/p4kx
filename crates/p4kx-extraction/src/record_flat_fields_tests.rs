// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io;

use p4kx_dataforge::{DataForge, DataForgeHeaderField, FlatFormat, FlatValues, TreeBuilder};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};

use super::super::stream_fields::{StreamAddress, StreamField};
use super::write_flat;
use crate::{
    Archive, Language, LocalizationSet, RecordLocalization, SourceLocation, SourceUseRole,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn csv_part_offsets_and_original_header_fields_survive_an_existing_prefix() -> TestResult {
    for version in [6, 8] {
        let bytes = DataForgeBuilder::new()
            .version(version)
            .structure(TestStruct::new("Item").field("value", TestType::Int32))
            .record(
                TestRecord::new(1, "Item", "Item.Named", "item/named.xml")
                    .tag("actual tag")
                    .value("value", TestValue::Int32(36)),
            )
            .build();
        let forge = DataForge::open(&bytes)?;
        let original = forge.records().next().ok_or("source Record")?;
        let record = TreeBuilder::new().build_positioned_partial(&original)?;
        let mut out = b"earlier rows in this Part\n".to_vec();
        let (rows, fields) = write_flat(
            &forge,
            &mut FlatValues::new(FlatFormat::CsvRaw),
            &record,
            &mut out,
            None,
        )?;
        assert_eq!(rows, 1, "there is one emitted leaf");
        for (column, emitted, original) in [
            (
                "record_name",
                b"Item.Named".as_slice(),
                b"Item.Named".as_slice(),
            ),
            (
                "record_path",
                b"item/named".as_slice(),
                b"item/named.xml".as_slice(),
            ),
        ] {
            let field = text_field(&fields, 0, column)?;
            assert_eq!(
                cell(&out, field)?,
                emitted,
                "actual Part-relative {column} span"
            );
            assert!(
                original_span(&bytes, field, "text", original),
                "authored {column} bytes"
            );
        }
        assert_tag(version, &bytes, &out, text_field(&fields, 0, "record_tag")?)?;
    }
    Ok(())
}

fn assert_tag(version: u32, data: &[u8], out: &[u8], tag: &StreamField<'_>) -> TestResult {
    assert!(tag.complete, "absence or the authored tag is known");
    if version == 6 {
        assert!(cell(out, tag)?.is_empty(), "v6 emits no tag bytes");
        let version = DataForgeHeaderField::Version.byte_span();
        assert!(
            tag.sites.iter().flatten().any(|site| matches!(
                &site.location, SourceLocation::Subresource { namespace, field, indices }
                    if namespace == "dataforgeDecodedBytes" && field == "slot"
                        && indices.as_slice() == [version.start, version.end]
            ) && site.role == SourceUseRole::Selector),
            "the layout version explains why no tag word exists"
        );
    } else {
        assert_eq!(cell(out, tag)?, b"actual tag", "stored tag output");
        assert!(
            original_span(data, tag, "text", b"actual tag"),
            "authored tag bytes"
        );
    }
    Ok(())
}

#[test]
fn reference_targets_use_selected_member_evidence_and_keep_dangling_storage() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Holder")
                .field("present", TestType::Reference)
                .field("dangling", TestType::Reference),
        )
        .structure(TestStruct::new("Target"))
        .record(
            TestRecord::new(1, "Holder", "Holder.Source", "item/source.xml")
                .value("present", TestValue::Reference(2))
                .value("dangling", TestValue::Reference(3)),
        )
        .record(TestRecord::new(
            2,
            "Target",
            "Target.Selected",
            "item/selected.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let original = forge.records().next().ok_or("source Record")?;
    let record = TreeBuilder::new().build_positioned_partial(&original)?;
    let mut out = Vec::new();
    let (_, fields) = write_flat(
        &forge,
        &mut FlatValues::new(FlatFormat::CsvRaw),
        &record,
        &mut out,
        None,
    )?;
    let selected = text_field(&fields, 0, "target_path")?;
    assert_eq!(
        cell(&out, selected)?,
        b"item/selected",
        "selected target path"
    );
    assert!(
        original_span(&bytes, selected, "text", b"item/selected.xml"),
        "target filename bytes"
    );
    let dangling = text_field(&fields, 1, "target_path")?;
    assert!(
        cell(&out, dangling)?.is_empty() && !dangling.complete,
        "unproved missing target"
    );
    let guid = text_field(&fields, 1, "value")?;
    assert_eq!(
        cell(&out, guid)?,
        b"00000000-0000-0000-0000-000000000003",
        "stored dangling GUID"
    );
    assert!(guid.complete, "the stored GUID is readable");
    Ok(())
}

#[test]
fn flat_text_keeps_the_actual_lookup_for_blank_cells_and_skips_placeholder_lookup() -> TestResult {
    let data = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Item")
                .field("present", TestType::Locale)
                .field("empty", TestType::Locale)
                .field("missing", TestType::Locale)
                .field("placeholder", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "Item", "Item.Texts", "item/texts.xml")
                .value("present", TestValue::text("@present"))
                .value("empty", TestValue::text("@empty"))
                .value("missing", TestValue::text("@missing"))
                .value("placeholder", TestValue::text("@LOC_EMPTY")),
        )
        .build();
    let forge = DataForge::open(&data)?;
    let original = forge.records().next().ok_or("source Record")?;
    let record = TreeBuilder::new().build_positioned_partial(&original)?;
    let texts = language_fixture()?;
    let locale =
        RecordLocalization::try_new(texts.entry(Language::English).ok_or("English Entry")?)?;
    let mut out = Vec::new();
    let (_, fields) = write_flat(
        &forge,
        &mut FlatValues::new(FlatFormat::CsvRaw),
        &record,
        &mut out,
        Some(&locale),
    )?;
    for (row, expected) in [
        (0, b"translated".as_slice()),
        (1, b"".as_slice()),
        (2, b"".as_slice()),
    ] {
        let text = text_field(&fields, row, "text")?;
        assert_eq!(
            cell(&out, text)?,
            expected,
            "display text of observed lookup {row}"
        );
        let lookup = text.lookup.as_ref().ok_or("actual lookup")?;
        assert!(
            lookup.authored_key.is_some() && lookup.evidence.is_some(),
            "bound authored lookup {row}"
        );
    }
    let placeholder = text_field(&fields, 3, "text")?;
    assert!(
        cell(&out, placeholder)?.is_empty(),
        "placeholder has no text"
    );
    assert!(
        placeholder.complete && placeholder.lookup.is_none(),
        "the authored placeholder needs no language lookup"
    );
    Ok(())
}

fn language_fixture() -> TestResult<LocalizationSet> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let ini = GlobalIniBuilder::new()
        .text("present", "translated")
        .text("empty", "")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(Language::English.global_ini_path(), ini))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    Ok(LocalizationSet::read(&archive, &[Language::English])?)
}

fn text_field<'a, 'locale>(
    fields: &'a [StreamField<'locale>],
    row: u64,
    name: &str,
) -> TestResult<&'a StreamField<'locale>> {
    fields
        .iter()
        .find(|field| {
            matches!(
                &field.address,
                StreamAddress::Text {row: at, column, ..} if *at == row && *column == name
            )
        })
        .ok_or_else(|| format!("missing column {name} in row {row}").into())
}

fn cell<'a>(out: &'a [u8], field: &StreamField<'_>) -> TestResult<&'a [u8]> {
    let StreamAddress::Text { start, end, .. } = &field.address else {
        return Err("expected a text span".into());
    };
    let start = usize::try_from(*start)?;
    let end = usize::try_from(*end)?;
    out.get(start..end)
        .ok_or_else(|| io::Error::other("bad text span").into())
}

fn original_span(out: &[u8], proof: &StreamField<'_>, label: &str, expected: &[u8]) -> bool {
    proof.sites.iter().flatten().any(|site| {
        let SourceLocation::Subresource {
            namespace,
            field,
            indices,
        } = &site.location
        else {
            return false;
        };
        let [start, end] = indices.as_slice() else {
            return false;
        };
        namespace == "dataforgeDecodedBytes"
            && field == label
            && usize::try_from(*start)
                .ok()
                .zip(usize::try_from(*end).ok())
                .and_then(|(start, end)| out.get(start..end))
                == Some(expected)
    })
}
