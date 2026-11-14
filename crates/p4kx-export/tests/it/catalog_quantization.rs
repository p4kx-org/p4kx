// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Quality quantizations in the Basetool catalog, through the public API.

use crate::quantization_fixture::{
    ANOTHER, GOLD, IRON, NO_BANDS, ORE_GOLD, ORE_IRON, ORPHAN, REVERSED, SHARED_GOLD,
    TWO_PROPERTIES, write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use crate::quality_v3_support as source;
use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog, read_crafting};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;
type BandValues = Vec<(i64, i64, i64)>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn entries(catalog: &Value) -> Result<&[Value], Box<dyn Error>> {
    source::rows(catalog, source::QUANTIZATIONS)
}

fn entry(catalog: &Value, wanted: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(wanted);
    entries(catalog)?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(wanted.as_str()))
        .ok_or_else(|| "canonical quality definition".into())
}

fn inverse(catalog: &Value, wanted: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(wanted);
    source::rows(catalog, source::INVERSE)?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(wanted.as_str()))
        .ok_or_else(|| "canonical inverse relation".into())
}

fn class_names(catalog: &Value) -> Result<Vec<&str>, Box<dyn Error>> {
    entries(catalog)?
        .iter()
        .map(|row| {
            source::at(row, "/className/value")?
                .as_str()
                .ok_or_else(|| "sourced class name".into())
        })
        .collect()
}

fn members(catalog: &Value, wanted: u128) -> Result<Vec<&str>, Box<dyn Error>> {
    let row = inverse(catalog, wanted)?;
    let list = source::at(row, "/resourceTypeGuids")?;
    source::state(list, "present")?;
    source::at(list, "/value")?
        .as_array()
        .ok_or("inverse members")?
        .iter()
        .map(|member| {
            source::state(member, "present")?;
            source::at(member, "/value")?
                .as_str()
                .ok_or_else(|| "sourced resource GUID".into())
        })
        .collect()
}

fn band_values(row: &Value) -> Result<BandValues, Box<dyn Error>> {
    source::at(row, source::BANDS)?
        .as_array()
        .ok_or("raw band list")?
        .iter()
        .map(|band| {
            let value = |field| -> Result<i64, Box<dyn Error>> {
                let number = source::at(band, &format!("/value/{field}"))?;
                source::state(number, "present")?;
                assert_eq!(
                    source::at(number, "/value/type")?,
                    &json!("int32"),
                    "the declared source width remains explicit"
                );
                source::at(number, "/value/value")?
                    .as_i64()
                    .ok_or_else(|| "source band Int32".into())
            };
            Ok((value("start")?, value("end")?, value("mappedValue")?))
        })
        .collect()
}

#[test]
fn a_quantization_keeps_its_authored_bands_separate_from_the_resource_inverse() -> TestResult {
    let (_, catalog) = build(None)?;
    let gold = entry(&catalog, GOLD)?;
    assert_eq!(
        source::at(gold, "/className/value")?,
        &json!("Quantization_Gold")
    );
    assert_eq!(
        source::at(gold, "/recordPath/value")?,
        &json!("libs/foundry/records/crafting/qualityquantization/quantization_gold")
    );
    assert_eq!(
        band_values(gold)?,
        [(0, 399, 264), (400, 999, 553), (1000, 1000, 1000)]
    );
    assert_eq!(
        members(&catalog, GOLD)?,
        [guid(ORE_GOLD), guid(SHARED_GOLD), guid(TWO_PROPERTIES)],
        "the genuine inverse deduplicates repeated properties and orders resource GUIDs"
    );
    assert!(
        gold.get("resourceTypeGuids").is_none(),
        "D does not duplicate the inverse projection"
    );
    Ok(())
}

#[test]
fn the_entries_are_ordered_by_guid_and_a_leftover_is_counted_as_skipped() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog)?,
        [
            "Quantization_Gold",
            "Quantization_Iron",
            "Quantization_Orphan",
            "Quantization_NoBands",
            "Quantization_Reversed",
            "Quantization_Another"
        ]
    );
    assert_eq!(members(&catalog, IRON)?, [guid(ORE_IRON)]);
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityQuantizations)
            .map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((6, 1, true)),
        "raw rows include readable ineligible definitions independently of selection completeness"
    );
    Ok(())
}

#[test]
fn an_unused_eligible_quantization_is_kept_with_no_resource_types_and_only_counted() -> TestResult {
    let (built, catalog) = build(None)?;
    for unlinked in [ORPHAN, ANOTHER] {
        assert_eq!(members(&catalog, unlinked)?, [] as [&str; 0]);
        assert_eq!(band_values(entry(&catalog, unlinked)?)?.len(), 3);
        assert_eq!(
            source::at(inverse(&catalog, unlinked)?, "/eligibility/value/status")?,
            &json!("eligible")
        );
    }
    assert_eq!(built.summary().quantizations().unused, 2);
    assert!(
        built.summary().warnings().is_empty(),
        "unused eligible records remain plain relationship data"
    );
    Ok(())
}

#[test]
fn unresolved_inverse_links_and_leftover_resource_selection_keep_their_existing_scope() -> TestResult
{
    let (built, catalog) = build(None)?;
    assert_eq!(
        built.summary().quantizations().unresolved_links,
        3,
        "wrong-family, ineligible and selected-leftover targets remain unresolved; dangling References add no link"
    );
    assert_eq!(
        members(&catalog, IRON)?,
        [guid(ORE_IRON)],
        "a leftover resource neither links nor counts"
    );
    Ok(())
}

