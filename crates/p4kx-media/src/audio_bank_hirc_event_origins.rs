// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankEventAction, BankHircField, BankHircObject};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

#[derive(Clone, Copy)]
pub(super) enum CountRead<'source> {
    Observed {
        count: Option<&'source BankHircField<u32>>,
        prefix: &'source Range<u64>,
    },
    Unsupported,
}

pub(super) struct EventSites {
    section: [u64; 1],
    object: [u64; 2],
    body: [u64; 1],
    bkhd: [u64; 1],
    version: u32,
}

impl EventSites {
    pub(super) const fn new(object: &BankHircObject, version: VersionSite) -> Self {
        Self {
            section: [object.section_ordinal()],
            object: [object.section_ordinal(), object.ordinal()],
            body: [object.body_offset()],
            bkhd: [version.body_offset],
            version: version.version,
        }
    }

    pub(super) const fn object_indices(&self) -> [u64; 2] {
        self.object
    }

    pub(super) const fn action_indices(&self, ordinal: u32) -> [u64; 3] {
        [self.object[0], self.object[1], ordinal as u64]
    }

    pub(super) fn header(
        &self,
        input: &MediaOriginInput,
        read: CountRead<'_>,
        has_issue: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = prepared.capture([self.route()])?;
        self.indices(&mut prepared, position)?;
        let encoding = prepared.capture(self.base())?;
        prepared.field("/data/payload/count_encoding", encoding.clone())?;
        let layout = match read {
            CountRead::Observed { count, prefix } => {
                self.observed_count(&mut prepared, count, prefix)?
            }
            CountRead::Unsupported => {
                prepared.field("/data/payload/action_count", encoding.clone())?;
                prepared.field("/data/payload/count_range", encoding.clone())?;
                encoding
            }
        };
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    pub(super) fn action(
        &self,
        input: &MediaOriginInput,
        count: &BankHircField<u32>,
        action: &BankEventAction,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.action_indices(action.ordinal);
        let route = (
            OriginSite::new("bnkHircEvent", &indices, "actionRouting"),
            SourceUseRole::Selector,
        );
        let count_range = coordinates(&count.range);
        let count = original(&count_range, "eventActionCount", SourceUseRole::Selector);
        let selection = self.base().into_iter().chain([route, count]);
        let position = prepared.capture(selection.clone())?;
        self.indices(&mut prepared, position.clone())?;
        let object = prepared.capture([self.route()])?;
        prepared.field("/subresource/indices/1", object)?;
        prepared.field("/data/payload/ordinal", position.clone())?;
        prepared.range("/data/payload/id_range", &position, &position)?;
        let source = coordinates(&action.action_id.range);
        let evidence = prepared.capture(selection.chain([original(
            &source,
            "eventActionId",
            SourceUseRole::Payload,
        )]))?;
        prepared.field("/data/payload/action_id", evidence)?;
        Ok(prepared.finish())
    }

    pub(super) fn tail(
        &self,
        input: &MediaOriginInput,
        count: &BankHircField<u32>,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let object = prepared.capture([self.route()])?;
        self.indices(&mut prepared, object)?;
        let prefix = self.action_indices(count.value);
        let count_range = coordinates(&count.range);
        let sites = self.base().into_iter().chain([
            original(&count_range, "eventActionCount", SourceUseRole::Conversion),
            (
                OriginSite::new("bnkHircEvent", &prefix, "completeActionRouting"),
                SourceUseRole::Selector,
            ),
        ]);
        let start = prepared.capture(sites.clone())?;
        let end = prepared.capture(sites.chain([self.body_length()]))?;
        prepared.range("/data/payload/trailing_range", &start, &end)?;
        Ok(prepared.finish())
    }

    fn observed_count(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        count: Option<&BankHircField<u32>>,
        prefix: &Range<u64>,
    ) -> Result<OriginContributions, MediaError> {
        let prefix_range = coordinates(prefix);
        let inspected = (!prefix.is_empty()).then_some(original(
            &prefix_range,
            "eventActionCount",
            SourceUseRole::Selector,
        ));
        let layout = prepared.capture(
            self.base()
                .into_iter()
                .chain([self.body_length()])
                .chain(inspected),
        )?;
        let count_origin = count.map_or_else(
            || Ok(layout.clone()),
            |count| {
                let range = coordinates(&count.range);
                prepared.capture(self.base().into_iter().chain([original(
                    &range,
                    "eventActionCount",
                    SourceUseRole::Payload,
                )]))
            },
        )?;
        prepared.field("/data/payload/action_count", count_origin)?;
        let position = prepared.capture(self.base())?;
        prepared.range("/data/payload/count_range", &position, &layout)?;
        Ok(layout)
    }

    fn indices(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        position: OriginContributions,
    ) -> Result<(), MediaError> {
        prepared.indices(position)?;
        let section = prepared.capture([section_routing(&self.section)])?;
        prepared.field("/subresource/indices/0", section)
    }

    const fn route(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircPrefix", &self.object, "objectRouting"),
            SourceUseRole::Selector,
        )
    }

    const fn body_length(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircObject", &self.body, "bodyLength"),
            SourceUseRole::Selector,
        )
    }

    pub(super) const fn version(&self) -> u32 {
        self.version
    }

    pub(super) const fn base(&self) -> [EvidenceSite<'_>; 4] {
        [
            self.route(),
            complete_directory("bankVersionSelection"),
            (
                OriginSite::new("bnkHircObject", &self.body, "typeCode"),
                SourceUseRole::Selector,
            ),
            (
                OriginSite::new("bnkBkhd", &self.bkhd, "version"),
                SourceUseRole::Selector,
            ),
        ]
    }
}

const fn coordinates(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}

const fn original<'source>(
    range: &'source [u64; 2],
    field: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'source> {
    (OriginSite::new("bnkHircField", range, field), role)
}
