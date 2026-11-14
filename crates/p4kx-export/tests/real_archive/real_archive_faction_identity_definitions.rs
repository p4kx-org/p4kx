// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SOURCE_TYPE: &str = "FactionIdentity";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn faction_identity_definitions_match_authored_records() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let filter = std::env::var("P4KX_FACTION_IDENTITY_FILTER")
        .map_err(|_| "configured identity Record filter")?;
    let filter = Filter::parse(&filter)?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected faction identities")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    verify(&forge, &catalog, &watch, &filter)?;
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    Ok(())
}

fn verify(
    forge: &DataForge<'_>,
    catalog: &Value,
    watch: &ArchiveWatch,
    filter: &Filter,
) -> TestResult {
    let mut expected = BTreeSet::new();
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name()? == SOURCE_TYPE && filter.matches(record.record_path()?) {
            expected.insert((record.guid().to_string(), record.index()));
        }
    }
    let rows = consumer::rows(catalog, Kind::FactionIdentities)?;
    assert!(
        !expected.is_empty(),
        "the Record filter must select original identities"
    );
    assert_eq!(
        rows.len(),
        expected.len(),
        "every physical owner remains exported"
    );
    let mut builder = TreeBuilder::new();
    let mut populated = 0;
    for (row, (guid, index)) in rows.iter().zip(expected) {
        let record = forge.record(index).ok_or("original identity Record")?;
        let tree = builder
            .build(&record)
            .map_err(|_| "read original identity")?;
        let actual_guid = consumer::identity(row, Kind::FactionIdentities)?;
        assert!(
            actual_guid == guid
                && row.pointer("/recordIndex/value").and_then(Value::as_u64)
                    == Some(u64::from(index)),
            "exact owner GUID and original physical position"
        );
        populated += usize::from(compare(&tree, row, forge, catalog)?);
    }
    if std::env::var_os("P4KX_EXPECT_FACTION_IDENTITY_WEIGHTS").is_some() {
        assert!(
            populated > 0,
            "at least one ordered encounter preset weight is verified"
        );
    }
    print_line(&format!(
        "verified {} physical faction identities, including {populated} with encounter weights",
        rows.len()
    ));
    Ok(())
}

fn compare(
    tree: &RecordTree<'_>,
    row: &Value,
    forge: &DataForge<'_>,
    catalog: &Value,
) -> TestResult<bool> {
    for name in ["name", "description"] {
        verify_locale(tree.field(name), field(row, name)?, catalog)?;
    }
    for (name, target) in [
        ("factionTag", "Tag"),
        ("factionRef", "Faction"),
        ("securityNetworkRef", "SecurityNetworkManifest"),
    ] {
        verify_link(forge, target, tree.field(name), field(row, name)?)?;
    }
    verify_array(
        tree.field("lootPoolThemeTags"),
        field(row, "lootPoolThemeTags")?,
        |source, row| verify_link(forge, "Tag", source, row),
    )?;
    let source = tree.field("encounterPresetWeights");
    verify_array(source, field(row, "encounterPresetWeights")?, verify_weight)?;
    Ok(matches!(source, Some(Original::Array(weights)) if !weights.is_empty()))
}

fn verify_locale(source: Option<&Original<'_>>, row: &Value, catalog: &Value) -> TestResult {
    let Some(Original::Locale(expected)) = source else {
        return verify_state(source, row);
    };
    let reference = consumer::present(row)?;
    let text = catalog
        .pointer("/texts/gameTexts")
        .and_then(Value::as_array)
        .and_then(|texts| {
            texts
                .iter()
                .find(|text| text.pointer("/id/value") == Some(reference))
        })
        .ok_or("protected original Locale reference")?;
    assert!(
        text.pointer("/key/value").and_then(Value::as_str) == Some(*expected),
        "the protected reference keeps the actual authored localization key"
    );
    Ok(())
}

fn verify_weight(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    assert!(
        consumer::present(field(row, "sourceType")?)?.as_str() == Some(source.type_name()),
        "exact original weight source type"
    );
    let preset = field(row, "encounterPreset")?;
    if let Some(Original::Enum(expected)) = source.field("encounterPreset") {
        assert!(
            consumer::present(preset)?.as_str() == Some(*expected),
            "exact original enum token"
        );
    } else {
        verify_state(source.field("encounterPreset"), preset)?;
    }
    let weight = field(row, "weight")?;
    if let Some(Original::Single(expected)) = source.field("weight") {
        assert!(
            consumer::present(weight)?
                .get("rawBits")
                .and_then(Value::as_str)
                == Some(&format!("{:08x}", expected.to_bits())),
            "exact original Single bits"
        );
    } else {
        verify_state(source.field("weight"), weight)?;
    }
    Ok(())
}
