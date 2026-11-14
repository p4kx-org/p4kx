// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_export::Kind;
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};

fn commodity_ids(catalog: &Value) -> TestResult<HashSet<String>> {
    Ok(
        crate::real_archive_verification::guids(catalog, "commodities")?
            .into_iter()
            .map(str::to_owned)
            .collect(),
    )
}

fn assert_physical_ordinals(catalog: &Value) -> TestResult {
    for (guid, index) in [
        (fixture::ALPHA, fixture::ALPHA_INDEX),
        (fixture::BETA, fixture::BETA_INDEX),
    ] {
        let row = fixture::definition(catalog, guid)?;
        assert_eq!(
            fixture::payload(row, "recordIndex")?,
            &json!(index),
            "the original physical Record ordinal retains its numeric source value"
        );
    }
    Ok(())
}

#[test]
fn actual_definition_readers_use_only_canonical_datasets_and_direct_present_identities()
-> TestResult {
    let (_, mut catalog) = fixture::build(&fixture::database())?;
    assert_physical_ordinals(&catalog)?;
    let expected = commodity_ids(&catalog)?;
    assert!(
        !expected.is_empty(),
        "the producer has selected Commodity definitions"
    );
    let known = crate::real_archive_shop_items::guids_of(&catalog)?;
    assert!(
        expected.iter().all(|guid| known.contains(guid.as_str())),
        "the actual shop acceptance union includes every canonical Commodity"
    );
    catalog.as_object_mut().ok_or("catalog object")?.insert(
        "commodities".to_owned(),
        json!([{ "guid": "legacy-distraction" }]),
    );
    assert_eq!(
        commodity_ids(&catalog)?,
        expected,
        "a legacy root never becomes the row source"
    );
    for kind in [Kind::Commodities, Kind::ResourceTypes] {
        let rows = crate::real_archive_verification::list(&catalog, kind)?;
        assert!(!rows.is_empty(), "actual canonical {kind} rows are read");
        for row in rows {
            assert!(
                !reader::identity(row, kind)?.is_empty(),
                "actual sourced identity is retained"
            );
        }
    }
    Ok(())
}

#[test]
fn missing_canonical_tables_fail_the_actual_definition_reader() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    for path in [
        "/definitions/commodities",
        "/definitions/resourceTypes",
        "/projections/commodityPresentation",
    ] {
        let mut missing = catalog.clone();
        let (owner, field) = path.rsplit_once('/').ok_or("dataset path")?;
        missing
            .pointer_mut(owner)
            .and_then(Value::as_object_mut)
            .ok_or("dataset parent")?
            .remove(field)
            .ok_or("canonical dataset")?;
        let kind = if path.ends_with("resourceTypes") {
            Kind::ResourceTypes
        } else {
            Kind::Commodities
        };
        assert!(
            reader::rows(&missing, kind).is_err(),
            "missing canonical {path} must fail"
        );
    }
    Ok(())
}

#[test]
fn wrong_source_states_and_missing_presentation_identities_are_not_empty_successes() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    for state in ["absent", "null", "unknown"] {
        let mut invalid = catalog.clone();
        let identity = invalid
            .pointer_mut("/definitions/commodities/rows/0/guid")
            .and_then(Value::as_object_mut)
            .ok_or("identity carrier")?;
        identity.insert("state".to_owned(), json!(state));
        identity.insert("value".to_owned(), Value::Null);
        assert!(
            reader::rows(&invalid, Kind::Commodities).is_err(),
            "invalid identity state {state} cannot disappear"
        );
    }
    let mut missing = catalog;
    missing
        .pointer_mut("/projections/commodityPresentation/rows")
        .and_then(Value::as_array_mut)
        .ok_or("presentation rows")?
        .pop()
        .ok_or("actual projection")?;
    assert!(
        reader::presentations(&missing).is_err(),
        "identity closure requires every definition's one presentation"
    );
    Ok(())
}

#[test]
fn shared_and_nullable_packaging_never_changes_canonical_ingredient_identity() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let resource = fixture::guid(fixture::RESOURCE);
    let (mut repeated, mut unlinked) = (0, 0);
    let mut unique = HashSet::new();
    for row in reader::presentations(&catalog)? {
        let subtype = reader::nullable_guid(row.get("subtypeGuid").ok_or("subtype carrier")?)?;
        if let Some(guid) = subtype {
            unique.insert(guid);
        }
        match subtype {
            Some(guid) if guid == resource => repeated += 1,
            None => unlinked += 1,
            _ => {}
        }
    }
    assert_eq!(
        (repeated, unlinked, unique.len()),
        (2, 1, 2),
        "authored alternatives retain n:1 counts, null and a separately deduplicated resource set"
    );
    let unused = fixture::guid(fixture::RESOURCE_UNUSED);
    let lines = HashSet::from([resource.as_str(), unused.as_str()]);
    crate::real_archive_crafting::check_resource_links(&catalog, &lines)?;
    let mut missing = catalog.clone();
    missing
        .pointer_mut("/definitions/resourceTypes/rows")
        .and_then(Value::as_array_mut)
        .ok_or("resource rows")?
        .retain(|row| {
            row.pointer("/guid/value").and_then(Value::as_str) != Some(resource.as_str())
        });
    assert!(
        crate::real_archive_crafting::check_resource_links(&missing, &lines).is_err(),
        "a true missing definition is not a valid n:1 case"
    );
    Ok(())
}

