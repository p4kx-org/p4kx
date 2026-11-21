// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Value as Raw};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::quality_v3_capture_fixture::{NAN_BITS, NAN_HEX};
use crate::quality_v3_capture_support as capture;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("CraftingQualityDistribution_Base"))
        .structure(
            TestStruct::new("CraftingQualityDistributionNormal")
                .parent("CraftingQualityDistribution_Base")
                .field("min", TestType::Int32)
                .field("max", TestType::Int32)
                .field("mean", TestType::Double)
                .field("twin", TestType::Double)
                .field("stddev", TestType::Double),
        )
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
        ))
        .record(values::distribution(
            values::DISTRIBUTION,
            "DuplicateFields",
            TestValue::Struct(
                TestInstance::new("CraftingQualityDistributionNormal")
                    .value("min", TestValue::Int32(0))
                    .value("max", TestValue::Int32(i32::MAX))
                    .value("mean", TestValue::Double(f64::from_bits(NAN_BITS)))
                    .value("twin", TestValue::Double(f64::from_bits(NAN_BITS)))
                    .value("stddev", TestValue::Double(1.0)),
            ),
        ))
}

#[test]
fn quality_v3_one_retained_cell_cannot_excuse_two_identical_original_named_float_slots()
-> TestResult {
    let (_folder, archive) = open()?;
    let input = DataForgeEntry::read(&archive)?;
    assert_original_slots(&input)?;
    let mut options = common::options();
    assert!(
        matches!(
            build_catalog_from(&archive, &input, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "unrepresented duplicate authored slots require diagnostic publication"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &input, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    assert_retained_and_uncovered(&catalog)
}

fn assert_original_slots(input: &DataForgeEntry) -> TestResult {
    let forge = input.open()?;
    let tree = forge.record(0).ok_or("original physical owner")?.tree()?;
    let Some(Raw::Struct(curve)) = tree.field("qualityDistribution") else {
        return Err("original inline curve".into());
    };
    let means: Vec<_> = curve
        .fields()
        .iter()
        .filter(|field| field.name() == "mean")
        .collect();
    assert_eq!(
        means.len(),
        2,
        "the owning canonical format read preserves both original declaration slots"
    );
    for mean in means {
        let Raw::Double(value) = mean.value() else {
            return Err("original Double mean slot".into());
        };
        assert_eq!(
            value.to_bits(),
            NAN_BITS,
            "both original slots have identical width and IEEE bits"
        );
    }
    Ok(())
}

fn assert_retained_and_uncovered(catalog: &Value) -> TestResult {
    let row = support::row(catalog, support::DISTRIBUTIONS, 0)?;
    let mean = support::at(
        row,
        "/qualityDistribution/value/payload/value/value/value/mean",
    )?;
    support::state(mean, "present")?;
    assert_eq!(
        support::at(mean, "/value")?,
        &json!({"type":"f64", "rawBits":NAN_HEX, "finite":null}),
        "the one retained named cell preserves its original IEEE bits"
    );
    capture::assert_projected(catalog, "qualityDistributions")?;
    capture::assert_uncovered(
        catalog,
        "qualityDistributions",
        "selected",
        0,
        "qualityDistribution.mean",
    )?;
    support::assert_field_error(
        catalog,
        "qualityDistribution.mean",
        "CraftingQualityDistributionNormal",
    )?;
    let diagnostics = support::at(catalog, "/exportHealth/diagnostics")?
        .as_array()
        .ok_or("diagnostics")?;
    let number = diagnostics
        .iter()
        .find(|item| {
            item.get("kind").and_then(Value::as_str) == Some("qualityDistributions")
                && item.get("code").and_then(Value::as_str) == Some("unsupportedValue")
                && item.get("stage").and_then(Value::as_str) == Some("projection")
                && item.get("sourceRole").and_then(Value::as_str) == Some("selected")
        })
        .ok_or("remaining original numeric token failure")?;
    assert_eq!(
        support::at(number, "/count")?,
        &json!(1),
        "one actual retained named cell covers one original token, and the other stays uncovered"
    );
    Ok(())
}

fn renamed_database() -> TestResult<Vec<u8>> {
    const ORIGINAL: &[u8] = b"twin\0";
    const REPLACEMENT: &[u8] = b"mean\0";
    let mut data = database().build();
    let positions: Vec<_> = data
        .windows(ORIGINAL.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == ORIGINAL).then_some(index))
        .collect();
    let [position] = positions.as_slice() else {
        return Err("one unique synthetic property-name string".into());
    };
    assert_eq!(
        ORIGINAL.len(),
        REPLACEMENT.len(),
        "the synthetic declaration rename preserves every table offset"
    );
    data.get_mut(*position..*position + ORIGINAL.len())
        .ok_or("synthetic name bytes")?
        .copy_from_slice(REPLACEMENT);
    Ok(data)
}

fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("duplicate-original-slots.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", renamed_database()?))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
