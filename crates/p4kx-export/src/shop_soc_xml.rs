// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::Container;
use p4kx_extraction::model::SocEntitySourceError;

use super::observation::{self, CapturedScan};
use super::{Anchors, ShopComponent};
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryRead, EntrySoc, ShopDeclarations,
};
use crate::shop_inventory_name;
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::shop_inventory_path::InventoryStem;
use crate::{CatalogError, shop_registry};

pub(crate) fn read_components(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    anchors: &Anchors,
) -> Result<CapturedScan, CatalogError> {
    let soc = match EntrySoc::parse(read, capture)? {
        Ok(soc) => soc,
        Err(SocEntitySourceError::Chunks(_)) if legacy_tokens(read.bytes()) => {
            return observation::read_components(read, capture, anchors);
        }
        Err(_) => return Ok(unavailable()),
    };
    let mut found = CapturedScan::default();
    for chunk in soc.chunks() {
        if let Ok(chunk) = chunk
            && !chunk.is_lossy()
        {
            for entity in chunk.entities() {
                take_declaration(
                    &mut found,
                    entity.shop_declarations(capture)?,
                    anchors,
                    capture,
                )?;
            }
        } else {
            incomplete(&mut found);
        }
    }
    if soc
        .non_entity_chunk_payloads()
        .any(|bytes| has_legacy_declarations(bytes, anchors))
    {
        incomplete(&mut found);
    }
    Ok(found)
}

fn take_declaration(
    found: &mut CapturedScan,
    declaration: ShopDeclarations<'_>,
    anchors: &Anchors,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    let (guid, markers, values, unsupported) = match declaration {
        ShopDeclarations::Absent => return Ok(()),
        ShopDeclarations::Unattributed => {
            incomplete(found);
            return Ok(());
        }
        ShopDeclarations::Authored {
            guid,
            markers,
            values,
            unsupported,
        } => (guid, markers, values, unsupported),
    };
    if unsupported {
        incomplete(found);
    }
    let guid = guid.to_ascii_lowercase();
    let [metadata] = anchors.guid_receipts(&guid) else {
        found.scan.rejected += values
            .iter()
            .filter(|path| shop_inventory_name::stem(path.as_str()).is_some())
            .count();
        incomplete(found);
        return Ok(());
    };
    if !shop_registry::is_guid(&guid) {
        incomplete(found);
        return Ok(());
    }
    capture_paths(found, capture, guid, metadata, markers, values, unsupported)
}

fn capture_paths(
    found: &mut CapturedScan,
    capture: &EntryCapture<'_>,
    guid: String,
    metadata: &AuthoredEntryPath,
    markers: Vec<AuthoredEntryPath>,
    values: Vec<AuthoredEntryPath>,
    unsupported: bool,
) -> Result<(), CatalogError> {
    let mut empty = Vec::new();
    let mut populated = false;
    for value in values {
        let value = value.selected_by(metadata)?;
        if value.as_str().trim().is_empty() {
            empty.push(value);
            continue;
        }
        populated = true;
        if let Some(stem) = shop_inventory_name::stem(value.as_str()) {
            let inventory = InventoryStem::captured(capture, &value, stem)?;
            found.scan.components.push(ShopComponent {
                entity: guid.clone(),
                inventory: Some(inventory),
            });
        } else {
            found.scan.malformed = true;
            found
                .rejected
                .entry(guid.clone())
                .or_default()
                .push(RejectedInventoryPath::Text(value));
        }
    }
    if !populated && !unsupported {
        let observed = found.no_file.entry(guid.clone()).or_default();
        for mark in markers {
            observed.push(mark.selected_by(metadata)?);
        }
        observed.extend(empty);
        found.scan.components.push(ShopComponent {
            entity: guid,
            inventory: None,
        });
    }
    Ok(())
}

/// Checks whether the lexical scanner can read an unframed string stream.
fn legacy_tokens(bytes: &[u8]) -> bool {
    !bytes.starts_with(Container::Ivo.signature())
        && bytes.contains(&0)
        && std::str::from_utf8(bytes).is_ok_and(|text| {
            text.chars()
                .all(|character| character == '\0' || !character.is_control())
        })
}

fn has_legacy_declarations(bytes: &[u8], anchors: &Anchors) -> bool {
    let observed = super::components(bytes, anchors);
    observed.rejected > 0 || observed.malformed || !observed.components.is_empty()
}

fn unavailable() -> CapturedScan {
    let mut found = CapturedScan::default();
    incomplete(&mut found);
    found
}

const fn incomplete(found: &mut CapturedScan) {
    found.scan.malformed = true;
    found.source_incomplete = true;
    found.unattributed_declarations = true;
}

#[cfg(test)]
#[path = "shop_soc_xml_tests.rs"]
mod tests;
