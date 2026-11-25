// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::Language;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};

#[test]
fn optional_presentation_values_and_unknown_origin_remain_real_values() -> TestResult {
    for declared in [false, true] {
        let database = fixture::parent_arrays(declared);
        let (_, catalog) = fixture::build(&database)?;
        for choice in reader::presentations(&catalog)? {
            assert!(
                reader::present(choice.get("name").ok_or("nullable name carrier")?)?.is_null(),
                "no authored name invents a text reference"
            );
        }
    }
    let catalog = fixture::unknown(&fixture::database())?;
    assert!(
        reader::rows(&catalog, Kind::ResourceTypes)?
            .iter()
            .all(|row| {
                row.pointer("/guid/containsEncryptedSource")
                    .is_some_and(Value::is_null)
            }),
        "unknown source proof is not promoted by an acceptance reader"
    );
    let database = fixture::dangling_first();
    let (_directory, archive) = fixture::open(&database, fixture::DcbOrigin::Plain)?;
    let mut options = fixture::options();
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    reader::presentations(&catalog)?;
    Ok(())
}

#[test]
fn actual_encrypted_inputs_keep_direct_proof_through_acceptance_reads() -> TestResult {
    let database = fixture::database();
    let (_directory, archive) = fixture::open(&database, fixture::DcbOrigin::Encrypted)?;
    let built = p4kx_export::build_catalog(&archive, fixture::options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    for kind in [Kind::Commodities, Kind::ResourceTypes] {
        for row in reader::rows(&catalog, kind)? {
            assert_eq!(
                row.pointer("/guid/containsEncryptedSource"),
                Some(&json!(true)),
                "the actual encrypted primary source remains directly filterable"
            );
        }
    }
    let choice = fixture::presentation(&catalog, fixture::RESOURCE_FALLBACK)?;
    let target = fixture::text_row(&catalog, "publicNames", fixture::payload(choice, "name")?)?;
    for field in ["sourceText", "displayText"] {
        let language = fixture::at(target, &format!("/translations/value/en/{field}"))?;
        assert_eq!(
            reader::present(language)?,
            &json!("Fallback resource"),
            "both actual English forms are reached without a copied flat name"
        );
        assert_eq!(
            language.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the separately encrypted locale input keeps each form's direct marker"
        );
    }
    Ok(())
}

#[test]
fn source_carriers_allow_additive_metadata_but_require_all_contract_keys() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let original = fixture::definition(&catalog, fixture::ALPHA)?
        .get("guid")
        .ok_or("actual producer identity carrier")?;
    let mut additive = original.clone();
    additive
        .as_object_mut()
        .ok_or("identity carrier")?
        .insert("extraMetadata".to_owned(), json!({"known":true}));
    assert_eq!(
        reader::present(&additive)?,
        reader::present(original)?,
        "additive object metadata does not invalidate the published contract"
    );
    for field in [
        "state",
        "value",
        "sourceUsesRefs",
        "sourceSetComplete",
        "containsEncryptedSource",
    ] {
        let mut missing = original.clone();
        missing
            .as_object_mut()
            .ok_or("identity carrier")?
            .remove(field)
            .ok_or("actual required source key")?;
        assert!(
            reader::source_value(&missing).is_err(),
            "missing {field} is not an optional value"
        );
    }
    let mut incomplete = original.clone();
    incomplete
        .as_object_mut()
        .ok_or("identity carrier")?
        .insert("sourceSetComplete".to_owned(), json!(false));
    assert!(
        reader::source_value(&incomplete).is_err(),
        "a false direct marker cannot claim an incomplete contributor set"
    );
    incomplete
        .as_object_mut()
        .ok_or("identity carrier")?
        .insert("containsEncryptedSource".to_owned(), Value::Null);
    assert_eq!(
        reader::present(&incomplete)?,
        reader::present(original)?,
        "unknown source proof preserves the actual payload"
    );
    Ok(())
}

