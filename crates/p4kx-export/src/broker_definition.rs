// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{PhysicalSourceRef, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_rows::mission_definitions::DefinitionSource;
use crate::catalog_template_modules::mission_owner_definition::MissionOwnerDefinition;
use crate::schema_fields::every_field_required;
use crate::template_configuration::{MissionFlowRef, MissionPropertyRef};
use crate::template_meanings::FlowMeanings;
use crate::template_number_capture::TemplateNumbers;
use crate::template_reference_index::TemplateReferences;
use crate::template_source::TemplateSource;

/// One physical Broker owner and its single complete original-value inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BrokerDefinition<'source> {
    input_ref: PhysicalSourceRef,
    #[serde(flatten)]
    header: CanonicalRecordHeader<'source>,
    source: TemplateSource<'source>,
    mission_flows: Vec<MissionFlowRef>,
    mission_properties: Vec<MissionPropertyRef>,
}

impl<'source> BrokerDefinition<'source> {
    /// Borrows the original DataForge source identity.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Borrows the physical Broker header with its field origins.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the original source arena shared by the declaration links.
    #[must_use]
    pub const fn source(&self) -> &TemplateSource<'source> {
        &self.source
    }

    /// Borrows the links to this owner's canonical flow declarations.
    #[must_use]
    pub fn mission_flows(&self) -> &[MissionFlowRef] {
        &self.mission_flows
    }

    /// Borrows the links to this owner's canonical property declarations.
    #[must_use]
    pub fn mission_properties(&self) -> &[MissionPropertyRef] {
        &self.mission_properties
    }

    pub(crate) fn emit<'fields, 'scope, 'read>(
        input: &OriginalCatalogInput<'_, '_>,
        tree: &RecordTree<'source>,
        references: &mut TemplateReferences<'fields, 'scope, 'read, 'source>,
        meanings: FlowMeanings<'_, 'fields, 'scope, 'read, 'source>,
        links: (Vec<MissionFlowRef>, Vec<MissionPropertyRef>),
    ) -> Result<(Self, DefinitionSource<'source>), CatalogError> {
        let members = meanings.members();
        let fields = members.fields();
        fields.check_tree(tree)?;
        let source = meanings.emit(references)?;
        let proof = DefinitionSource::new(tree, TemplateNumbers::broker(members, input)?);
        let definition = MissionOwnerDefinition::read(input, fields, tree, source, links)?;
        Ok((Self::from(definition), proof))
    }
}

impl<'source> From<MissionOwnerDefinition<'source>> for BrokerDefinition<'source> {
    fn from(definition: MissionOwnerDefinition<'source>) -> Self {
        let MissionOwnerDefinition {
            input_ref,
            header,
            source,
            mission_flows,
            mission_properties,
        } = definition;
        Self {
            input_ref,
            header,
            source,
            mission_flows,
            mission_properties,
        }
    }
}
