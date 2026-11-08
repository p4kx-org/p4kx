// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::guid_text::CatalogGuid;
use crate::resource_group::ResourceMemberships;

/// Binds every selected membership to its actual group GUID and authored list position.
pub(crate) fn values(
    memberships: ResourceMemberships,
    origins: &CatalogOrigins,
    resource_index: u32,
) -> Result<SourcedValue<Vec<SourcedValue<CatalogGuid>>>, CatalogError> {
    let selector = origins.dataforge.at(
        SourceLocation::DataForge {
            record_index: resource_index,
            field: "$guid".to_owned(),
        },
        SourceUseRole::Selector,
    )?;
    let mut all_uses = vec![selector.clone()];
    let mut values = Vec::with_capacity(memberships.members.len());
    for member in memberships.members {
        let mut uses = vec![selector.clone()];
        for source in member.sources {
            uses.push(origins.dataforge.at(
                SourceLocation::DataForge {
                    record_index: source.record_index,
                    field: "$guid".to_owned(),
                },
                SourceUseRole::Payload,
            )?);
            uses.push(origins.dataforge.at(
                SourceLocation::DataForge {
                    record_index: source.record_index,
                    field: format!("resources[{}]", source.position),
                },
                SourceUseRole::Selector,
            )?);
        }
        all_uses.extend(uses.iter().cloned());
        values.push(
            origins
                .uses
                .value(SourceValue::Present(member.group), &uses, true),
        );
    }
    Ok(origins.uses.value(
        SourceValue::Present(values),
        &all_uses,
        memberships.complete,
    ))
}
