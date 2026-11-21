// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Filter, SourceValueState};
use p4kx_testkit::real_archive_path;
use serde::Serialize;
use serde_json::Value;

use super::super::{planet_climate_asset, planet_geology_asset};
use super::TestResult;
use crate::CatalogOptions;
use crate::build::original_input::test_support::with_archive_input;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_json::LooseJson;
use crate::catalog_read::original::EntryCapture;
use crate::entry_source::EntrySource;
use crate::export_ledger::{ExportLedger, SourceDisposition};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_planet_asset_projection_pairs_original_and_output() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let manifest = archive.build().ok_or("installed Build receipt")?;
    assert_eq!(manifest.change_number(), "13189690", "exact PTU changelist");
    let paths = std::env::var_os("P4KX_PLANET_ASSET_PROJECTION_PATHS")
        .ok_or("two exact private Entry selectors")?;
    let text = std::fs::read_to_string(paths)?;
    let mut selected = text.lines();
    let climate_path = selected.next().ok_or("exact climate Entry selector")?;
    let geology_path = selected.next().ok_or("exact geology Entry selector")?;
    assert!(
        selected.next().is_none(),
        "two exact private Entry selectors"
    );
    assert!(
        planet_climate_asset::FAMILY.includes(climate_path),
        "climate scope"
    );
    assert!(
        planet_geology_asset::FAMILY.includes(geology_path),
        "geology scope"
    );
    with_archive_input(&archive, |forge, original| {
        let capture = EntryCapture::for_input(original);
        let mut ledger = ExportLedger::default();
        let climate_filter = Filter::parse(climate_path)?;
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        options.filter = Some(&climate_filter);
        let climate =
            planet_climate_asset::read(&archive, original, &capture, &mut ledger, &options)?;
        let geology_filter = Filter::parse(geology_path)?;
        options.filter = Some(&geology_filter);
        let geology =
            planet_geology_asset::read(&archive, forge, original, &capture, &mut ledger, &options)?;
        assert!(
            ledger
                .entries
                .iter()
                .all(|entry| { entry.disposition == SourceDisposition::Projected }),
            "every selected Entry projected"
        );
        assert_eq!(
            ledger.entries.len(),
            2,
            "exactly two definition source reads"
        );
        assert_climate(&archive, &capture, climate_path, &climate.rows)?;
        assert_geology(&archive, &capture, geology_path, &geology.rows)
    })
}

fn check_pair<T: Serialize>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    path: &str,
    rows: &[T],
    check: impl FnOnce(&EntryJsonObject<'_, '_>, &Value) -> TestResult,
) -> TestResult {
    let [row] = rows else {
        return Err("exactly one projected definition".into());
    };
    let wanted = crate::starmap_index::key(path);
    let entry = archive
        .entries()
        .iter()
        .find(|entry| crate::starmap_index::key(entry.path()) == wanted)
        .ok_or("original selected Entry")?;
    let source = EntrySource::outer(capture, archive, entry)?;
    let read =
        LooseJson::read(archive, entry, &source, None)?.map_err(|_| "original Entry CRC read")?;
    let parsed = read
        .parse(capture, None)?
        .map_err(|_| "original JSON tokens")?;
    let input = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())?
        .ok_or("source object")?;
    let output = serde_json::to_value(row)?;
    same_cell(
        &input.field("GUID", entry_json_object::guid)?,
        &output,
        "/guid",
    )?;
    check(&input, &output)
}

fn assert_climate<T: Serialize>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    path: &str,
    rows: &[T],
) -> TestResult {
    check_pair(archive, capture, path, rows, |input, output| {
        let raw = input.field("minAnnualRainfall", entry_json_object::number)?;
        same_cell(&raw, output, "/envelope/minAnnualRainfall")?;
        let raw = input.field("shadingClassCloseGUID", entry_json_object::guid)?;
        same_cell(&raw, output, "/appearance/shadingClassClose")?;
        let raw = input.field("subAssets", entry_json_object::array_len)?;
        let count = output
            .pointer("/subAssets/value")
            .and_then(Value::as_array)
            .map(Vec::len);
        let expected = raw.value().and_then(|n| usize::try_from(n.get()).ok());
        assert!(
            count.is_some() && count == expected,
            "authored sub-asset cardinality"
        );
        let setups = input.object_array("subAssets", |asset| {
            asset.field("harvestableSetupRecord", entry_json_object::guid)
        })?;
        let mut exact_candidate = false;
        for (position, source) in setups.value().into_iter().flatten().enumerate() {
            let Some(source) = source.value() else {
                continue;
            };
            let prefix = format!("/subAssets/value/{position}/value");
            same_cell(source, output, &format!("{prefix}/harvestableSetupRecord"))?;
            if source.state() == SourceValueState::Present {
                let link = output
                    .pointer(&format!("{prefix}/harvestableSetup"))
                    .ok_or("setup link")?;
                exact_candidate |= link.pointer("/state/value").and_then(Value::as_str)
                    == Some("filtered")
                    && link
                        .pointer("/candidates/0/sourceType/value")
                        .and_then(Value::as_str)
                        == Some("HarvestableSetup")
                    && link.pointer("/candidates/0/guid/value")
                        == output.pointer(&format!("{prefix}/harvestableSetupRecord/value"));
            }
        }
        assert!(
            exact_candidate,
            "exact physical setup candidate remains visible under filtering"
        );
        Ok(())
    })
}

fn assert_geology<T: Serialize>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    path: &str,
    rows: &[T],
) -> TestResult {
    check_pair(archive, capture, path, rows, |input, output| {
        let raw = input.field("Mineable_cfg_path", entry_json_object::text)?;
        assert!(
            raw.value().is_some_and(|path| !path.is_empty()),
            "known mineable sample"
        );
        same_cell(&raw, output, "/mineableConfigPath")?;
        let raw = input.field("Mineable_Material_path", entry_json_object::text)?;
        same_cell(&raw, output, "/mineableMaterialPath")?;
        for link in [
            "/references/mineableConfig/state/value",
            "/references/mineableMaterial/state/value",
        ] {
            assert!(
                output.pointer(link).and_then(Value::as_str) == Some("resolved"),
                "authored path resolves in the original directory"
            );
        }
        Ok(())
    })
}

fn same_cell(original: &impl Serialize, output: &Value, pointer: &str) -> TestResult {
    let authored = serde_json::to_value(original)?;
    let exported = output.pointer(pointer).ok_or("exported field")?;
    assert!(
        [
            "state",
            "value",
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete"
        ]
        .iter()
        .all(|member| exported.get(*member).is_some()
            && exported.get(*member) == authored.get(*member)),
        "original value and immediate source evidence at {pointer}",
    );
    Ok(())
}
