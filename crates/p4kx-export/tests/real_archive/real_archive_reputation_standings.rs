// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, TreeBuilder, Value as Original};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_standing_drift_words_match_the_physical_source_rows() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build reputation standing catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let rows = crate::catalog_v3_consumer::rows(&catalog, Kind::ReputationStandings)?;
    let mut positions = BTreeSet::new();
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut trees = TreeBuilder::new();
    let mut source_singles = 0;
    for row in rows {
        let position = row
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("physical standing Record index")?;
        if !positions.insert(position) {
            return Err("repeated physical standing Record index".into());
        }
        let record = forge.record(position).ok_or("original standing Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original standing Record")?;
        let guid = tree.guid().to_string();
        if tree.type_name() != "SReputationStandingParams"
            || row.pointer("/guid/value").and_then(Value::as_str) != Some(guid.as_str())
        {
            return Err("original standing identity differs".into());
        }
        let drift = row.get("driftTimeHours").ok_or("sourced standing drift")?;
        source_singles += usize::from(check_drift(drift, tree.field("driftTimeHours"))?);
    }
    if source_singles == 0 {
        return Err("configured Build contains no traced standing drift Single".into());
    }
    watch
        .check()
        .map_err(|_| "Archive changed during standing verification")?;
    crate::real_archive_verification::print_line(&format!(
        "reputation standings: {} physical rows and {source_singles} original drift words checked",
        positions.len(),
    ));
    Ok(())
}

fn check_drift(drift: &Value, original: Option<&Original<'_>>) -> TestResult<bool> {
    match (original, drift.get("state").and_then(Value::as_str)) {
        (Some(Original::Single(original)), state) => {
            if state != Some("present") {
                return Err("original standing drift Single is not present".into());
            }
            check_single(drift, *original)?;
            Ok(true)
        }
        (None, Some("absent"))
        | (Some(Original::Null), Some("null"))
        | (Some(Original::Error(_)), Some("unreadable"))
        | (Some(_), Some("invalid" | "unsupported")) => Ok(false),
        _ => Err("original standing drift state differs".into()),
    }
}

fn check_single(drift: &Value, original: f32) -> TestResult {
    let bits = format!("{:08x}", original.to_bits());
    if drift.pointer("/value/rawBits").and_then(Value::as_str) != Some(&bits) {
        return Err("original standing drift word differs".into());
    }
    let finite = drift
        .pointer("/value/finite")
        .ok_or("finite drift projection")?;
    if original.is_finite() {
        let value = finite.to_string().parse::<f32>()?;
        if value.to_bits() != original.to_bits() {
            return Err("finite standing drift hours differ".into());
        }
    } else if !finite.is_null() {
        return Err("nonfinite standing drift has a decimal projection".into());
    }
    Ok(())
}
