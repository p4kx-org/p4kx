// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
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
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    reader::presentations(&catalog)?;
    Ok(())
}

#[test]
fn actual_encrypted_inputs_keep_direct_proof_through_acceptance_reads() -> TestResult {
    let database = fixture::database();
    let (_directory, archive) = fixture::open(&database, fixture::DcbOrigin::Encrypted)?;
    let built = p4kx_export::build_catalog(&archive, &fixture::options())?;
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
    let built = p4kx_export::build_catalog(&archive, &fixture::options())?;
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
