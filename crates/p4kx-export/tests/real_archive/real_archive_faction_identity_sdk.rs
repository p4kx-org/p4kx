// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::Kind;
use p4kx_export::catalog_queries::faction_identities::FactionIdentitySnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::{Archive, Build, DataForgeEntry};
use serde_json::Value;

#[path = "real_archive_faction_text_scope.rs"]
mod text_scope;

use super::TestResult;
use crate::catalog_v3_consumer;
use crate::real_archive_catalog_table_parity::Origins;
use text_scope::TextScope;

pub(super) fn verify(archive: &Archive, entry: &DataForgeEntry, catalog: &Value) -> TestResult {
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )
    .map_err(|_| "admit the original FactionIdentity Archive Entry")?;
    let queries = session
        .parsed(None)
        .map_err(|_| "open the bound SDK DataForge")?;
    let texts = TextScope::read(catalog)?;
    let mut origins = Origins::default();
    for expected in catalog_v3_consumer::rows(catalog, Kind::FactionIdentities)?.as_slice() {
        let owner = expected
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or("original FactionIdentity Record index")?;
        queries
            .with_faction_identities(CatalogRecordSelection::RecordIndex(owner), None, |sdk| {
                compare(&sdk, archive.build(), expected, &texts, owner, &mut origins)
            })
            .map_err(|_| "read the original physical FactionIdentity through the SDK")??;
    }
    origins.verify_in_document(catalog)
}

fn compare(
    snapshot: &FactionIdentitySnapshot<'_>,
    build: Option<&Build>,
    expected: &Value,
    texts: &TextScope<'_>,
    owner: u32,
    origins: &mut Origins,
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("SDK did not return exactly one physical FactionIdentity".into());
    };
    if snapshot.build.as_ref() != build
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || !snapshot.coverage.unreadable_headers.is_empty()
        || !snapshot.coverage.unreadable_bodies.is_empty()
    {
        return Err("SDK Build, availability or physical FactionIdentity read differs".into());
    }
    let actual = serde_json::from_slice(&serde_json::to_vec(row)?)?;
    text_scope::compare_row(expected, &actual)?;
    origins.extend(&snapshot.source_inventory, std::slice::from_ref(&actual))?;
    compare_texts(snapshot, expected, texts, owner, origins)
}

fn compare_texts(
    snapshot: &FactionIdentitySnapshot<'_>,
    expected: &Value,
    texts: &TextScope<'_>,
    owner: u32,
    origins: &mut Origins,
) -> TestResult {
    let mut expected_ids: BTreeSet<_> = ["/name/value", "/description/value"]
        .into_iter()
        .filter_map(|path| expected.pointer(path).and_then(Value::as_str))
        .collect();
    for text in &snapshot.game_texts {
        let id = text.id().value().ok_or("protected text identity")?;
        if !expected_ids.remove(id.as_str()) {
            return Err("unexpected or duplicate SDK protected text row".into());
        }
        let expected_text = texts.for_owner(id.as_str(), owner)?;
        let actual_text = serde_json::to_value(text)?;
        text_scope::compare_row(&expected_text, &actual_text)?;
        origins.extend(
            &snapshot.source_inventory,
            std::slice::from_ref(&actual_text),
        )?;
    }
    if expected_ids.is_empty() {
        Ok(())
    } else {
        Err("missing SDK protected text row for original Locale reference".into())
    }
}
