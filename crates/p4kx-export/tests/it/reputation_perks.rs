// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{
    self as archive_fixture, DcbOrigin, TestResult, at, guid, text_row,
};

#[path = "reputation_perk_fixture.rs"]
mod fixture;

const ROWS: &str = "/definitions/reputationPerks/rows";
const LIST: &str = "/definitions/reputationPerks/rows/0";

fn export(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = archive_fixture::options();
    options.filter = filter;
    let built = build_catalog(archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}

#[test]
fn perk_list_keeps_owned_slots_shared_record_links_and_exact_payload_types() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::complete(), DcbOrigin::Plain)?;
    let catalog = export(&archive, None)?;
    let list = at(&catalog, LIST)?;
    assert_eq!(
        at(list, "/guid/value")?,
        &json!(guid(fixture::LIST)),
        "GUID sorting retains the list identity"
    );
    let entries = at(list, "/perks/value")?
        .as_array()
        .ok_or("ordered perk entries")?;
    let [item, discount, enabled, authored, missing] = entries.as_slice() else {
        return Err("five actual perk source slots".into());
    };
    assert_eq!(
        at(item, "/value/standing/value/guid/value")?,
        &json!(guid(fixture::STANDING)),
        "original standing target"
    );
    assert_eq!(
        at(discount, "/value/standing/value/guid/value")?,
        &json!(guid(fixture::STANDING)),
        "another perk on the same standing"
    );
    assert_eq!(
        at(item, "/value/scope/value/guid/value")?,
        &json!(guid(fixture::SCOPE)),
        "original scope target"
    );
    assert_eq!(
        at(
            item,
            "/value/perk/value/details/rewardItem/value/guid/value"
        )?,
        &json!(guid(fixture::ITEM)),
        "real item link"
    );
    assert_category(discount, enabled)?;
    assert_authored(authored)?;
    assert_eq!(
        at(missing, "/value/perk/state")?,
        "null",
        "explicit null perk pointer"
    );
    assert_eq!(
        at(&catalog, "/definitions/reputationPerks/rows/1/guid/value")?,
        &json!(guid(fixture::EMPTY)),
        "path spelling cannot exclude the other physical list"
    );
    assert_eq!(
        at(&catalog, "/definitions/reputationPerks/rows/1/perks/value")?,
        &json!([]),
        "present empty source list"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "complete",
        "a readable unclassified subtype remains an original payload"
    );
    Ok(())
}

fn assert_schema(catalog: &Value) -> TestResult {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../schemas/basetool-catalog.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;
    validator
        .validate(catalog)
        .map_err(|error| format!("source-backed perk rows meet schema v3: {error}"))?;
    Ok(())
}

fn assert_category(discount: &Value, enabled: &Value) -> TestResult {
    let detail = at(discount, "/value/perk/value/details")?;
    let marker = at(detail, "/discountPercentage/containsEncryptedSource")?;
    assert_eq!(
        marker, false,
        "the discount word retains its plain physical source"
    );
    assert_eq!(
        at(detail, "/type")?,
        "discountItemCategory",
        "the exact concrete kind selects the discount facts"
    );
    assert_eq!(
        at(detail, "/discountPercentage/value/rawBits")?,
        &json!(format!("{:08x}", fixture::DISCOUNT_BITS)),
        "original Single bits"
    );
    assert_eq!(
        at(detail, "/discountPercentage/value/finite")?,
        &json!(12.5),
        "the stored numeric value stays in its source unit"
    );
    assert_eq!(
        at(detail, "/category/id/value")?,
        &json!(guid(fixture::CATEGORY_ID)),
        "the category's GUID remains a scalar"
    );
    assert_eq!(
        at(detail, "/category/franchise/value/guid/value")?,
        &json!(guid(fixture::FRANCHISE)),
        "franchise identity"
    );
    assert_eq!(
        at(detail, "/category/type/value")?,
        "FixtureCategory",
        "stored EItemType label"
    );
    assert_eq!(
        at(detail, "/category/subType/value")?,
        "FixtureUnknownSubtype",
        "stored EItemSubType label"
    );
    assert_eq!(
        at(detail, "/category/requiredTag/value/status")?,
        "dangling",
        "the absent tag target is explicit"
    );
    assert_enabled(enabled)
}

