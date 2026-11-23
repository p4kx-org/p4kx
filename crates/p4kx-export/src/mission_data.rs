// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::{CatalogProjection, ProtectedKey};
use crate::catalog_read::original::OriginalFlowFields;

use crate::contract_mission_data::MissionType;
use crate::record_observation::RecordObservation;

/// Reads the mission type and the protected reference from its original name field.
pub(crate) fn mission_type<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<MissionType<'source>, CatalogError> {
    let scope = projection
        .original()
        .record_scope(admission.record(), tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let type_name = projection.finish_protected_key(mission_type_key(&fields)?)?;
    Ok(MissionType {
        source: CanonicalRecordHeader::read(tree, projection.origins())?,
        type_name,
    })
}

/// Captures the protected source field shared by mission types and contract rows.
pub(crate) fn mission_type_key<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
) -> Result<ProtectedKey<'source>, CatalogError> {
    ProtectedKey::read(fields, None, "LocalisedTypeName")
}
