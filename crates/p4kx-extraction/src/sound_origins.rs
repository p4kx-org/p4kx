// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use crate::origin_companion::write_selected_origin_companion;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, InputScope, OriginArtifactBinding, OriginOutputAddress,
    SourceEvidenceError, SourceInventory, SourceInventorySelection, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

use super::{Sound, SoundInput, WemOriginField, WemOriginPlan};

pub(super) struct SoundOrigins {
    inventory: SourceInventory,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    summary: FieldOrigins,
}

impl SoundOrigins {
    pub(super) fn new(
        uses: &SourceUses,
        fields: Vec<(OriginOutputAddress, FieldOrigin)>,
        summary: FieldOrigins,
    ) -> io::Result<Self> {
        Ok(Self {
            inventory: uses.freeze().map_err(io::Error::other)?,
            fields,
            summary,
        })
    }

    pub(super) const fn summary_origins(&self) -> &FieldOrigins {
        &self.summary
    }

    pub(super) fn summary_provenance(
        &self,
    ) -> Result<SourceInventorySelection<'_>, SourceEvidenceError> {
        self.inventory.select_uses(
            self.summary
                .iter()
                .flat_map(|(_, origin)| origin.source_uses_refs()),
        )
    }

    pub(super) fn write(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        let inventory = self
            .inventory
            .select_uses(
                self.fields
                    .iter()
                    .flat_map(|(_, origin)| origin.source_uses_refs()),
            )
            .map_err(io::Error::other)?;
        write_selected_origin_companion(
            writer,
            binding,
            &inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

pub(super) struct SoundEvidence<'uses, 'sound> {
    pub(super) uses: &'uses SourceUses,
    pub(super) payload: Option<SourceScope>,
    sound: &'sound Sound,
    plan: WemOriginPlan<'sound>,
}

impl<'uses, 'sound> SoundEvidence<'uses, 'sound> {
    pub(super) fn observe(sound: &'sound Sound, uses: &'uses SourceUses) -> io::Result<Self> {
        let payload = sound
            .source_input()
            .map(|input| uses.observe(input, InputScope::Payload))
            .transpose()
            .map_err(io::Error::other)?;
        let wem = sound.wem();
        Ok(Self {
            uses,
            payload,
            sound,
            plan: WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count()),
        })
    }

    pub(super) fn planned(&self, field: WemOriginField) -> io::Result<Vec<SourceUse>> {
        let Some(payload) = &self.payload else {
            return Ok(Vec::new());
        };
        self.plan
            .field(field)
            .map_err(io::Error::other)?
            .iter()
            .map(|site| site.bind(payload).map_err(io::Error::other))
            .collect()
    }

    pub(super) fn name_origin(&self) -> io::Result<FieldOrigin> {
        let (Some(payload), Some(input)) = (&self.payload, &self.sound.original) else {
            return Ok(self.uses.origin(&[], false));
        };
        let name_scope = match input {
            SoundInput::Archive(_) => InputScope::ArchiveDirectoryMetadata,
            SoundInput::File(_) => InputScope::FileMetadata,
        };
        let original = payload
            .for_scope(name_scope)
            .at(
                SourceLocation::Metadata {
                    field: "path".to_owned(),
                },
                SourceUseRole::Payload,
            )
            .map_err(io::Error::other)?;
        Ok(self.uses.origin(&[original], true))
    }

    pub(super) fn append_range(
        &self,
        uses: &mut Vec<SourceUse>,
        namespace: &str,
        field: &str,
        range: &Range<u64>,
        role: SourceUseRole,
    ) -> io::Result<()> {
        if let Some(payload) = &self.payload
            && !range.is_empty()
        {
            uses.push(range_use(payload, namespace, field, range, role)?);
        }
        Ok(())
    }

    pub(super) fn origin(&self, uses: &[SourceUse]) -> FieldOrigin {
        self.uses.origin(uses, self.payload.is_some())
    }

    pub(super) fn combine<'a>(
        &self,
        origins: impl IntoIterator<Item = &'a FieldOrigin>,
    ) -> io::Result<FieldOrigin> {
        self.uses
            .origin_with_origins(origins, true)
            .map_err(io::Error::other)
    }
}

pub(super) fn range_use(
    payload: &SourceScope,
    namespace: &str,
    field: &str,
    range: &Range<u64>,
    role: SourceUseRole,
) -> io::Result<SourceUse> {
    payload
        .at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: vec![range.start, range.end],
                field: field.into(),
            },
            role,
        )
        .map_err(io::Error::other)
}

pub(super) fn insert_summary(
    fields: &mut FieldOrigins,
    name: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let path = FieldPath::parse(name).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
