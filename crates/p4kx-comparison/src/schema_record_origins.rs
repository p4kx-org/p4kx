// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_extraction::{FieldOrigin, SharedFieldOrigin, SourceEvidenceError, SourceUse, SourceUses};

use crate::schema_origin_aggregates;
use crate::schema_report_origins::SchemaSource;
use crate::schema_types::SchemaDiff;

#[derive(Clone, Copy)]
pub(crate) enum RecordFamily {
    Struct,
    Enum,
}

pub(crate) struct RecordOrigins {
    structures: Option<SharedFieldOrigin>,
    enumerations: Option<SharedFieldOrigin>,
}

impl RecordOrigins {
    pub(crate) fn capture(
        uses: &SourceUses,
        side: SchemaSource<'_, '_>,
        diff: &SchemaDiff,
    ) -> Result<Self, SourceEvidenceError> {
        let capture = |enumeration| -> Result<SharedFieldOrigin, SourceEvidenceError> {
            let operands = schema_origin_aggregates::record_sites(side, enumeration)?;
            Ok(uses.shared_origin(&operands, side.survey_complete()))
        };
        Ok(Self {
            structures: (!diff.structs.is_empty())
                .then(|| capture(false))
                .transpose()?,
            enumerations: (!diff.enums.is_empty())
                .then(|| capture(true))
                .transpose()?,
        })
    }

    pub(crate) fn origin(
        &self,
        uses: &SourceUses,
        family: RecordFamily,
        name: &SourceUse,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let shared = match family {
            RecordFamily::Struct => self.structures.as_ref(),
            RecordFamily::Enum => self.enumerations.as_ref(),
        }
        .ok_or(SourceEvidenceError::ForeignEvidence)?;
        uses.origin_with_shared(shared, slice::from_ref(name), true)
    }
}
