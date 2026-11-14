// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, Filter, Guid, Language, LocalizationSet,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_item_durability as proof;
use crate::real_archive_shop_inventory_v3;
use crate::real_archive_verification::print_line;

#[path = "mission_item_token_original.rs"]
mod original;

use self::original::{Case, Expected};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn source_selected_mission_item_tokens_match_static_preview_and_runtime_placeholders() -> TestResult
{
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let locale = LocalizationSet::read(&archive, &[Language::English])
        .map_err(|_| "read original English localization")?;
    let english = locale
        .entry(Language::English)
        .ok_or("original English localization")?
        .parse()
        .map_err(|_| "parse original English localization")?;
    let cases = original::select(&forge, &watch, &english)?;
    let singleton = cases
        .singleton
        .ok_or("an original singleton mission item title preview")?;
    let runtime = cases
        .runtime
        .ok_or("an original tag-selected mission item description")?;
    for case in [&singleton, &runtime] {
        let catalog = build(&archive, &entry, case.generator_path)?;
        verify(&catalog, case)?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    print_line(
        "Mission item tokens: one original singleton preview and one tag-selected runtime placeholder compared",
    );
    Ok(())
}

fn build(archive: &Archive, entry: &DataForgeEntry, path: &str) -> TestResult<Value> {
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build selected Generator catalog")?;
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn verify(catalog: &Value, case: &Case<'_>) -> TestResult {
    let rows = proof::rows(catalog, "/projections/missionTitles/rows")?;
    let mut rows = rows.iter().filter(|row| same_contract(row, &case.contract));
    let row = rows
        .next()
        .ok_or("the exact original contract has a title row")?;
    assert!(
        rows.next().is_none(),
        "the same physical contract appears once"
    );
    let field = row
        .get(case.field)
        .ok_or("projected original mission text")?;
    let display = proof::text(field, "/translations/en/value")?;
    let placeholder = format!("[{}]", case.token.to_ascii_uppercase());
    match case.expected {
        Expected::StaticPreview {
            item,
            display: expected,
        } => {
            assert!(
                item != Guid::default()
                    && display.contains(expected)
                    && !display.contains(&placeholder),
                "singleton preview includes the original localized item and resolves its own placeholder"
            );
        }
        Expected::Runtime => {
            assert!(
                case.variant.is_some() && display.contains(&placeholder),
                "tag-driven runtime selection retains the unresolved token"
            );
        }
    }
    verify_audit(field, case)?;
    let texts = TextIndex::new(catalog)?;
    let authored = texts
        .reference(
            field.get("key").ok_or("original mission key carrier")?,
            TextDomain::GameText,
            TextReferenceShape::Required,
        )?
        .ok_or("original mission key in protected text table")?;
    assert!(
        authored
            .source()
            .pointer("/key/value")
            .and_then(Value::as_str)
            == Some(case.key),
        "the rendered projection retains its independently read source Locale key"
    );
    Ok(())
}

fn verify_audit(field: &Value, case: &Case<'_>) -> TestResult {
    let rows = proof::rows(field, "/itemTokens/en")?;
    let audit = rows
        .iter()
        .find(|row| {
            row.pointer("/name/value").and_then(Value::as_str) == Some(case.token)
                && row.pointer("/variant/value").and_then(Value::as_str) == case.variant
        })
        .ok_or("the actual token and variant have one structured audit row")?;
    for name in ["name", "variant", "outcome"] {
        let cell = audit.get(name).ok_or("source-backed token datum")?;
        assert!(
            cell.get("containsEncryptedSource").is_some()
                && cell
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|uses| !uses.is_empty()),
            "every token datum has contributing provenance"
        );
    }
    let outcome = audit.pointer("/outcome/value").ok_or("token outcome")?;
    match case.expected {
        Expected::StaticPreview { item, .. } => assert!(
            outcome.get("type").and_then(Value::as_str) == Some("staticPreview")
                && outcome.get("itemGuid").and_then(Value::as_str)
                    == Some(item.to_string().as_str())
                && outcome
                    .get("itemRecordIndex")
                    .and_then(Value::as_u64)
                    .is_some(),
            "static outcome retains the physical item identity"
        ),
        Expected::Runtime => assert!(
            outcome.get("type").and_then(Value::as_str) == Some("unresolved")
                && outcome.get("reason").and_then(Value::as_str) == Some("runtimeQuery"),
            "runtime outcome explains the preserved token"
        ),
    }
    Ok(())
}

fn same_contract(row: &Value, expected: &Value) -> bool {
    let Some(actual) = row.get("contract") else {
        return false;
    };
    ["recordIndex", "handler", "contract", "subContract"]
        .into_iter()
        .all(|field| actual.get(field) == expected.get(field))
}
