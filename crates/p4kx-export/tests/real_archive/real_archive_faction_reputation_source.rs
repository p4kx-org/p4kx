// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

const KIND: Kind = Kind::FactionReputations;
const TYPE: &str = "FactionReputation";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_faction_reputation_owner_fields_match_the_export() -> TestResult {
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
        .map_err(|_| "build faction reputation catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let (owners, links) = compare_owners(&forge, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during faction reputation verification")?;
    print_line(&format!(
        "faction reputation: {owners} physical owners and {links} stored link slots checked"
    ));
    Ok(())
}

fn compare_owners(forge: &DataForge<'_>, catalog: &Value) -> TestResult<(usize, usize)> {
    let rows = consumer::rows(catalog, KIND)?;
    assert!(
        !rows.is_empty(),
        "configured Build has faction reputation owners"
    );
    let mut positions = BTreeSet::new();
    let mut order = Vec::new();
    let mut trees = TreeBuilder::new();
    let mut links = 0;
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, KIND, "recordIndex")?
                .as_u64()
                .ok_or("physical faction reputation index")?,
        )?;
        let record = forge
            .record(index)
            .ok_or("original faction reputation Record")?;
        let guid = consumer::identity(row, KIND)?;
        assert!(
            positions.insert(index),
            "one definition per physical occurrence"
        );
        assert!(matches!(record.type_name(), Ok(TYPE)), "exact source type");
        assert!(
            guid == record.guid().to_string(),
            "exact original owner GUID"
        );
        order.push((guid, index));
        let tree = trees
            .build(&record)
            .map_err(|_| "read original faction reputation body")?;
        if let Some(Original::UInt64(geid)) = tree.field("GEID") {
            let decimal = geid.to_string();
            assert!(
                row.pointer("/geid/value").and_then(Value::as_str) == Some(decimal.as_str()),
                "exact original GEID"
            );
        }
        for name in ["allies", "enemies", "perkReputationRewardList"] {
            links += compare_links(row.get(name).ok_or("sourced link array")?, tree.field(name))?;
        }
    }
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal define owner order"
    );
    Ok((rows.len(), links))
}

fn compare_links(carrier: &Value, original: Option<&Original<'_>>) -> TestResult<usize> {
    let Some(Original::Array(slots)) = original else {
        require_nonpresent(carrier, original)?;
        return Ok(0);
    };
    let actual = consumer::source_value(carrier)?
        .and_then(Value::as_array)
        .ok_or("ordered physical source links")?;
    assert_eq!(
        actual.len(),
        slots.len(),
        "every original link slot is represented"
    );
    for (row, original) in actual.iter().zip(slots.iter()) {
        let (guid, status) = match original {
            Original::Reference(target) => (target.guid(), "present"),
            Original::Error(ValueError::DanglingReference { guid }) => (*guid, "dangling"),
            other => {
                require_nonpresent(row, Some(other))?;
                continue;
            }
        };
        let guid = guid.to_string();
        assert!(
            row.pointer("/value/guid/value").and_then(Value::as_str) == Some(guid.as_str())
                && row.pointer("/value/status").and_then(Value::as_str) == Some(status),
            "exact original GUID and resolution at each slot"
        );
    }
    Ok(slots.len())
}

fn require_nonpresent(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let state = carrier.get("state").and_then(Value::as_str);
    let valid = match original {
        None => state == Some("absent"),
        Some(Original::Null) => state == Some("null"),
        Some(Original::Error(_)) => state == Some("unreadable"),
        Some(_) => matches!(state, Some("invalid" | "unsupported")),
    };
    if !valid {
        return Err("original nonpresent faction reputation link state differs".into());
    }
    Ok(())
}
