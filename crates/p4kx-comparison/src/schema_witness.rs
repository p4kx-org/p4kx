// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_dataforge::{DataForge, DataType, Enum, Property, Struct};
use p4kx_extraction::{
    DataForgeOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

const STRUCTS: &str = "dataforgeStructDefinition";
const ENUMS: &str = "dataforgeEnumDefinition";
const PROPERTIES: &str = "dataforgePropertyDefinition";

pub(crate) type Sites = Vec<SourceUse>;

pub(crate) struct SchemaWitness {
    scope: SourceScope,
    pub(crate) complete: bool,
    structs: u64,
    enums: u64,
    properties: u64,
    records: u64,
    mappings: u64,
    mapping_indices: BTreeMap<u32, u32>,
}

impl SchemaWitness {
    pub(crate) fn new(
        uses: &SourceUses,
        forge: &DataForge<'_>,
        origin: Option<&DataForgeOrigin>,
    ) -> Result<Self, SourceEvidenceError> {
        let (source, complete) = match origin {
            Some(DataForgeOrigin::Archive(entry)) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            _ => (SourceInput::Unknown, false),
        };
        Ok(Self {
            scope: uses.observe(source, InputScope::Payload)?,
            complete,
            structs: forge.struct_count() as u64,
            enums: forge.enum_count() as u64,
            properties: forge.property_count() as u64,
            records: forge.record_count() as u64,
            mappings: forge.data_mapping_count() as u64,
            mapping_indices: forge
                .data_mappings()
                .map(|row| (row.struct_index, row.index))
                .collect(),
        })
    }

    fn at(
        &self,
        namespace: &str,
        field: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        )
    }

    pub(crate) fn struct_name(&self, index: u32) -> Result<SourceUse, SourceEvidenceError> {
        self.at(
            STRUCTS,
            "name",
            vec![u64::from(index)],
            SourceUseRole::Payload,
        )
    }

    pub(crate) fn enum_name(&self, index: u32) -> Result<SourceUse, SourceEvidenceError> {
        self.at(
            ENUMS,
            "name",
            vec![u64::from(index)],
            SourceUseRole::Payload,
        )
    }

    pub(crate) fn option_name(&self, index: usize) -> Result<SourceUse, SourceEvidenceError> {
        self.at(
            "dataforgeEnumOption",
            "name",
            vec![index as u64],
            SourceUseRole::Payload,
        )
    }

    fn selector(
        &self,
        namespace: &str,
        field: &str,
        indices: Vec<u64>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.at(namespace, field, indices, SourceUseRole::Selector)
    }

    pub(crate) fn struct_names(&self) -> Result<Sites, SourceEvidenceError> {
        self.definition_names(STRUCTS, self.structs)
    }

    pub(crate) fn enum_names(&self) -> Result<Sites, SourceEvidenceError> {
        self.definition_names(ENUMS, self.enums)
    }

    fn definition_names(&self, namespace: &str, count: u64) -> Result<Sites, SourceEvidenceError> {
        let mut sites = vec![self.selector(namespace, "definitionCount", Vec::new())?];
        if count != 0 {
            sites.push(self.selector(namespace, "definitionNames", vec![0, count])?);
        }
        Ok(sites)
    }

    pub(crate) fn parent(&self, owner: Struct<'_>) -> Result<Sites, SourceEvidenceError> {
        let mut sites = vec![self.selector(STRUCTS, "parent", vec![u64::from(owner.index())])?];
        if let Some(parent) = owner.parent().filter(|parent| parent.name().is_ok()) {
            sites.push(self.struct_name(parent.index())?);
        }
        Ok(sites)
    }

    pub(crate) fn parent_complete(&self, owner: Struct<'_>) -> bool {
        self.complete && owner.parent().is_none_or(|parent| parent.name().is_ok())
    }

    pub(crate) fn property_names(&self, owner: Struct<'_>) -> Result<Sites, SourceEvidenceError> {
        let range = owner.property_range();
        let bounds = [range.start as u64, range.end as u64];
        let mut sites = vec![self.selector(
            STRUCTS,
            "propertyRange",
            vec![u64::from(owner.index()), bounds[0], bounds[1]],
        )?];
        if !range.is_empty() {
            sites.push(self.selector(
                "dataforgePropertyDefinitionRange",
                "name",
                bounds.to_vec(),
            )?);
        }
        Ok(sites)
    }

    pub(crate) fn option_names(&self, owner: Enum<'_>) -> Result<Sites, SourceEvidenceError> {
        let range = owner.option_range();
        let mut sites = vec![self.selector(
            ENUMS,
            "optionRange",
            vec![
                u64::from(owner.index()),
                range.start as u64,
                range.end as u64,
            ],
        )?];
        if !range.is_empty() {
            sites.push(self.selector(
                "dataforgeEnumOptionRange",
                "name",
                vec![range.start as u64, range.end as u64],
            )?);
        }
        Ok(sites)
    }

    pub(crate) fn property_name(
        &self,
        property: Property<'_>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.property_word(property, "name", SourceUseRole::Payload)
    }

    fn property_word(
        &self,
        property: Property<'_>,
        name: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.at(PROPERTIES, name, vec![property.index() as u64], role)
    }

    pub(crate) fn data_type(
        &self,
        property: Property<'_>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.property_word(property, "dataType", SourceUseRole::Conversion)
    }

    pub(crate) fn conversion(
        &self,
        property: Property<'_>,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.property_word(property, "conversion", SourceUseRole::Conversion)
    }

    pub(crate) fn target(&self, property: Property<'_>) -> Result<Sites, SourceEvidenceError> {
        let mut sites = vec![self.property_word(property, "dataType", SourceUseRole::Selector)?];
        let namespace = match property.data_type() {
            Ok(DataType::Enum) => ENUMS,
            Ok(
                DataType::Class
                | DataType::Reference
                | DataType::StrongPointer
                | DataType::WeakPointer,
            ) => STRUCTS,
            _ => return Ok(sites),
        };
        sites.push(self.property_word(property, "target", SourceUseRole::Selector)?);
        if let Some(target) = property.target_struct() {
            if target.name().is_ok() {
                sites.push(self.struct_name(target.index())?);
            }
        } else if let Some(target) = property.target_enum() {
            if target.name().is_ok() {
                sites.push(self.enum_name(target.index())?);
            }
        } else {
            sites.push(self.selector(namespace, "definitionCount", Vec::new())?);
        }
        Ok(sites)
    }

    pub(crate) fn target_complete(&self, property: Property<'_>) -> bool {
        self.complete
            && property
                .target_struct()
                .is_none_or(|target| target.name().is_ok())
            && property
                .target_enum()
                .is_none_or(|target| target.name().is_ok())
    }

    pub(crate) fn mapped_instances(&self, index: u32) -> Result<Sites, SourceEvidenceError> {
        if let Some(&mapping) = self.mapping_indices.get(&index) {
            let mut sites = vec![self.selector(
                "dataforgeDataMapping",
                "structIndex",
                vec![u64::from(mapping)],
            )?];
            sites.push(self.at(
                "dataforgeDataMapping",
                "instanceCount",
                vec![u64::from(mapping)],
                SourceUseRole::Payload,
            )?);
            return Ok(sites);
        }
        let mut sites = vec![self.header("dataMappingCount")?];
        if self.mappings != 0 {
            sites.push(self.selector(
                "dataforgeDataMappingRange",
                "structIndex",
                vec![0, self.mappings],
            )?);
        }
        Ok(sites)
    }

    fn header(&self, field: &str) -> Result<SourceUse, SourceEvidenceError> {
        self.selector("dataforgeHeader", field, Vec::new())
    }

    pub(crate) fn record_types(&self) -> Result<Sites, SourceEvidenceError> {
        let mut uses = vec![self.header("recordCount")?];
        if self.records != 0 {
            uses.push(self.selector(
                "dataforgeRecordHeaderRange",
                "sourceType",
                vec![0, self.records],
            )?);
        }
        Ok(uses)
    }

    pub(crate) fn structure_headers(&self, field: &str) -> Result<SourceUse, SourceEvidenceError> {
        self.selector(
            "dataforgeStructDefinitionRange",
            field,
            vec![0, self.structs],
        )
    }

    pub(crate) fn enum_headers(&self, field: &str) -> Result<SourceUse, SourceEvidenceError> {
        self.selector("dataforgeEnumDefinitionRange", field, vec![0, self.enums])
    }

    pub(crate) fn property_headers(&self, field: &str) -> Result<SourceUse, SourceEvidenceError> {
        self.selector(
            "dataforgePropertyDefinitionRange",
            field,
            vec![0, self.properties],
        )
    }
}
