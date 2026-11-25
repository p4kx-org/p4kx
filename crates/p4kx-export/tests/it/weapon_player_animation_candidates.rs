// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression, write_catalog_from};

use serde_json::Value;

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

#[path = "weapon_player_animation_evidence.rs"]
mod evidence;
#[path = "weapon_player_animation_fixture.rs"]
mod fixture;
#[path = "weapon_player_animation_sdk.rs"]
mod sdk;
#[path = "weapon_player_animation_sources_sdk.rs"]
mod sources_sdk;

use fixture::{CANDIDATE, database, documents};

#[test]
fn original_reload_options_join_only_through_their_physical_scopes() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Encrypted, documents())?;
    let (built, catalog) = fixture.diagnostic()?;
    check_compressed_replay(&fixture, built.json())?;
    let items = origin::rows(&catalog, "/definitions/items/rows")?;
    let physical: Vec<_> = items
        .iter()
        .map(|item| item.pointer("/recordIndex/value").and_then(Value::as_u64))
        .collect();
    assert_eq!(
        physical,
        [Some(2), Some(1)],
        "item definition order follows canonical GUIDs across physical scan order"
    );
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    schema
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    let candidates = origin::rows(
        &catalog,
        "/projections/weaponPlayerAnimationCandidates/rows",
    )?;
    let [first, _] = candidates else {
        return Err("two distinct options for the same physical weapon and scoped fragment".into());
    };
    assert_eq!(
        first
            .pointer("/weaponRecordIndex/value")
            .and_then(Value::as_u64),
        Some(1),
        "the join identifies the original personal weapon Record",
    );
    assert_eq!(
        first
            .pointer("/attachmentComponentOrdinal/value")
            .and_then(Value::as_u64),
        Some(0),
        "the join identifies the physical AttachDef component",
    );
    check_candidate_order(candidates)?;
    evidence::check(&catalog, candidates)?;
    check_sources(&catalog)?;
    origin::closure(&catalog)
}

fn check_candidate_order(candidates: &[Value]) -> TestResult {
    let positions = candidates
        .iter()
        .map(|candidate| {
            origin::at(candidate, "/optionPosition/value/indices/0")?
                .as_str()
                .ok_or("the original XML byte position")?
                .parse::<usize>()
                .map_err(Into::into)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let authored = CANDIDATE
        .match_indices("<Fragment Tags=")
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(
        positions, authored,
        "the candidates retain the two distinct authored positions in source order",
    );
    Ok(())
}

fn check_compressed_replay(fixture: &Fixture, expected: &[u8]) -> TestResult {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..crate::health_origin_fixture::options()
    };
    let (_, packed) = write_catalog_from(
        &fixture.archive,
        &fixture.entry,
        options.with_full_provenance(),
        Compression::Zstd { level: 1 },
        || Ok(Vec::new()),
    )?;
    let decoded = zstd::stream::decode_all(packed.as_slice())?;
    assert_eq!(
        decoded, expected,
        "weapon candidates and their origins survive both compressed writer passes in source order",
    );
    Ok(())
}

fn check_sources(catalog: &Value) -> TestResult {
    let sources = origin::at(catalog, "/definitions/weaponAnimationSources")?;
    check_shared_tag_selection(catalog, sources)?;
    let statuses: Vec<_> = origin::rows(sources, "/fragmentSelectors")?
        .iter()
        .filter_map(|row| row.pointer("/status/value").and_then(Value::as_str))
        .collect();
    assert!(
        statuses.contains(&"matched"),
        "the identical physical FragDef admits the alternatives"
    );
    assert!(
        statuses.contains(&"differentScope"),
        "an equal fragment spelling cannot cross another FragDef"
    );
    assert!(
        statuses.contains(&"noMatchingGroup"),
        "an explicit unmatched FragmentID is retained"
    );
    assert!(
        statuses.contains(&"unresolvedTarget"),
        "a missing SubADB preserves its source selector"
    );
    let branches = origin::rows(sources, "/branches")?;
    assert!(
        branches
            .iter()
            .any(|row| row.pointer("/traversal/value").and_then(Value::as_str) == Some("cycle")),
        "an actual cycle has an explicit source result"
    );
    let databases = origin::rows(sources, "/databases")?;
    let has_proc_layer = databases
        .iter()
        .flat_map(|row| row["options"].as_array().into_iter().flatten())
        .flat_map(|row| row["elements"].as_array().into_iter().flatten())
        .any(|row| row.pointer("/name/value").and_then(Value::as_str) == Some("ProcLayer"));
    assert!(
        has_proc_layer,
        "uninterpreted original procedural layers survive"
    );
    Ok(())
}

fn check_shared_tag_selection(catalog: &Value, sources: &Value) -> TestResult {
    let scopes = origin::rows(sources, "/scopes")?;
    let [first, rest @ ..] = scopes else {
        return Err("physical ADB tag scopes".into());
    };
    assert_eq!(rest.len(), 2, "three databases declare the shared tag root");
    let declarations = origin::rows(first, "/tags/declarations")?;
    assert_eq!(declarations.len(), 1, "the original imported tag");
    let imports = origin::rows(first, "/tags/imports")?;
    assert_eq!(imports.len(), 1, "the original shared import");
    let uses = origin::rows(catalog, "/provenance/uses")?;
    for scope in scopes {
        assert_eq!(
            origin::rows(scope, "/tags/declarations")?,
            declarations,
            "same physical definition retains the same declaration rows"
        );
        assert_eq!(
            origin::rows(scope, "/tags/imports")?,
            imports,
            "each scope preserves its original matching import"
        );
        let database = origin::at(scope, "/database/inputRef")?;
        let coverage = origin::rows(scope, "/tags/coverage/sourceUsesRefs")?;
        assert!(
            uses.iter().any(|row| {
                row.get("source") == Some(database)
                    && row.get("id").is_some_and(|id| coverage.contains(id))
            }),
            "the coverage includes the arriving database's own selector"
        );
    }
    Ok(())
}
