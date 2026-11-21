// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    DataForge, DataForgeHeaderField, DataForgeOrigin, FieldOrigin, InputScope,
    SourceCoordinateOmission, SourceEvidenceError, SourceInput, SourceInventory, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use SourceUseRole::{Conversion, Payload, Selector};

/// Original inputs of the DataForge summary facts and previewed Record identities.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataForgeSummaryProvenance {
    /// The version in the DataForge header.
    pub version: FieldOrigin,
    /// The declared Record count.
    pub record_count: FieldOrigin,
    /// The declared struct count.
    pub struct_count: FieldOrigin,
    /// The declared enum count.
    pub enum_count: FieldOrigin,
    rows: Vec<DataForgeSummaryRecordProvenance>,
    inventory: SourceInventory,
}

impl DataForgeSummaryProvenance {
    /// Returns the origins in the same definition order as the displayed Records.
    #[must_use]
    pub fn rows(&self) -> &[DataForgeSummaryRecordProvenance] {
        &self.rows
    }

    /// Returns the physical input and exact original coordinates.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

/// Original inputs of one identity in the DataForge summary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataForgeSummaryRecordProvenance {
    /// The Record name or the GUID used when its name is unreadable.
    pub name: FieldOrigin,
    /// The type name resolved through the original struct definition.
    pub type_name: FieldOrigin,
    /// The canonical record path derived from its stored filename.
    pub record_path: FieldOrigin,
}

pub(crate) struct ReadIdentity<'f> {
    pub(crate) index: u32,
    pub(crate) name: Option<&'f str>,
    pub(crate) struct_index: Option<u32>,
    pub(crate) type_name: Option<&'f str>,
    pub(crate) path: Option<&'f str>,
    pub(crate) file_name: Option<&'f str>,
}

pub(crate) struct Capture {
    uses: SourceUses,
    scope: SourceScope,
    complete: bool,
    rows: Vec<DataForgeSummaryRecordProvenance>,
}

impl Capture {
    pub(crate) fn new(source: Option<&DataForgeOrigin>) -> Result<Self, SourceEvidenceError> {
        let (input, complete) = match source {
            Some(DataForgeOrigin::Archive(entry)) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            Some(DataForgeOrigin::File(file)) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let uses = SourceUses::new();
        let scope = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            complete,
            rows: Vec::new(),
        })
    }

    pub(crate) fn row(
        &mut self,
        forge: &DataForge<'_>,
        identity: &ReadIdentity<'_>,
    ) -> Result<(), SourceEvidenceError> {
        let name = self.name(forge, identity)?;
        let type_name = self.type_name(forge, identity)?;
        let record_path = self.record_path(forge, identity)?;
        self.rows.push(DataForgeSummaryRecordProvenance {
            name,
            type_name,
            record_path,
        });
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<DataForgeSummaryProvenance, SourceEvidenceError> {
        use DataForgeHeaderField::{EnumCount, RecordCount, StructCount, Version};

        Ok(DataForgeSummaryProvenance {
            version: self.header(Version, "version")?,
            record_count: self.header(RecordCount, "recordCount")?,
            struct_count: self.header(StructCount, "structCount")?,
            enum_count: self.header(EnumCount, "enumCount")?,
            rows: self.rows,
            inventory: self.uses.freeze()?,
        })
    }

    fn header(
        &self,
        field: DataForgeHeaderField,
        label: &str,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let used = self.site("dataforgeHeader", label, field.byte_span(), Payload)?;
        Ok(self.uses.origin(&[used], self.complete))
    }

    fn name(
        &self,
        forge: &DataForge<'_>,
        row: &ReadIdentity<'_>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut inputs = vec![self.definition(row.index, "recordName", Selector)?];
        match row.name {
            None => inputs.push(self.definition(row.index, "guid", Conversion)?),
            Some(name) => {
                let span = forge.original_text_span(name);
                if let Some(span) = span {
                    inputs.push(self.site("dataforgeStringBytes", "recordName", span, Payload)?);
                } else if !name.is_empty() {
                    inputs.push(self.unknown_string(Payload)?);
                }
            }
        }
        Ok(self.uses.origin(&inputs, self.complete))
    }

    fn type_name(
        &self,
        forge: &DataForge<'_>,
        row: &ReadIdentity<'_>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut inputs = vec![self.definition(row.index, "sourceType", Selector)?];
        if let Some(index) = row.struct_index {
            inputs.push(self.scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeStructDefinition".into(),
                    indices: vec![u64::from(index)],
                    field: "name".into(),
                },
                Selector,
            )?);
            if let Some(name) = row.type_name {
                inputs.push(self.text(forge, "typeName", name, Payload)?);
            }
        }
        Ok(self.uses.origin(&inputs, self.complete))
    }

    fn record_path(
        &self,
        forge: &DataForge<'_>,
        row: &ReadIdentity<'_>,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut inputs = vec![self.definition(row.index, "fileName", Selector)?];
        if let Some(path) = row.path {
            if let Some(stored) = row.file_name {
                inputs.push(self.text(forge, "fileName", stored, Payload)?);
                if stored != path {
                    inputs.push(self.text(forge, "fileName", stored, Conversion)?);
                }
            } else {
                inputs.push(self.unknown_string(Payload)?);
            }
        }
        Ok(self.uses.origin(&inputs, self.complete))
    }

    fn definition(
        &self,
        record_index: u32,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::DataForge {
                record_index,
                field: field.into(),
            },
            role,
        )
    }

    fn text(
        &self,
        forge: &DataForge<'_>,
        field: &str,
        text: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        forge.original_text_span(text).map_or_else(
            || self.unknown_string(role),
            |span| self.site("dataforgeStringBytes", field, span, role),
        )
    }

    fn unknown_string(&self, role: SourceUseRole) -> Result<SourceUse, SourceEvidenceError> {
        self.scope
            .at_opaque(role, SourceCoordinateOmission::Unavailable)
    }

    fn site(
        &self,
        namespace: &str,
        field: &str,
        span: Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: vec![span.start, span.end],
                field: field.into(),
            },
            role,
        )
    }
}
