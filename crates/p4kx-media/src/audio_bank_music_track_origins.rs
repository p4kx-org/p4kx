// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircField, BankHircObject, BankMusicTrackFields};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Subresource};

#[path = "audio_bank_music_track_origin_header.rs"]
mod header;
#[path = "audio_bank_music_track_origin_rows.rs"]
mod rows;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum List {
    Source,
    Clip,
    SwitchMember,
}

impl List {
    const fn namespace(self) -> &'static str {
        match self {
            Self::Source => super::SOURCE,
            Self::Clip => super::CLIP,
            Self::SwitchMember => super::SWITCH_MEMBER,
        }
    }

    const fn route(self) -> &'static str {
        match self {
            Self::Source => "sourcePrefixRouting",
            Self::Clip => "clipRouting",
            Self::SwitchMember => "switchMemberRouting",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    SourceCount,
    Sources,
    ClipCount,
    Clips,
    AutomationCount,
    Automation,
    Parent,
    Kind,
    SwitchHeader,
    SwitchMembers,
    Complete,
}

pub(super) struct TrackSites<'a> {
    object: ObjectSites,
    version: [u64; 1],
    fields: &'a BankMusicTrackFields,
}

impl<'a> TrackSites<'a> {
    pub(super) const fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: &'a BankMusicTrackFields,
    ) -> Self {
        Self {
            object: ObjectSites::new(object),
            version: [version.body_offset],
            fields,
        }
    }

    pub(super) fn subresource(&self, row: Option<(List, u64)>) -> Subresource {
        row.map_or_else(
            || Subresource::new(super::NAMESPACE, self.object.indices().to_vec()),
            |(list, ordinal)| Subresource::new(list.namespace(), self.indices(ordinal).to_vec()),
        )
    }

    const fn indices(&self, ordinal: u64) -> [u64; 3] {
        let [section, object] = self.object.indices();
        [section, object, ordinal]
    }

    fn evidence(
        &self,
        prepared: &BankOrigins<'_, '_, '_>,
        phase: Phase,
        sites: &[EvidenceSite<'_>],
    ) -> Result<OriginContributions, MediaError> {
        let object = self.object.indices();
        let words = self.selectors();
        let selectors = words.iter().filter_map(|(at, bytes, name)| {
            bytes
                .as_ref()
                .filter(|_| phase >= *at)
                .map(|span| field(span, name, SourceUseRole::Selector))
        });
        let routes = self
            .routes()
            .into_iter()
            .filter(|(at, observed, _)| phase >= *at && *observed)
            .map(|(_, _, name)| track_site(&object, name));
        prepared.capture(
            self.object
                .versioned(&self.version)
                .into_iter()
                .chain([self.object.extent(SourceUseRole::Selector)])
                .chain(routes)
                .chain(selectors)
                .chain(sites.iter().copied()),
        )
    }

    fn selectors(&self) -> [Selector; 5] {
        let fields = self.fields;
        [
            (
                Phase::Sources,
                fields.source_count.as_ref().map(coordinate),
                "musicTrackSourceCount",
            ),
            (
                Phase::Clips,
                fields.clip_count.as_ref().map(coordinate),
                "musicTrackClipCount",
            ),
            (
                Phase::Automation,
                fields.automation_count.as_ref().map(coordinate),
                "musicTrackAutomationCount",
            ),
            (
                Phase::SwitchHeader,
                fields.kind_code.as_ref().map(coordinate),
                "musicTrackKind",
            ),
            (
                Phase::SwitchMembers,
                fields
                    .switch
                    .as_ref()
                    .and_then(|switch| switch.member_count.as_ref())
                    .map(coordinate),
                "musicTrackSwitchMemberCount",
            ),
        ]
    }

    const fn routes(&self) -> [(Phase, bool, &'static str); 5] {
        let fields = self.fields;
        [
            (
                Phase::ClipCount,
                fields.clip_count.is_some(),
                "completeSourceRouting",
            ),
            (
                Phase::AutomationCount,
                fields.automation_count.is_some(),
                "completeClipRouting",
            ),
            (
                Phase::Parent,
                fields.automation_range.is_some(),
                "automationRouting",
            ),
            (Phase::Kind, fields.kind_code.is_some(), "nodeRouting"),
            (Phase::Complete, true, "trackLayoutDecision"),
        ]
    }
}

type Selector = (Phase, Option<[u64; 2]>, &'static str);

const fn track_site<'a>(indices: &'a [u64], field: &'static str) -> EvidenceSite<'a> {
    (
        OriginSite::new("bnkHircMusicTrack", indices, field),
        SourceUseRole::Selector,
    )
}

const fn coordinate<T>(word: &BankHircField<T>) -> [u64; 2] {
    span(&word.range)
}

const fn span(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}

const fn field<'a>(
    bytes: &'a [u64; 2],
    name: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'a> {
    (OriginSite::new("bnkHircField", bytes, name), role)
}
