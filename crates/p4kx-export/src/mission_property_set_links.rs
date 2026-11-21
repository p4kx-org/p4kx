// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, StructValue, Value};

use crate::guid_text::CatalogGuid;
use crate::mission_property_fields as fields;
use crate::mission_property_set::MissionPropertyLevel;
use crate::mission_property_set_index::MissionPropertySetIndex;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::spawn_model;

/// Source-only property-set identities within one immutable original Generator.
pub(crate) struct MissionPropertySetLinks<'tree, 'source> {
    tree: &'tree RecordTree<'source>,
    index: MissionPropertySetIndex<'tree, 'source>,
}

impl<'tree, 'source> MissionPropertySetLinks<'tree, 'source> {
    pub(crate) fn new(tree: &'tree RecordTree<'source>) -> Result<Self, ReadError> {
        Ok(Self {
            tree,
            index: MissionPropertySetIndex::new(tree.root())?,
        })
    }

    /// Returns effective authored set identities without projecting property values.
    pub(crate) fn row(
        &self,
        handler: &'tree StructValue<'source>,
        contract: &'tree StructValue<'source>,
        sub: Option<&'tree StructValue<'source>>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<Vec<CatalogGuid>, ReadError> {
        row_scopes(handler, contract, sub, capture)?
            .into_iter()
            .filter_map(|scope| self.source(scope.parameters, capture).transpose())
            .map(|source| source.map(|source| source.guid))
            .collect()
    }

    pub(super) fn source(
        &self,
        parameters: Option<&'tree StructValue<'source>>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<Option<PropertySetSource<'tree, 'source>>, ReadError> {
        let Some(parameters) = parameters else {
            return Ok(None);
        };
        capture.check_parent(parameters)?;
        let Some(value) = parameters.field("propertyOverrides") else {
            return Ok(None);
        };
        if matches!(value, Value::Null) {
            return Ok(None);
        }
        let location = self
            .index
            .location(parameters)
            .ok_or("property source is outside its original Record")?;
        let location = if location.is_empty() {
            "propertyOverrides".to_owned()
        } else {
            format!("{location}.propertyOverrides")
        };
        let Value::Array(values) = value else {
            return Err(fields::malformed(
                parameters,
                "propertyOverrides",
                value,
                "an authored property array",
                capture,
            ));
        };
        Ok(Some(PropertySetSource {
            guid: identity(self.tree.guid(), &location)?,
            location,
            parameters,
            value,
            values,
        }))
    }
}

pub(super) struct PropertySetSource<'tree, 'source> {
    pub(super) guid: CatalogGuid,
    pub(super) location: String,
    pub(super) parameters: &'tree StructValue<'source>,
    pub(super) value: &'tree Value<'source>,
    pub(super) values: &'tree [Value<'source>],
}

pub(super) struct PropertySetScope<'tree, 'source> {
    pub(super) level: MissionPropertyLevel,
    pub(super) parameters: Option<&'tree StructValue<'source>>,
}

/// Selects the original authored levels for one parent or sub-contract row.
pub(super) fn row_scopes<'tree, 'source>(
    handler: &'tree StructValue<'source>,
    contract: &'tree StructValue<'source>,
    sub: Option<&'tree StructValue<'source>>,
    capture: &dyn FieldCapture<'source>,
) -> Result<[PropertySetScope<'tree, 'source>; 3], ReadError> {
    let handler = fields::parameter(handler, "contractParams", capture)?;
    let contract = fields::parameter(contract, "paramOverrides", capture)?;
    let sub = sub
        .map(|sub| {
            fields::parameter(sub, "paramOverrides", capture).map(|params| params.or(Some(sub)))
        })
        .transpose()?
        .flatten();
    Ok([
        PropertySetScope {
            level: MissionPropertyLevel::Handler,
            parameters: handler,
        },
        PropertySetScope {
            level: MissionPropertyLevel::Contract,
            parameters: contract,
        },
        PropertySetScope {
            level: MissionPropertyLevel::SubContract,
            parameters: sub,
        },
    ])
}

/// Identifies an authored set independently of mutable values and physical aliases.
pub(crate) fn identity(record: Guid, location: &str) -> Result<CatalogGuid, serde_json::Error> {
    spawn_model::content_guid(&("missionPropertySet", CatalogGuid(record), location))
}

#[cfg(test)]
#[path = "mission_property_set_links_tests.rs"]
mod tests;
