// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

use super::{ROWS, TestResult};
use crate::component_sdk_parity::ComponentSdkParity;

const EXPLOSIVES: &str = "/configurations/explosives/rows";

pub(super) fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    catalog: &Value,
    owner: u32,
) -> TestResult {
    let comparison = ComponentSdkParity::new(archive, entry, catalog)?;
    let session = CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "admit the same installed Archive for the ordnance SDK")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse the same DataForge for the ordnance SDK")?;
    let selection = CatalogRecordSelection::RecordIndex(owner);
    parsed
        .with_missile_configurations(selection, None, |snapshot| {
            comparison.compare(ROWS, owner, &snapshot)
        })
        .map_err(|_| "query missile configuration for the installed owner")??;
    parsed
        .with_explosives(selection, None, |snapshot| {
            comparison.compare(EXPLOSIVES, owner, &snapshot)
        })
        .map_err(|_| "query explosive configuration for the installed owner")??;
    Ok(())
}
