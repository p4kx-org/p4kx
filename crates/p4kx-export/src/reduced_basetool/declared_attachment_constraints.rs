// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::declared_equipment::{ComponentCoordinate, Owner};
use super::number::Number;
use super::port_restriction_value::{self, Restriction};
use crate::catalog_queries::equipment::{ItemAttachment, ItemAttachmentComponent};

pub(super) const PATH: &str = "configurations.itemAttachmentConstraints";

#[derive(Serialize)]
pub(super) struct AttachmentConstraint<'read, 'source> {
    #[serde(flatten)]
    owner: ComponentCoordinate<'read>,
    definition: Cell<Definition<'read, 'source>>,
}

impl<'read, 'source> AttachmentConstraint<'read, 'source> {
    pub(super) fn read(
        source: &'read ItemAttachmentComponent<'source>,
        owner: &Owner<'_>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            owner: owner.component(
                PATH,
                &source.owner_guid(),
                source.owner_record_index(),
                source.component_ordinal(),
            )?,
            definition: completeness::cell(PATH, source.definition())?.try_map(Definition::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Definition<'read, 'source> {
    item_type: Cell<Restriction<'read, 'source, &'source str>>,
    item_subtype: Cell<Restriction<'read, 'source, &'source str>>,
    size: Cell<Restriction<'read, 'source, Number<'read>>>,
    tags: Cell<&'source str>,
    required_tags: Cell<&'source str>,
}

impl<'read, 'source> Definition<'read, 'source> {
    fn read(source: &'read ItemAttachment<'source>) -> Result<Self, ReducedError> {
        Ok(Self {
            item_type: port_restriction_value::label(PATH, source.item_type())?,
            item_subtype: port_restriction_value::label(PATH, source.item_subtype())?,
            size: port_restriction_value::number(PATH, source.size())?,
            tags: completeness::cell(PATH, source.tags())?.map(|value| *value),
            required_tags: completeness::cell(PATH, source.required_tags())?.map(|value| *value),
        })
    }
}
