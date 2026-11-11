// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::BankHircObject;

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter};

pub(crate) struct StateSites {
    object: ObjectSites,
    header: [u64; 1],
}

impl StateSites {
    pub(crate) const fn new(object: &BankHircObject, version: VersionSite) -> Self {
        Self {
            object: ObjectSites::new(object),
            header: [version.body_offset],
        }
    }

    pub(crate) fn record<'source, 'writer, 'run>(
        &self,
        input: &'source MediaOriginInput,
        writer: &'writer MediaWriter<'run>,
        first_read: Option<&Range<u64>>,
    ) -> Result<BankOrigins<'source, 'writer, 'run>, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = if let Some(range) = first_read {
            self.observed(
                &prepared,
                range,
                "stateRowPosition",
                SourceUseRole::Selector,
            )?
        } else {
            prepared.capture(self.object.versioned(&self.header))?
        };
        prepared.indices(position)?;
        let [section, _] = self.object.indices();
        let origin = prepared.capture([section_routing(&[section])])?;
        prepared.field("/subresource/indices/0", origin)?;
        let object = prepared.capture([self.object.route()])?;
        prepared.field("/subresource/indices/1", object)?;
        Ok(prepared)
    }

    pub(crate) fn summary(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        extent: Option<&Range<u64>>,
        has_issue: bool,
    ) -> Result<(), MediaError> {
        let version = prepared.capture([
            self.object.route(),
            (
                OriginSite::new("bnkBkhd", &self.header, "version"),
                SourceUseRole::Payload,
            ),
        ])?;
        prepared.field("/data/payload/version", version)?;
        let layout = self.selection(prepared, extent)?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(())
    }

    pub(crate) fn field(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        name: &str,
        field: &Range<u64>,
        original: &'static str,
    ) -> Result<(), MediaError> {
        let receipt = self.observed(prepared, field, original, SourceUseRole::Payload)?;
        prepared.subtree(&format!("/data/payload/{name}"), receipt)
    }

    pub(crate) fn range(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        name: &str,
        extent: &Range<u64>,
    ) -> Result<(), MediaError> {
        let evidence = self.observed(prepared, extent, "stateExtent", SourceUseRole::Selector)?;
        prepared.subtree(&format!("/data/payload/{name}"), evidence)
    }

    pub(crate) fn property_value(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        range: &Range<u64>,
    ) -> Result<(), MediaError> {
        let evidence =
            self.observed(prepared, range, "stateValueBits", SourceUseRole::Conversion)?;
        prepared.field("/data/payload/value", evidence)
    }

    fn selection(
        &self,
        prepared: &BankOrigins<'_, '_, '_>,
        extent: Option<&Range<u64>>,
    ) -> Result<OriginContributions, MediaError> {
        extent.map_or_else(
            || prepared.capture(self.object.versioned(&self.header)),
            |range| self.observed(prepared, range, "stateExtent", SourceUseRole::Selector),
        )
    }

    fn observed(
        &self,
        prepared: &BankOrigins<'_, '_, '_>,
        range: &Range<u64>,
        original: &'static str,
        role: SourceUseRole,
    ) -> Result<OriginContributions, MediaError> {
        let indices = [range.start, range.end];
        prepared.capture(
            self.object
                .versioned(&self.header)
                .into_iter()
                .chain([(OriginSite::new("bnkHircState", &indices, original), role)]),
        )
    }
}