#[test]
fn empty_literal_and_unknown_origin_controls_keep_their_payload() -> TestResult {
    for (marker, complete) in [(json!(false), true), (Value::Null, false)] {
        let carrier = json!({
            "state":"present", "value":"synthetic literal",
            "containsEncryptedSource":marker, "sourceSetComplete":complete, "sourceUsesRefs":[]
        });
        assert_eq!(
            reader::present(&carrier)?,
            &json!("synthetic literal"),
            "a consistent empty origin retains the actual protocol payload"
        );
    }
    Ok(())
}

#[test]
fn empty_encrypted_origin_cannot_supply_a_known_witness() -> TestResult {
    let database = fixture::database();
    let (_directory, archive) = fixture::open(&database, fixture::DcbOrigin::Encrypted)?;
    let built = p4kx_export::build_catalog(&archive, fixture::options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let mut carrier = fixture::definition(&catalog, fixture::ALPHA)?
        .get("guid")
        .ok_or("actual encrypted GUID carrier")?
        .clone();
    assert_eq!(
        carrier.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the original encrypted source supplies a positive witness"
    );
    carrier
        .as_object_mut()
        .ok_or("GUID carrier")?
        .insert("sourceUsesRefs".to_owned(), json!([]));
    assert!(
        reader::source_value(&carrier).is_err(),
        "a positive encryption marker requires an actual contributor witness"
    );
    Ok(())
}

#[test]
fn empty_complete_unknown_origin_is_not_an_authenticated_literal() -> TestResult {
    let (_, catalog) = fixture::build(&fixture::database())?;
    let mut carrier = fixture::definition(&catalog, fixture::ALPHA)?
        .get("guid")
        .ok_or("actual GUID carrier")?
        .clone();
    let fields = carrier.as_object_mut().ok_or("GUID carrier")?;
    fields.insert("sourceUsesRefs".to_owned(), json!([]));
    fields.insert("containsEncryptedSource".to_owned(), Value::Null);
    fields.insert("sourceSetComplete".to_owned(), json!(true));
    assert!(
        reader::source_value(&carrier).is_err(),
        "complete empty contributors carry the known literal marker instead of unknown"
    );
    Ok(())
}

#[test]
fn mixed_text_sources_keep_direct_markers_and_protection_in_one_row() -> TestResult {
    let database = fixture::with_keys("@shared", "@shared", "@resource_name");
    let french = TestEntry::stored(
        Language::French.global_ini_path(),
        "shared=Texte partagé\n".as_bytes(),
    )
    .encrypted()
    .with_encryption_field(7);
    let (_directory, archive) =
        fixture::open_bytes_with_entries(database.build(), fixture::DcbOrigin::Plain, [french])?;
    let mut options = fixture::options();
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    reader::presentations(&catalog)?;

    let owner = fixture::presentation(&catalog, fixture::ALPHA)?;
    let public_id = reader::present(fixture::at(owner, "/name")?)?;
    let protected_id = reader::present(fixture::at(owner, "/description")?)?;
    assert_ne!(
        public_id, protected_id,
        "equal original keys keep distinct text domains"
    );
    let public = fixture::text_row(&catalog, "publicNames", public_id)?;
    let protected = fixture::text_row(&catalog, "gameTexts", protected_id)?;
    assert!(
        fixture::text_row(&catalog, "gameTexts", public_id).is_err()
            && fixture::text_row(&catalog, "publicNames", protected_id).is_err(),
        "each physical input remains reachable through the correct text domain",
    );
    let schema = p4kx_export::catalog_schema();
    assert_eq!(
        schema.pointer("/$defs/CommodityPresentation/properties/description/x-gameText"),
        Some(&Value::Bool(true)),
        "the protected reference is marked independently of encryption",
    );
    assert_ne!(
        schema.pointer("/$defs/CommodityPresentation/properties/name/x-gameText"),
        Some(&Value::Bool(true)),
        "the short name stays public across all three source states",
    );

    for row in [public, protected] {
        verify_mixed_translations(&catalog, row)?;
    }
    reject_aggregate_only_text_marker(public)
}

fn verify_mixed_translations(catalog: &Value, row: &Value) -> TestResult {
    assert_eq!(
        reader::present(fixture::at(row, "/key")?)?,
        &json!("@shared"),
        "the same original key is present in both domains",
    );
    assert_eq!(
        fixture::at(row, "/translations/containsEncryptedSource")?,
        &json!(true),
        "the enclosing provenance includes an encrypted locale",
    );
    for (language, text, marker) in [
        (Language::English, "Shared text", Some(true)),
        (Language::German, "Gemeinsamer Text", Some(false)),
        (Language::French, "Texte partagé", None),
    ] {
        for field in ["sourceText", "displayText"] {
            let path = format!("/translations/value/{}/{field}", language.code());
            let carrier = fixture::at(row, &path)?;
            assert_eq!(
                reader::present(carrier)?,
                &json!(text),
                "{path} retains its original readable text",
            );
            assert_eq!(
                fixture::at(carrier, "/containsEncryptedSource")?,
                &json!(marker),
                "{path} carries only its own source marker",
            );
            assert_eq!(
                fixture::at(carrier, "/sourceSetComplete")?,
                &json!(true),
                "{path} identifies its complete source set",
            );
            verify_locale_use(catalog, carrier, language, marker)?;
        }
    }
    Ok(())
}

fn reject_aggregate_only_text_marker(public: &Value) -> TestResult {
    let path = "/translations/value/de/displayText";
    let mut aggregate_only = public.clone();
    aggregate_only
        .pointer_mut(path)
        .and_then(Value::as_object_mut)
        .ok_or("original clear translation carrier")?
        .remove("containsEncryptedSource")
        .ok_or("original direct marker")?;
    assert_eq!(
        fixture::at(&aggregate_only, "/translations/containsEncryptedSource")?,
        &json!(true),
        "the mutant retains the aggregate marker",
    );
    assert!(
        reader::present(fixture::at(&aggregate_only, path)?).is_err(),
        "an aggregate marker cannot replace the required direct field marker",
    );
    Ok(())
}

fn verify_locale_use(
    catalog: &Value,
    carrier: &Value,
    language: Language,
    marker: Option<bool>,
) -> TestResult {
    let refs = fixture::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("direct text source references")?;
    let expected_encryption = match marker {
        Some(true) => "encrypted",
        Some(false) => "unencrypted",
        None => "unknown",
    };
    let mut locale_payload = false;
    for id in refs {
        let used = inventory_row(catalog, "/provenance/uses", id)?;
        let source = inventory_row(
            catalog,
            "/provenance/sources",
            fixture::at(used, "/source")?,
        )?;
        assert_eq!(
            fixture::at(source, "/kind")?,
            &json!("archiveEntry"),
            "a text use closes to an actual Archive Entry",
        );
        assert_eq!(
            fixture::at(used, "/scope")?,
            &json!("payload"),
            "the source use addresses original payload bytes",
        );
        if used.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup") {
            assert_eq!(
                fixture::at(source, "/path/value")?,
                &json!(language.global_ini_path()),
                "the contribution is from the selected language Entry",
            );
            assert_eq!(
                fixture::at(used, "/effectiveEncryption")?,
                &json!(expected_encryption),
                "the direct use retains that Entry's declared encryption evidence",
            );
            locale_payload |= used.get("role").and_then(Value::as_str) == Some("payload");
        } else {
            assert_eq!(
                fixture::at(source, "/path/value")?,
                &json!("Data\\Game2.dcb"),
                "other contributions are original key selectors from the DCB",
            );
        }
    }
    assert!(
        locale_payload,
        "an actual locale payload supplies each present text"
    );
    Ok(())
}

fn inventory_row<'catalog>(
    catalog: &'catalog Value,
    path: &str,
    id: &Value,
) -> TestResult<&'catalog Value> {
    let mut matches = fixture::at(catalog, path)?
        .as_array()
        .ok_or("actual provenance inventory rows")?
        .iter()
        .filter(|row| row.get("id") == Some(id));
    let row = matches.next().ok_or("closed text provenance reference")?;
    if matches.next().is_some() {
        return Err("duplicate referenced provenance identity".into());
    }
    Ok(row)
}