fn assert_enabled(enabled: &Value) -> TestResult {
    assert_eq!(
        at(enabled, "/value/perk/value/details/type")?,
        "enabledItemCategory",
        "enabled category owns no discount field"
    );
    assert_eq!(
        at(enabled, "/value/standing/value/guid/value")?,
        &json!(guid(fixture::MISSING)),
        "dangling standing GUID is retained"
    );
    assert_eq!(
        at(enabled, "/value/standing/value/status")?,
        "dangling",
        "dangling standing state"
    );
    Ok(())
}

fn assert_authored(entry: &Value) -> TestResult {
    let body = at(entry, "/value/perk/value")?;
    assert_eq!(
        at(body, "/sourceType/value")?,
        fixture::AUTHORED,
        "actual source subtype survives"
    );
    assert_eq!(
        at(body, "/details/type")?,
        "authored",
        "unknown concrete descendants keep original fields"
    );
    let values = at(body, "/authoredFields/values")?
        .as_array()
        .ok_or("original retained fields")?;
    let [number] = values.as_slice() else {
        return Err("unknown perk keeps exactly its unprojected field".into());
    };
    assert_eq!(
        at(number, "/value/value/value/rawBits")?,
        &json!(format!("{:08x}", fixture::RAW_BITS)),
        "unknown raw nonfinite token"
    );
    Ok(())
}

fn assert_retained_variant(catalog: &Value) -> TestResult {
    let diagnostic = crate::health_origin_assertions::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| {
            row["kind"] == "reputationPerks"
                && row["stage"] == "projection"
                && row["code"] == "unsupportedValue"
                && row["severity"] == "information"
        })
        .ok_or("sourced note for a retained concrete perk")?;
    assert_eq!(diagnostic["count"], 1, "one unclassified original variant");
    let example = at(diagnostic, "/examples/0")?;
    let proof = crate::health_origin_assertions::present_text;
    proof(
        catalog,
        example,
        "field",
        &json!("perks[3].perk"),
        Some(true),
    )?;
    proof(
        catalog,
        example,
        "sourceType",
        &json!(fixture::AUTHORED),
        Some(true),
    )?;
    Ok(())
}

#[test]
fn perk_text_storage_and_filtered_shared_perk_list_preserve_their_sources() -> TestResult {
    let (_folder, archive) = archive_fixture::open(&fixture::complete(), DcbOrigin::Encrypted)?;
    let catalog = export(&archive, None)?;
    let first = at(&catalog, &format!("{LIST}/perks/value/0/value/perk/value"))?;
    assert_eq!(
        at(first, "/description/value")?,
        fixture::LITERAL,
        "the stored String stays literal"
    );
    assert_eq!(
        at(first, "/icon/value")?,
        fixture::ICON,
        "the icon remains a path"
    );
    assert_eq!(
        at(first, "/description/containsEncryptedSource")?,
        true,
        "literal description's immediate source marker"
    );
    for (field, key) in [
        ("displayName", "@shared"),
        ("displayDescription", "@own_description"),
    ] {
        let id = at(first, &format!("/{field}/value"))?;
        let text = text_row(&catalog, "gameTexts", id)?;
        assert_eq!(
            at(text, "/key/value")?,
            key,
            "the authored Locale keeps its key"
        );
    }
    assert_retained_variant(&catalog)?;
    assert_schema(&catalog)?;
    crate::commodity_v3_source_inventory::reference_closure(&catalog)?;
    assert_filtered_list(&archive)
}

fn assert_filtered_list(archive: &Archive) -> TestResult {
    let filter = Filter::new("factionreputation");
    let filtered = export(archive, Some(&filter))?;
    assert_eq!(
        at(&filtered, ROWS)?,
        &json!([]),
        "selection can omit the shared physical list"
    );
    assert_eq!(
        at(&filtered, "/definitions/reputationPerks/availability")?,
        "available",
        "source declaration survives filtering"
    );
    assert_eq!(
        at(&filtered, "/completeness/reputationPerks/status")?,
        "partial",
        "selection status"
    );
    let links = at(&filtered, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved original links")?;
    let filtered_guid = json!(guid(fixture::LIST));
    assert_eq!(
        links
            .iter()
            .filter(|link| {
                link["status"] == "excludedByFilter"
                    && link["kind"] == "factionReputations"
                    && link["sourceRole"] == "selected"
                    && link.pointer("/targetGuid/value") == Some(&filtered_guid)
            })
            .count(),
        2,
        "both owning factions retain original references to the same filtered list"
    );
    Ok(())
}
