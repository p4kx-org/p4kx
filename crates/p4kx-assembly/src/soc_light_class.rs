// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForgeError, Guid, Record, RecordTree, StructValue, TreeBuilder, Value};
use p4kx_extraction::cryxml::SourceAttribute;
use p4kx_model::SocEntity;

use crate::entities::{self, Entities};
use crate::soc_source_guid::SocSourceGuid;

const LIGHT_COMPONENT: &str = "LightComponentParams";

/// The original Entity class selection for one SOC light.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocLightClass<'source> {
    /// No original class selector is available.
    Absent,
    /// A stored selector is invalid or repeated.
    Invalid,
    /// The selector has no matching original Record.
    Unmatched,
    /// Several physical Records match the selector.
    Ambiguous(Vec<Record<'source>>),
    /// The selected original Record has a different declaration.
    UnexpectedRecordType(Record<'source>),
    /// The selected original Entity class.
    Matched(Record<'source>),
}

impl<'source> SocLightClass<'source> {
    /// Resolves the authored GUID or, when absent, the exact authored class name.
    #[must_use]
    pub fn resolve(entity: SocEntity<'_>, entities: &Entities<'source>) -> Self {
        Self::resolve_selector(SocLightClassSelector::read(entity), entities)
    }

    /// Resolves a physical class selector and retains every duplicate candidate.
    #[must_use]
    pub fn resolve_selector(
        selector: SocLightClassSelector<'_>,
        entities: &Entities<'source>,
    ) -> Self {
        let matches = match selector {
            SocLightClassSelector::Guid(guid) => entities.forge().records_by_guid(guid).collect(),
            SocLightClassSelector::Name(name) => entities.named_class_candidates(name).to_vec(),
            SocLightClassSelector::Absent => return Self::Absent,
            SocLightClassSelector::Invalid => return Self::Invalid,
        };
        match matches.as_slice() {
            [] => Self::Unmatched,
            [record] if entities::is_entity(record) => Self::Matched(*record),
            [record] => Self::UnexpectedRecordType(*record),
            _ => Self::Ambiguous(matches),
        }
    }
}

/// The exact authored selector of an SOC light's class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SocLightClassSelector<'document> {
    /// The entity gives no selector.
    Absent,
    /// The deciding selector is malformed or repeated.
    Invalid,
    /// A nonzero original class GUID.
    Guid(Guid),
    /// The original class name used in the absence of a nonzero GUID.
    Name(&'document str),
}

impl<'document> SocLightClassSelector<'document> {
    /// Reads the deciding original class selector.
    #[must_use]
    pub fn read(entity: SocEntity<'document>) -> Self {
        match SocSourceGuid::read(entity.class_guid_attributes()) {
            SocSourceGuid::Present(guid) if guid != Guid::default() => Self::Guid(guid),
            SocSourceGuid::Invalid => Self::Invalid,
            _ => Self::name(entity.class_name_attributes()),
        }
    }

    fn name(mut attributes: impl Iterator<Item = SourceAttribute<'document>>) -> Self {
        match (attributes.next(), attributes.next()) {
            (None, _) => Self::Absent,
            (Some(attribute), None) if !attribute.value().trim().is_empty() => {
                Self::Name(attribute.value())
            }
            _ => Self::Invalid,
        }
    }
}

/// The physical Entity Record tree behind a light class selection.
#[derive(Debug)]
pub struct SocLightClassSource<'source> {
    tree: RecordTree<'source>,
}

impl<'source> SocLightClassSource<'source> {
    /// Reads the selected Record without following links into another Record.
    ///
    /// # Errors
    /// Returns an error if the original Entity Record cannot be read.
    pub fn read(
        record: &Record<'source>,
        trees: &mut TreeBuilder<'source>,
    ) -> Result<Self, DataForgeError> {
        Ok(Self {
            tree: trees.build(record)?,
        })
    }

    /// Returns the original Record tree and its physical identity.
    #[must_use]
    pub const fn tree(&self) -> &RecordTree<'source> {
        &self.tree
    }

    /// Visits every concrete light component and its original Components ordinal.
    pub fn components(&self) -> impl Iterator<Item = (usize, &StructValue<'source>)> {
        let slots: &[Value<'source>] = match self.tree.field("Components") {
            Some(Value::Array(slots)) => slots,
            _ => &[],
        };
        slots.iter().enumerate().filter_map(|(ordinal, value)| {
            let Value::Struct(component) = value else {
                return None;
            };
            (component.type_name() == LIGHT_COMPONENT).then_some((ordinal, component))
        })
    }
}
