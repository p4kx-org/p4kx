// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::blueprint::as_struct;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::OriginalFlowFields;
use crate::mission_intros::{Grants, INTRO_CONTRACTS};
use crate::mission_layers::items;
use crate::record_fields::{identity, text};
use crate::record_read::ReadError;
use crate::selection::is_leftover;

const CONTAINERS: [&str; 4] = [
    "contracts",
    INTRO_CONTRACTS,
    "legacyContracts",
    "PVPBountyContract",
];

/// One admitted contract in its actual original handler container.
pub(crate) struct ContractCandidate<'tree, 'source> {
    pub(crate) container: &'static str,
    pub(crate) contract: &'tree StructValue<'source>,
    pub(crate) name: Option<&'source str>,
}

pub(crate) fn handlers<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    context: &CatalogRecordContext<'_>,
    grants: &mut Grants,
    mut visit: impl FnMut(&'tree StructValue<'source>, &mut Grants) -> Result<usize, ReadError>,
) -> Result<usize, ReadError> {
    fields.check_tree(tree)?;
    grants.begin_record(fields, context)?;
    let mut skipped = 0_usize;
    for value in items(Some(tree.root()), "generators") {
        grants.handler_slot(fields, value)?;
        if let Some(handler) = as_struct(value) {
            skipped = skipped.saturating_add(visit(handler, grants)?);
        }
    }
    Ok(skipped)
}

pub(crate) fn candidates<'tree, 'source>(
    tree: &RecordTree<'source>,
    handler: &'tree StructValue<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    grants: &mut Grants,
    mut visit: impl FnMut(ContractCandidate<'tree, 'source>) -> Result<usize, ReadError>,
) -> Result<usize, ReadError> {
    let mut skipped = 0_usize;
    for container in CONTAINERS {
        let intro = (container == INTRO_CONTRACTS)
            .then(|| grants.candidates(fields, handler))
            .transpose()?;
        for value in items(Some(handler), container) {
            let contract = as_struct(value);
            let name = text(contract, "debugName");
            let id = contract
                .and_then(identity)
                .filter(|_| !is_leftover(tree.record_path(), name.unwrap_or_default()));
            if let Some(intro) = &intro {
                grants.candidate(fields, intro, value, id)?;
            }
            if let Some(contract) = contract {
                if id.is_none() {
                    skipped = skipped.saturating_add(1);
                } else {
                    skipped = skipped.saturating_add(visit(ContractCandidate {
                        container,
                        contract,
                        name,
                    })?);
                }
            }
        }
    }
    Ok(skipped)
}

pub(crate) fn rows<'tree, 'source>(
    tree: &RecordTree<'source>,
    contract: &'tree StructValue<'source>,
    mut visit: impl FnMut(Option<&'tree StructValue<'source>>) -> Result<(), ReadError>,
) -> Result<usize, ReadError> {
    visit(None)?;
    let mut skipped = 0_usize;
    for sub in items(Some(contract), "subContracts")
        .iter()
        .filter_map(as_struct)
    {
        let name = text(Some(sub), "debugName").unwrap_or_default();
        if identity(sub).is_none() || is_leftover(tree.record_path(), name) {
            skipped = skipped.saturating_add(1);
        } else {
            visit(Some(sub))?;
        }
    }
    Ok(skipped)
}
