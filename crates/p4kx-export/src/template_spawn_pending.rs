// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::CatalogError;
use crate::catalog_read::original::OriginalValueFact;
use crate::catalog_v3::tag_catalog::dependent_origins::TagInputs;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::guid_text::CatalogGuid;
use crate::record_read::ReadError;
use crate::spawn_pools::{Registered, Target};
use crate::template_source_members::TemplateMembers;
use crate::template_spawn_projection::{TemplateSpawnProjection, TemplateSpawnResult};
use p4kx_extraction::{SourceValue, Value};

/// Only the actual content read can construct a pending emitted result.
pub(super) struct PendingSpawn<'fields, 'scope, 'read, 'source> {
    source: &'fields OriginalValueFact<'scope, 'read, 'source>,
    inputs: TagInputs,
    result: Result<Registered, ReadError>,
}

impl<'fields, 'scope, 'read, 'source> PendingSpawn<'fields, 'scope, 'read, 'source> {
    pub(super) const fn read(
        source: &'fields OriginalValueFact<'scope, 'read, 'source>,
        inputs: TagInputs,
        result: Result<Registered, ReadError>,
    ) -> Self {
        Self {
            source,
            inputs,
            result,
        }
    }

    pub(super) fn required(self) -> Result<Self, ReadError> {
        match self.result {
            Ok(_) => Ok(self),
            Err(error) => Err(error),
        }
    }

    pub(super) fn emit(
        self,
        members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    ) -> Result<(TemplateSpawnProjection, Option<(u64, ReadError)>), CatalogError> {
        let source = members.value_ref(self.source)?;
        let sources = members.fields().sources();
        let (result, failure) = match self.result {
            Ok(Registered { target, empty }) => {
                let bind = |guid| self.inputs.bind(SourceValue::Present(guid), sources);
                let result = match target {
                    Target::Ships(guid) => TemplateSpawnResult::Ships {
                        pool: bind(guid)?,
                        empty,
                    },
                    Target::Npcs(guid) => TemplateSpawnResult::Npcs {
                        pool: bind(guid)?,
                        empty,
                    },
                    Target::Entities(guid) => TemplateSpawnResult::Entities {
                        pool: bind(guid)?,
                        empty,
                    },
                    Target::Location(guid) => TemplateSpawnResult::Location { query: bind(guid)? },
                };
                (result, None)
            }
            Err(error) => {
                let state: SourceValue<CatalogGuid> =
                    if crate::export_source_failure::diagnosis(error.as_ref()).code
                        == Some(ExportDiagnosticCode::UnsupportedValue)
                    {
                        SourceValue::Unsupported
                    } else {
                        SourceValue::Unreadable
                    };
                let mut inputs = self.inputs;
                inputs.retain([], false);
                let identifier = inputs.bind(state, sources)?;
                (TemplateSpawnResult::Unavailable { identifier }, Some(error))
            }
        };
        Ok((
            TemplateSpawnProjection { source, result },
            failure.map(|error| (self.source.ordinal(), error)),
        ))
    }

    pub(super) fn is_spawn(value: &Value<'_>) -> bool {
        matches!(value, Value::Struct(source) if
            crate::template_property_kind::PropertyValueKind::read(source.type_name())
                .is_some_and(crate::template_property_kind::PropertyValueKind::is_spawn))
    }
}