#[test]
fn a_malformed_quantization_is_retained_with_a_field_error_and_explicit_ineligibility() -> TestResult
{
    let (built, catalog) = build(None)?;
    assert!(
        band_values(entry(&catalog, NO_BANDS)?)?.is_empty(),
        "complete empty source bands remain present"
    );
    assert_eq!(
        source::at(inverse(&catalog, NO_BANDS)?, "/eligibility/value")?,
        &json!({"status":"notEligible", "reason":"emptyBands"})
    );
    let reversed = band_values(entry(&catalog, REVERSED)?)?;
    assert!(
        reversed.iter().any(|(start, end, _)| end < start),
        "configured invalid intervals are retained exactly"
    );
    source::assert_field_error(
        &catalog,
        "qualityQuantization.bands[1].end",
        "CraftingQualityQuantizationBand",
    )?;
    assert_eq!(
        source::at(inverse(&catalog, REVERSED)?, "/eligibility/value")?,
        &json!({"status":"notEligible", "reason":"invalidBands"})
    );
    assert!(
        built.summary().failures().is_empty(),
        "a retained owner's field failure is not a whole-record omission"
    );
    assert!(
        !built.summary().is_complete(),
        "the actual configured range violation still blocks normal publication"
    );
    Ok(())
}

#[test]
fn a_filter_selects_by_record_path_but_the_links_stay_complete() -> TestResult {
    let filter = Filter::new("*/quantization_gold");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(class_names(&catalog)?, ["Quantization_Gold"]);
    assert_eq!(
        members(&catalog, GOLD)?.len(),
        3,
        "required resource dependencies remain outside the output Filter"
    );
    assert_eq!(
        catalog["completeness"]["qualityQuantizations"],
        json!({"status":"partial", "filter":"*/quantization_gold"})
    );
    assert!(
        built.summary().warnings().is_empty(),
        "filtered unlinked records create no extra warning"
    );
    Ok(())
}

fn commodity_subtypes(catalog: &Value) -> Result<Vec<&str>, Box<dyn Error>> {
    let rows = catalog
        .pointer("/projections/commodityPresentation/rows")
        .and_then(Value::as_array)
        .ok_or("canonical Commodity presentations")?;
    let mut subtypes = Vec::new();
    for row in rows {
        let carrier = row.get("subtypeGuid").ok_or("canonical subtype carrier")?;
        source::state(carrier, "present")?;
        match source::at(carrier, "/value")? {
            Value::Null => {}
            Value::String(guid) => subtypes.push(guid.as_str()),
            _ => return Err("subtype is a GUID string or explicit null".into()),
        }
    }
    Ok(subtypes)
}

#[test]
fn every_resource_type_a_quantization_links_to_resolves_to_a_commodity() -> TestResult {
    let (_, catalog) = build(None)?;
    let subtypes = commodity_subtypes(&catalog)?;
    for row in entries(&catalog)? {
        let owner = source::at(row, "/guid/value")?
            .as_str()
            .ok_or("quality GUID")?;
        for member in source::rows(&catalog, source::INVERSE)?
            .iter()
            .filter(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(owner))
        {
            for linked in source::at(member, "/resourceTypeGuids/value")?
                .as_array()
                .ok_or("inverse list")?
            {
                let linked = source::at(linked, "/value")?
                    .as_str()
                    .ok_or("resource GUID")?;
                assert!(subtypes.contains(&linked), "{linked} has no commodity");
            }
        }
    }
    let mut expected = vec![
        guid(ORE_GOLD),
        guid(SHARED_GOLD),
        guid(TWO_PROPERTIES),
        guid(ORE_IRON),
    ];
    let mut found: Vec<String> = subtypes.iter().map(|text| (*text).to_owned()).collect();
    expected.sort();
    found.sort();
    assert_eq!(
        found, expected,
        "only eligible referenced resources are backfilled"
    );
    Ok(())
}

#[test]
fn a_filter_lists_a_backfilled_commodity_only_when_its_record_path_matches() -> TestResult {
    let (_, catalog) = build(Some(&Filter::new("*/quantization_gold")))?;
    assert_eq!(commodity_subtypes(&catalog)?, [] as [&str; 0]);
    Ok(())
}

#[test]
fn a_filter_matching_both_paths_lists_the_raw_quantizations_and_backfilled_commodities()
-> TestResult {
    let (_, catalog) = build(Some(&Filter::new("libs/foundry/records/*")))?;
    assert_eq!(class_names(&catalog)?.len(), 6);
    let mut found = commodity_subtypes(&catalog)?;
    found.sort_unstable();
    let mut expected = [ORE_GOLD, SHARED_GOLD, TWO_PROPERTIES, ORE_IRON].map(guid);
    expected.sort_unstable();
    assert_eq!(found, expected);
    Ok(())
}

#[test]
fn the_library_reads_the_quantizations_with_their_links() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let crafting = read_crafting(&forge, None, None)?;
    let names: Vec<(&str, usize, usize)> = crafting
        .quality_quantizations
        .iter()
        .map(|quantization| {
            (
                quantization.class_name,
                quantization.bands.len(),
                quantization.resource_types.len(),
            )
        })
        .collect();
    assert_eq!(
        names.len(),
        5,
        "the leftover is read, the malformed are not"
    );
    assert!(names.contains(&("Quantization_Gold", 3, 3)));
    assert_eq!(crafting.failures.len(), 2);
    Ok(())
}
