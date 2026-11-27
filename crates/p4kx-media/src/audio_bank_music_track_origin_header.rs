// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    BankOrigins, MediaError, MediaOriginInput, MediaWriter, Phase, PreparedRecordOrigins,
    SourceUseRole, TrackSites, coordinate, field, track_site,
};

impl TrackSites<'_> {
    pub(in crate::audio_bank_music_track) fn header(
        &self,
        input: &MediaOriginInput,
        has_issue: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = prepared.capture([self.object.route()])?;
        self.object.indices_origin(&mut prepared, position)?;
        self.header_words(&mut prepared)?;
        self.parent(&mut prepared)?;
        self.switch(&mut prepared)?;
        let layout = self.evidence(&prepared, Phase::Complete, &[])?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    fn header_words(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let fields = self.fields;
        let kind = fields.kind_code.as_ref().map(coordinate);
        for (path, name, phase, bytes, role) in [
            (
                "/data/payload/source_count",
                "musicTrackSourceCount",
                Phase::SourceCount,
                fields.source_count.as_ref().map(coordinate),
                SourceUseRole::Payload,
            ),
            (
                "/data/payload/clip_count",
                "musicTrackClipCount",
                Phase::ClipCount,
                fields.clip_count.as_ref().map(coordinate),
                SourceUseRole::Payload,
            ),
            (
                "/data/payload/automation_count",
                "musicTrackAutomationCount",
                Phase::AutomationCount,
                fields.automation_count.as_ref().map(coordinate),
                SourceUseRole::Payload,
            ),
            (
                "/data/payload/kind_code",
                "musicTrackKind",
                Phase::Kind,
                kind,
                SourceUseRole::Payload,
            ),
            (
                "/data/payload/kind",
                "musicTrackKind",
                Phase::Kind,
                kind,
                SourceUseRole::Conversion,
            ),
        ] {
            self.header_word(prepared, path, name, phase, bytes.as_ref(), role)?;
        }
        let evidence = self.evidence(prepared, Phase::Parent, &[])?;
        optional_range(
            prepared,
            "/data/payload/automation_range",
            fields.automation_range.is_some(),
            &evidence,
        )
    }

    fn parent(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let parent = &self.fields.parent;
        let effect = parent.effect_count.as_ref().map(coordinate);
        let metadata = parent.metadata_count.as_ref().map(coordinate);
        let id = parent.parent_id.as_ref().map(coordinate);
        let sites: Vec<_> = [
            effect
                .as_ref()
                .map(|span| field(span, "musicTrackEffectCount", SourceUseRole::Selector)),
            metadata
                .as_ref()
                .map(|span| field(span, "musicTrackMetadataCount", SourceUseRole::Selector)),
            id.as_ref()
                .map(|span| field(span, "musicTrackParentId", SourceUseRole::Payload)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let evidence = self.evidence(prepared, Phase::Parent, &sites)?;
        prepared.field("/data/payload/parent_id", evidence)
    }

    fn switch(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let selector = self.evidence(prepared, Phase::SwitchHeader, &[])?;
        prepared.field("/data/payload/switch", selector)?;
        let Some(switch) = &self.fields.switch else {
            return Ok(());
        };
        for (path, name, span) in [
            (
                "/data/payload/switch/group_type",
                "musicTrackSwitchGroupType",
                switch.group_type.as_ref().map(coordinate),
            ),
            (
                "/data/payload/switch/group_id",
                "musicTrackSwitchGroupId",
                switch.group_id.as_ref().map(coordinate),
            ),
            (
                "/data/payload/switch/default_switch",
                "musicTrackDefaultSwitch",
                switch.default_switch.as_ref().map(coordinate),
            ),
            (
                "/data/payload/switch/member_count",
                "musicTrackSwitchMemberCount",
                switch.member_count.as_ref().map(coordinate),
            ),
        ] {
            self.header_word(
                prepared,
                path,
                name,
                Phase::SwitchHeader,
                span.as_ref(),
                SourceUseRole::Payload,
            )?;
        }
        let object = self.object.indices();
        let evidence = self.evidence(
            prepared,
            Phase::SwitchMembers,
            &[track_site(&object, "switchTransitionRouting")],
        )?;
        optional_range(
            prepared,
            "/data/payload/switch/transition_range",
            switch.transition_range.is_some(),
            &evidence,
        )
    }

    fn header_word(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        path: &str,
        name: &'static str,
        phase: Phase,
        bytes: Option<&[u64; 2]>,
        role: SourceUseRole,
    ) -> Result<(), MediaError> {
        let word = bytes.map(|span| field(span, name, role));
        let evidence = self.evidence(prepared, phase, word.as_slice())?;
        prepared.field(path, evidence)
    }
}

fn optional_range(
    prepared: &mut BankOrigins<'_, '_, '_>,
    path: &str,
    present: bool,
    evidence: &super::OriginContributions,
) -> Result<(), MediaError> {
    if present {
        prepared.range(path, evidence, evidence)
    } else {
        prepared.field(path, evidence.clone())
    }
}
