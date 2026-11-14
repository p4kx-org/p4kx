// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, VecDeque};

use super::{Anchors, Notice, Owner, Scan, Token, scan};
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryRead};
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::shop_inventory_path::InventoryStem;
use crate::{CatalogError, ExportAccountingError};

pub(crate) struct CapturedScan {
    pub(crate) scan: Scan<InventoryStem>,
    pub(crate) no_file: HashMap<String, Vec<AuthoredEntryPath>>,
    pub(crate) rejected: HashMap<String, Vec<RejectedInventoryPath>>,
    pub(crate) unattributed_declarations: bool,
}

pub(crate) fn read_components(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    anchors: &Anchors,
) -> Result<CapturedScan, CatalogError> {
    let mut markers: HashMap<String, Vec<AuthoredEntryPath>> = HashMap::new();
    let mut rejected: HashMap<String, Vec<RejectedInventoryPath>> = HashMap::new();
    let mut unattributed_declarations = false;
    let scan = scan(
        read.bytes(),
        anchors,
        |token, window, stem, owner| {
            let path = authored(read, capture, token, window, &owner)?;
            InventoryStem::captured(capture, &path, stem)
        },
        |notice, token, window, owner| {
            match (notice, owner) {
                (Notice::Marker, Some(owner)) => {
                    let path = authored(read, capture, token, window, owner)?;
                    retain(&mut markers, owner, path)?;
                }
                (Notice::Rejected, Some(owner)) => {
                    let value = rejected_value(read, capture, token, window, owner)?;
                    retain(&mut rejected, owner, value)?;
                }
                (Notice::Rejected, None) => unattributed_declarations = true,
                (Notice::Marker, None) => {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
            Ok(())
        },
    )?;
    let mut no_file = HashMap::new();
    for component in &scan.components {
        if component.inventory.is_none()
            && let Some(original) = markers.remove(&component.entity)
        {
            no_file.insert(component.entity.clone(), original);
        }
    }
    Ok(CapturedScan {
        scan,
        no_file,
        rejected,
        unattributed_declarations,
    })
}

fn retain<T>(
    values: &mut HashMap<String, Vec<T>>,
    owner: &Owner<'_>,
    value: T,
) -> Result<(), CatalogError> {
    values
        .try_reserve(1)
        .map_err(CatalogError::SourceUseAllocation)?;
    let values = values.entry(owner.anchor.entity.to_owned()).or_default();
    values
        .try_reserve(1)
        .map_err(CatalogError::SourceUseAllocation)?;
    values.push(value);
    Ok(())
}

fn rejected_value(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    token: Token<'_>,
    window: &VecDeque<Token<'_>>,
    owner: &Owner<'_>,
) -> Result<RejectedInventoryPath, CatalogError> {
    if token.text.is_empty() {
        let property = window
            .back()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        return Ok(RejectedInventoryPath::Unreadable(authored(
            read, capture, *property, window, owner,
        )?));
    }
    Ok(RejectedInventoryPath::Text(authored(
        read, capture, token, window, owner,
    )?))
}

fn authored(
    read: &EntryRead,
    capture: &EntryCapture<'_>,
    token: Token<'_>,
    window: &VecDeque<Token<'_>>,
    owner: &Owner<'_>,
) -> Result<AuthoredEntryPath, CatalogError> {
    let mut path = read.string_slot(capture, token.offset, token.text)?;
    for selected in window.iter().skip(owner.start) {
        path = path.selected_by(&read.string_slot(capture, selected.offset, selected.text)?)?;
    }
    for selected in owner.anchor.selectors {
        path = path.selected_by(selected)?;
    }
    Ok(path)
}
