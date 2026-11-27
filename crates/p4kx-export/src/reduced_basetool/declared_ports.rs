// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::declared_equipment::{ComponentCoordinate, Owner};
use super::number::Number;
use super::port_restriction_value::{self, Original, Restriction};
use super::selected_texts::TextRefs;
use crate::catalog_text_contract::PublicNameRef;
use crate::catalog_v3::catalog_item_alignment::{
    ItemPort, ItemPortContainer, PortItemType, PortRestrictions,
};

pub(super) const PATH: &str = "configurations.itemPorts";

#[derive(Serialize)]
pub(super) struct PortContainer<'read, 'source> {
    #[serde(flatten)]
    owner: ComponentCoordinate<'read>,
    ports: Cell<Vec<Cell<DeclaredPort<'read, 'source>>>>,
}

impl<'read, 'source> PortContainer<'read, 'source> {
    pub(super) fn read(
        source: &'read ItemPortContainer<'source>,
        owner: &Owner<'_>,
        texts: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            owner: owner.component(
                PATH,
                &source.owner_guid(),
                source.owner_record_index(),
                source.component_ordinal(),
            )?,
            ports: completeness::list(PATH, source.ports(), |port| {
                DeclaredPort::read(port, texts)
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredPort<'read, 'source> {
    port_ordinal: Cell<&'read u32>,
    name: Cell<&'source str>,
    display_name: Cell<&'read str>,
    restrictions: Restrictions<'read, 'source>,
}

impl<'read, 'source> DeclaredPort<'read, 'source> {
    fn read(
        source: &'read ItemPort<'source>,
        texts: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let display_name = completeness::cell(PATH, source.display_name())?;
        texts.name(display_name.value);
        Ok(Self {
            port_ordinal: completeness::cell(PATH, source.port_ordinal())?,
            name: completeness::cell(PATH, source.name())?.map(|name| *name),
            display_name: display_name.map(PublicNameRef::as_str),
            restrictions: Restrictions::read(source.restrictions())?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Restrictions<'read, 'source> {
    types: Cell<Vec<Cell<Type<'read, 'source>>>>,
    min_size: Cell<Restriction<'read, 'source, Number<'read>>>,
    max_size: Cell<Restriction<'read, 'source, Number<'read>>>,
    flags: Cell<&'source str>,
    port_tags: Cell<&'source str>,
    required_port_tags: Cell<&'source str>,
}

impl<'read, 'source> Restrictions<'read, 'source> {
    fn read(source: &'read PortRestrictions<'source>) -> Result<Self, ReducedError> {
        Ok(Self {
            types: completeness::list(PATH, source.types(), Type::read)?,
            min_size: port_restriction_value::number(PATH, source.min_size())?,
            max_size: port_restriction_value::number(PATH, source.max_size())?,
            flags: completeness::cell(PATH, source.flags())?.map(|value| *value),
            port_tags: completeness::cell(PATH, source.port_tags())?.map(|value| *value),
            required_port_tags: completeness::cell(PATH, source.required_port_tags())?
                .map(|value| *value),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Type<'read, 'source> {
    r#type: Cell<Restriction<'read, 'source, &'source str>>,
    sub_types: Cell<Vec<Cell<Original<'read, 'source>>>>,
}

impl<'read, 'source> Type<'read, 'source> {
    fn read(source: &'read PortItemType<'source>) -> Result<Self, ReducedError> {
        Ok(Self {
            r#type: port_restriction_value::label(PATH, source.item_type())?,
            sub_types: port_restriction_value::subtypes(PATH, source.sub_types())?,
        })
    }
}
