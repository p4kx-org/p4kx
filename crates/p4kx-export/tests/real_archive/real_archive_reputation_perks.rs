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

const LIST_TYPE: &str = "SPerkReputationListParams";
const KIND: Kind = Kind::ReputationPerks;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_perk_list_occurrences_match_catalog_rows() -> TestResult {
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
        .map_err(|_| "build reputation perks catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let (lists, entries) = check_lists(&forge, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during perk verification")?;
    print_line(&format!(
        "reputation perks: {lists} physical lists and {entries} owned slots checked"
    ));
    Ok(())
}

fn check_lists(forge: &DataForge<'_>, catalog: &Value) -> TestResult<(usize, usize)> {
    let rows = consumer::rows(catalog, KIND)?;
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| matches!(record.type_name(), Ok(LIST_TYPE)))
        .map(|record| record.index())
        .collect();
    assert!(
        !expected.is_empty(),
        "configured Build contains reputation perk lists"
    );
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut trees = TreeBuilder::new();
    let mut entries = 0;
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, KIND, "recordIndex")?
                .as_u64()
                .ok_or("physical perk-list index")?,
        )?;
        let source = forge.record(index).ok_or("original perk-list record")?;
        let guid = consumer::identity(row, KIND)?;
        assert!(seen.insert(index), "one row per physical perk list");
        assert!(
            matches!(source.type_name(), Ok(LIST_TYPE)),
            "exact source record type"
        );
        assert!(guid == source.guid().to_string(), "exact source list GUID");
        order.push((guid, index));
        let tree = trees
            .build(&source)
            .map_err(|_| "read original perk-list body")?;
        entries += check_entries(
            row.get("perks").ok_or("sourced perk slots")?,
            tree.field("perks"),
        )?;
    }
    assert_eq!(seen, expected, "all physical perk-list records emitted");
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal define perk-list order"
    );
    Ok((rows.len(), entries))
}

fn check_entries(carrier: &Value, original: Option<&Original<'_>>) -> TestResult<usize> {
    let projected = consumer::source_value(carrier)?;
    let Some(Original::Array(entries)) = original else {
        check_nonpresent(carrier, original)?;
        return Ok(0);
    };
    let rows = projected
        .and_then(Value::as_array)
        .ok_or("ordered perk slots")?;
    assert_eq!(rows.len(), entries.len(), "one carrier per original entry");
    for (row, original) in rows.iter().zip(entries.iter()) {
        let Original::Struct(entry) = original else {
            check_nonpresent(row, Some(original))?;
            continue;
        };
        let row = consumer::source_value(row)?.ok_or("original perk entry")?;
        for name in ["scope", "standing"] {
            check_reference(
                row.get(name).ok_or("sourced perk binding")?,
                entry.field(name),
            )?;
        }
        let carrier = row.get("perk").ok_or("sourced concrete perk")?;
        let Some(Original::Struct(perk)) = entry.field("perk") else {
            check_nonpresent(carrier, entry.field("perk"))?;
            continue;
        };
        let body = consumer::source_value(carrier)?.ok_or("original perk body")?;
        assert!(
            body.pointer("/sourceType/value").and_then(Value::as_str) == Some(perk.type_name()),
            "concrete perk source type"
        );
        if let Some(Original::Single(original)) = perk.field("discountPercentage") {
            let bits = format!("{:08x}", original.to_bits());
            assert!(
                body.pointer("/details/discountPercentage/value/rawBits")
                    .and_then(Value::as_str)
                    == Some(bits.as_str()),
                "exact original discount word"
            );
        }
    }
    Ok(entries.len())
}

fn check_reference(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let (guid, status) = match original {
        Some(Original::Reference(target)) => (target.guid(), "present"),
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        _ => return check_nonpresent(carrier, original),
    };
    let guid = guid.to_string();
    assert!(
        carrier.pointer("/value/guid/value").and_then(Value::as_str) == Some(guid.as_str())
            && carrier.pointer("/value/status").and_then(Value::as_str) == Some(status),
        "exact original perk association GUID and resolution"
    );
    Ok(())
}

fn check_nonpresent(carrier: &Value, original: Option<&Original<'_>>) -> TestResult {
    let state = carrier.get("state").and_then(Value::as_str);
    let accepted = match original {
        None => state == Some("absent"),
        Some(Original::Null) => state == Some("null"),
        Some(Original::Error(_)) => state == Some("unreadable"),
        Some(_) => matches!(state, Some("invalid" | "unsupported")),
    };
    if !accepted {
        return Err("original nonpresent perk state differs".into());
    }
    Ok(())
}