#[test]
fn actual_resource_fallback_reuse_is_not_a_global_guid_collision_waiver() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    crate::real_archive_verification::check_unique_guids(&catalog)?;
    let fallback = fixture::definition(&catalog, fixture::RESOURCE_FALLBACK)?;
    assert_eq!(
        fixture::payload(fallback, "selection")?,
        &json!("resourceFallback"),
        "the reused source is the actual Resource fallback"
    );
    let presentation = fixture::presentation(&catalog, fixture::RESOURCE_FALLBACK)?;
    assert_eq!(
        fixture::payload(presentation, "commodityGuid")?,
        &json!(fixture::guid(fixture::RESOURCE_FALLBACK)),
        "the projection retains the fallback source identity"
    );
    Ok(())
}

#[test]
fn a_same_guid_nonfallback_cannot_hide_behind_resource_fallback_reuse() -> TestResult {
    let (_, mut catalog) = fixture::build(&fixture::database())?;
    let expected = fixture::guid(fixture::RESOURCE_FALLBACK);
    let rows = catalog
        .pointer_mut("/definitions/commodities/rows")
        .and_then(Value::as_array_mut)
        .ok_or("actual Commodity rows")?;
    let fallback = rows
        .iter_mut()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(expected.as_str()))
        .ok_or("actual resource-only fallback")?;
    fallback
        .get_mut("selection")
        .and_then(Value::as_object_mut)
        .ok_or("selection carrier")?
        .insert("value".to_owned(), json!("entity"));
    let Err(error) = crate::real_archive_verification::check_unique_guids(&catalog) else {
        return Err("the real uniqueness helper accepted an altered source identity".into());
    };
    assert!(
        error.to_string().contains("occurs twice"),
        "the exact cross-family identity rule rejects the mutation"
    );
    Ok(())
}

#[test]
fn repeated_resource_fallback_row_is_rejected_within_its_family() -> TestResult {
    let (_, mut catalog) = fixture::build(&fixture::database())?;
    let guid = fixture::guid(fixture::RESOURCE_FALLBACK);
    let rows = catalog
        .pointer_mut("/definitions/resourceTypes/rows")
        .and_then(Value::as_array_mut)
        .ok_or("ResourceType rows")?;
    let repeated = rows
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or("actual ResourceType fallback")?
        .clone();
    rows.push(repeated);
    assert!(
        crate::real_archive_verification::check_unique_guids(&catalog).is_err(),
        "a legitimate cross-family fallback does not admit a repeated row inside ResourceTypes"
    );
    Ok(())
}

#[test]
fn quality_definitions_require_the_canonical_table_and_source_identity() -> TestResult {
    let (_, mut catalog) = fixture::build(&fixture::database())?;
    let rows = reader::rows(&catalog, Kind::QualityQuantizations)?;
    assert!(
        !rows.is_empty(),
        "the actual producer retains selected quantization definitions"
    );
    for row in rows {
        assert!(
            !reader::identity(row, Kind::QualityQuantizations)?.is_empty(),
            "canonical sourced GUID identity"
        );
        assert!(
            reader::field(row, Kind::QualityQuantizations, "recordIndex")?.is_number(),
            "canonical physical position"
        );
    }
    catalog.as_object_mut().ok_or("catalog")?.insert(
        "qualityQuantizations".to_owned(),
        json!([{"guid":"distraction"}]),
    );
    catalog
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("definitions")?
        .remove("qualityQuantizations")
        .ok_or("canonical quality table")?;
    assert!(
        reader::rows(&catalog, Kind::QualityQuantizations).is_err(),
        "a flat distraction cannot replace a required canonical table"
    );
    Ok(())
}

#[test]
fn quality_identity_allows_physical_alternatives_but_rejects_repeated_positions_and_wrong_families()
-> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let original = reader::rows(&catalog, Kind::QualityQuantizations)?
        .first()
        .ok_or("quality definition")?;
    let mut alias = original.clone();
    *alias
        .pointer_mut("/recordIndex/value")
        .ok_or("physical position")? = json!(10_000);
    let mut valid = catalog.clone();
    valid
        .pointer_mut("/definitions/qualityQuantizations/rows")
        .and_then(Value::as_array_mut)
        .ok_or("quality rows")?
        .push(alias.clone());
    crate::real_archive_verification::check_unique_guids(&valid)?;
    let mut repeated = catalog.clone();
    repeated
        .pointer_mut("/definitions/qualityQuantizations/rows")
        .and_then(Value::as_array_mut)
        .ok_or("quality rows")?
        .push(original.clone());
    assert!(
        crate::real_archive_verification::check_unique_guids(&repeated).is_err(),
        "physical alternatives do not excuse repeated original positions"
    );
    *alias
        .pointer_mut("/sourceType/value")
        .ok_or("source family")? = json!("ResourceType");
    let mut wrong = catalog;
    wrong
        .pointer_mut("/definitions/qualityQuantizations/rows")
        .and_then(Value::as_array_mut)
        .ok_or("quality rows")?
        .push(alias);
    assert!(
        crate::real_archive_verification::check_unique_guids(&wrong).is_err(),
        "a different source family cannot use the physical-alternative rule"
    );
    Ok(())
}
