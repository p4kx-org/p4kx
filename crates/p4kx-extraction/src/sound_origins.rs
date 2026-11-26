// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_audio::Wem;

use crate::origin_companion::write_selected_origin_companion;
use crate::{
    FieldOrigin, FieldOrigins, FieldPath, OriginArtifactBinding, OriginOutputAddress,
    SourceEvidenceError, SourceInventory, SourceInventorySelection, SourceUse, SourceUseRole,
    SourceUses,
};

use super::{WemOriginField, WemOriginPlan};

#[path = "sound_origin_input.rs"]
mod input;
pub use input::SoundOriginInput;
pub(super) use input::SoundPayload;

pub(crate) struct SoundOrigins {
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

    pub(crate) fn write(
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

pub(crate) fn capture_raw_sound(
    input: Option<SoundOriginInput<'_>>,
    length: u64,
) -> io::Result<SoundOrigins> {
    let uses = SourceUses::new();
    let origin = match input {
        Some(input) if !matches!(input.source(), crate::SourceInput::Unknown) => {
            SoundPayload::observe(input, &uses)?.raw_origin(&uses, input.source(), length)?
        }
        _ => uses.origin(&[], false),
    };
    SoundOrigins::new(
        &uses,
        vec![(OriginOutputAddress::OpaquePayload, origin)],
        FieldOrigins::new(),
    )
}

pub(super) struct SoundEvidence<'uses, 'sound> {
    uses: &'uses SourceUses,
    pub(super) payload: Option<SoundPayload>,
    plan: WemOriginPlan<'sound>,
}

impl<'uses, 'sound> SoundEvidence<'uses, 'sound> {
    pub(super) fn observe(
        wem: &'sound Wem<impl AsRef<[u8]>>,
        input: Option<SoundOriginInput<'_>>,
        uses: &'uses SourceUses,
    ) -> io::Result<Self> {
        let payload = input
            .map(|input| SoundPayload::observe(input, uses))
            .transpose()?;
        Ok(Self {
            uses,
            payload,
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
            .map(|site| payload.planned(site))
            .collect()
    }

    pub(super) fn name_origin(&self) -> io::Result<FieldOrigin> {
        self.payload.as_ref().map_or_else(
            || Ok(self.uses.origin(&[], false)),
            |payload| payload.name_origin(self.uses),
        )
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
            uses.push(payload.range(namespace, field, range, role)?);
        }
        Ok(())
    }

    pub(super) fn origin(&self, uses: &[SourceUse]) -> io::Result<FieldOrigin> {
        self.payload.as_ref().map_or_else(
            || Ok(self.uses.origin(uses, false)),
            |payload| payload.origin(self.uses, uses),
        )
    }

    pub(super) fn authored(&self) -> FieldOrigin {
        self.uses.origin(&[], true)
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

pub(super) fn insert_summary(
    fields: &mut FieldOrigins,
    name: &str,
    origin: FieldOrigin,
) -> io::Result<()> {
    let path = FieldPath::parse(name).map_err(io::Error::other)?;
    fields.insert(path, origin).map_err(io::Error::other)
}
