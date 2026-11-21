// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceUseRole, SourceUses,
};

use crate::audio::hirc_path_uses::{self, source_use};

use super::{HircArrival, HircInboundSource, HircPathSource, NodeKindEvidence};

/// Original node fields and the authored edge that selected a resolved path step.
#[derive(Debug)]
#[non_exhaustive]
pub struct HircPathOrigins {
    /// The object ID in its physical soundbank.
    pub id: FieldOrigin,
    /// The object type code, applicable mode byte and bank layout selector.
    pub kind: FieldOrigin,
    /// The preceding authored action, child or stinger selector.
    pub inbound: FieldOrigin,
}

impl HircPathSource {
    /// Registers the same-walk origins of the public path-step fields.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the observations.
    pub fn origins(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<HircPathOrigins, SourceEvidenceError> {
        let inbound = match &self.inbound {
            HircArrival::Start => uses.origin(&[], true),
            HircArrival::Link(Some(edge)) => edge.origin(archive, uses)?,
            HircArrival::Link(None) => uses.origin(&[], false),
        };
        let Some(entry) = self.entry(archive) else {
            return Ok(HircPathOrigins {
                id: uses.origin(&[], false),
                kind: uses.origin(&[], false),
                inbound,
            });
        };
        let scope = uses.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let complete = entry.ancestors().all(|ancestor| ancestor.is_some());
        let id = source_use(
            &scope,
            "bnkHircObject",
            "id",
            self.id.start as u64..self.id.end as u64,
            SourceUseRole::Payload,
        )?;
        let mut kind = vec![source_use(
            &scope,
            "bnkHircObject",
            "typeCode",
            self.kind_code.start as u64..self.kind_code.end as u64,
            SourceUseRole::Conversion,
        )?];
        let version = hirc_path_uses::version_use(&scope, self.version_bytes(), &mut kind)?;
        let mode = match &self.kind_evidence {
            NodeKindEvidence::TypeCode => true,
            NodeKindEvidence::RandomSequence(Some(mode)) => {
                kind.push(source_use(
                    &scope,
                    "bnkHircRandomSequence",
                    "randomSequenceMode",
                    mode.clone(),
                    SourceUseRole::Conversion,
                )?);
                true
            }
            NodeKindEvidence::RandomSequence(None) => false,
        };
        Ok(HircPathOrigins {
            id: uses.origin(&[id], complete),
            kind: uses.origin(&kind, complete && version && mode),
            inbound,
        })
    }
}

impl HircInboundSource {
    pub(in crate::audio) fn origin(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut actual = Vec::new();
        let complete = match self {
            Self::Child(child) => hirc_path_uses::child(archive, uses, child, &mut actual)?,
            Self::Route(route) => hirc_path_uses::route(archive, uses, route, &mut actual)?,
            Self::Stinger { action, schedule } => {
                let action = hirc_path_uses::route(archive, uses, action, &mut actual)?;
                let schedule = match schedule {
                    Some(row) => hirc_path_uses::route(archive, uses, row, &mut actual)?,
                    None => false,
                };
                action && schedule
            }
        };
        Ok(uses.origin(&actual, complete))
    }
}
