// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_v3::location_entity_declaration;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_members::TemplateMembers;

/// The authored pickup and drop-off fields of one physical hauling order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct HaulingTargetBinding {
    order: SourceValueRef,
    pick_up_target_types: Option<SourceValueRef>,
    drop_off_target_types: Option<SourceValueRef>,
}

impl HaulingTargetBinding {
    /// Returns the order occurrence in its owner's original value inventory.
    #[must_use]
    pub const fn order(&self) -> SourceValueRef {
        self.order
    }

    /// Returns the original pickup Reference array, including an empty or unreadable declaration.
    #[must_use]
    pub const fn pick_up_target_types(&self) -> Option<SourceValueRef> {
        self.pick_up_target_types
    }

    /// Returns the original drop-off Reference array, including an empty or unreadable declaration.
    #[must_use]
    pub const fn drop_off_target_types(&self) -> Option<SourceValueRef> {
        self.drop_off_target_types
    }
}

pub(crate) fn read_template(
    members: &TemplateMembers<'_, '_, '_, '_>,
) -> Result<Vec<HaulingTargetBinding>, CatalogError> {
    read(members.fields(), members.values(), |fact| {
        members.value_ref(fact)
    })
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    values: impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>>,
    mut value_ref: impl FnMut(
        &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceValueRef, CatalogError>,
) -> Result<Vec<HaulingTargetBinding>, CatalogError> {
    let mut rows = Vec::new();
    for order in values {
        let Value::Struct(structure) = order.value() else {
            continue;
        };
        if structure.field("pickUpTargetTypes").is_none()
            && structure.field("dropOffTargetTypes").is_none()
        {
            continue;
        }
        let pickup = declared_array(fields, order, "pickUpTargetTypes")?;
        let dropoff = declared_array(fields, order, "dropOffTargetTypes")?;
        if pickup.is_some() || dropoff.is_some() {
            rows.push(HaulingTargetBinding {
                order: value_ref(order)?,
                pick_up_target_types: pickup.map(&mut value_ref).transpose()?,
                drop_off_target_types: dropoff.map(&mut value_ref).transpose()?,
            });
        }
    }
    Ok(rows)
}

fn declared_array<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    order: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
    let Some((_, field)) = fields.field(Some(order), name)? else {
        return Ok(None);
    };
    let storage = field.field().storage();
    let declared = storage.data_type() == DataType::Reference
        && storage.is_array()
        && fields.declared_target_name(field)? == Some(location_entity_declaration::SOURCE_TYPE);
    Ok(declared.then_some(field))
}
