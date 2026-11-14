// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankActionFields, BankHircField, BankHircObject, BankPlayBankDecision,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct Sites {
    ordinal: [u64; 2],
    body: [u64; 1],
    bkhd: [u64; 1],
    version: u32,
    action_type: Option<[u64; 2]>,
    target: Option<[u64; 2]>,
    select_type: bool,
    reached: bool,
    play: Option<PlaySites>,
}

struct PlaySites {
    property_count: Option<[u64; 2]>,
    ranged_property_count: Option<[u64; 2]>,
    bank_id: Option<[u64; 2]>,
    bank_type: Option<[u64; 2]>,
}

pub(super) fn prepare(
    input: &MediaOriginInput,
    sites: &Sites,
    has_issue: bool,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = BankOrigins::new(input, writer)?;
    let routing = prepared.capture([sites.route()])?;
    prepared.indices(routing)?;
    let section = prepared.capture([section_routing(&[sites.ordinal[0]])])?;
    prepared.field("/subresource/indices/0", section)?;
    sites.scalar_fields(&mut prepared)?;
    sites.play_bank(&mut prepared)?;
    let layout = prepared.capture(
        sites
            .base()
            .into_iter()
            .chain(sites.branch_sites())
            .chain(sites.scalar_sites().into_iter().flatten())
            .chain(sites.play_words())
            .chain(sites.bound()),
    )?;
    prepared.field("/data/payload/layout", layout.clone())?;
    if has_issue {
        prepared.subtree("/issues", layout)?;
    }
    Ok(prepared.finish())
}

impl Sites {
    pub(super) fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: Option<&BankActionFields>,
    ) -> Self {
        let decision = fields.map(|fields| &fields.play_bank);
        let play = match decision {
            Some(BankPlayBankDecision::Play(fields)) => Some(PlaySites {
                property_count: fields.property_count.as_ref().map(field_range),
                ranged_property_count: fields.ranged_property_count.as_ref().map(field_range),
                bank_id: fields.bank_id.as_ref().map(field_range),
                bank_type: fields.bank_type.as_ref().map(field_range),
            }),
            _ => None,
        };
        Self {
            ordinal: [object.section_ordinal(), object.ordinal()],
            body: [object.body_offset()],
            bkhd: [version.body_offset],
            version: version.version,
            action_type: fields
                .and_then(|fields| fields.action_type.as_ref())
                .map(field_range),
            target: fields
                .and_then(|fields| fields.target_id.as_ref())
                .map(field_range),
            select_type: matches!(
                decision,
                Some(BankPlayBankDecision::NonPlay | BankPlayBankDecision::Play(_))
            ),
            reached: fields.is_some(),
            play,
        }
    }

    pub(super) const fn indices(&self) -> [u64; 2] {
        self.ordinal
    }

    pub(super) const fn version(&self) -> u32 {
        self.version
    }

    pub(super) fn target_site(&self) -> Result<EvidenceSite<'_>, MediaError> {
        self.target
            .as_ref()
            .map(|range| action_site(range, "targetId", SourceUseRole::Selector))
            .ok_or_else(|| {
                MediaError::Contract("decoded Action lacks its original target ID".into())
            })
    }

    pub(super) fn action_selector(&self) -> Result<EvidenceSite<'_>, MediaError> {
        self.action_type
            .as_ref()
            .map(|range| action_site(range, "actionType", SourceUseRole::Selector))
            .ok_or_else(|| MediaError::Contract("decoded Action lacks its original type".into()))
    }

    pub(super) fn bank_selectors(&self) -> impl Iterator<Item = EvidenceSite<'_>> {
        self.play.iter().flat_map(|play| {
            let id = play
                .bank_id
                .as_ref()
                .map(|range| action_site(range, "bankId", SourceUseRole::Selector));
            play.counts().into_iter().flatten().chain(id)
        })
    }

    pub(super) const fn route(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircPrefix", &self.ordinal, "objectRouting"),
            SourceUseRole::Selector,
        )
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

    const fn body_length(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircObject", &self.body, "bodyLength"),
            SourceUseRole::Selector,
        )
    }

    fn bound(&self) -> Option<EvidenceSite<'_>> {
        self.reached.then(|| self.body_length())
    }

    fn scalar_sites(&self) -> [Option<EvidenceSite<'_>>; 2] {
        [
            self.action_type
                .as_ref()
                .map(|range| action_site(range, "actionType", SourceUseRole::Payload)),
            self.target
                .as_ref()
                .map(|range| action_site(range, "targetId", SourceUseRole::Payload)),
        ]
    }

    fn scalar_fields(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        for (path, site) in ["/data/payload/action_type", "/data/payload/target_id"]
            .into_iter()
            .zip(self.scalar_sites())
        {
            let evidence =
                prepared.capture(self.base().into_iter().chain(site.or_else(|| self.bound())))?;
            prepared.field(path, evidence)?;
        }
        Ok(())
    }

    fn branch_sites(&self) -> impl Iterator<Item = EvidenceSite<'_>> {
        let action_type = self
            .action_type
            .as_ref()
            .filter(|_| self.select_type)
            .map(|range| action_site(range, "actionType", SourceUseRole::Selector));
        let counts = self.play.iter().flat_map(PlaySites::counts).flatten();
        action_type.into_iter().chain(counts)
    }

    fn play_words(&self) -> impl Iterator<Item = EvidenceSite<'_>> {
        self.play.iter().flat_map(PlaySites::words).flatten()
    }

    fn play_bank(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let parent = prepared.capture(
            self.base()
                .into_iter()
                .chain(self.branch_sites())
                .chain(self.play_words()),
        )?;
        prepared.field("/data/payload/play_bank", parent)?;
        if let Some(play) = &self.play {
            for (path, site) in [
                "/data/payload/play_bank/bank_id",
                "/data/payload/play_bank/bank_type",
            ]
            .into_iter()
            .zip(play.words())
            {
                let evidence = prepared.capture(
                    self.base()
                        .into_iter()
                        .chain(self.branch_sites())
                        .chain(site.or_else(|| self.bound())),
                )?;
                prepared.field(path, evidence)?;
            }
        }
        Ok(())
    }
}

const fn field_range<T>(field: &BankHircField<T>) -> [u64; 2] {
    [field.range.start, field.range.end]
}

impl PlaySites {
    fn counts(&self) -> [Option<EvidenceSite<'_>>; 2] {
        [
            (&self.property_count, "propertyCount"),
            (&self.ranged_property_count, "rangedPropertyCount"),
        ]
        .map(|(range, name)| {
            range
                .as_ref()
                .map(|range| action_site(range, name, SourceUseRole::Selector))
        })
    }

    fn words(&self) -> [Option<EvidenceSite<'_>>; 2] {
        [(&self.bank_id, "bankId"), (&self.bank_type, "bankType")].map(|(range, name)| {
            range
                .as_ref()
                .map(|range| action_site(range, name, SourceUseRole::Payload))
        })
    }
}

const fn action_site<'a>(
    range: &'a [u64; 2],
    name: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'a> {
    (OriginSite::new("bnkHircField", range, name), role)
}
