// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankHircObject, BankMusicSwitchFields, BankMusicSwitchGroup, BankMusicSwitchNode,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_segment::MusicSites;
use crate::audio_bank_music_transition_origins::{
    MusicTransitionSites, TransitionNames, field, span,
};
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

#[path = "audio_bank_music_switch_target_origins.rs"]
mod target;

const NAMESPACE: &str = "bnkHircMusicSwitch";
const GROUP_COUNT: &str = "musicSwitchGroupCount";
const TREE_SIZE: &str = "musicSwitchTreeSize";

pub(super) struct SwitchSites<'a> {
    node: MusicSites<'a>,
    transitions: MusicTransitionSites,
    continue_playback: Option<[u64; 2]>,
    group_count: Option<[u64; 2]>,
    tree_size: Option<[u64; 2]>,
    tree_mode: Option<[u64; 2]>,
    tree_range: Option<[u64; 2]>,
}

impl<'a> SwitchSites<'a> {
    pub(super) fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: &'a BankMusicSwitchFields,
    ) -> Self {
        Self {
            node: MusicSites::for_node(object, version, &fields.node, NAMESPACE),
            transitions: MusicTransitionSites::new(
                &fields.transitions,
                TransitionNames {
                    count: "musicSwitchTransitionRuleCount",
                    extent: "musicSwitchTransitionRuleExtent",
                },
            ),
            continue_playback: fields
                .continue_playback_code
                .as_ref()
                .map(|word| span(&word.range)),
            group_count: fields.group_count.as_ref().map(|word| span(&word.range)),
            tree_size: fields.tree_size.as_ref().map(|word| span(&word.range)),
            tree_mode: fields.tree_mode_code.as_ref().map(|word| span(&word.range)),
            tree_range: fields.tree_range.as_ref().map(span),
        }
    }

    pub(super) const fn node(&self) -> &MusicSites<'a> {
        &self.node
    }

    pub(super) fn header(
        &self,
        input: &MediaOriginInput,
        has_issue: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.node.prepare_node_header(input, writer)?;
        self.transitions.write(&self.node, &mut prepared)?;
        self.header_words(&mut prepared)?;
        self.tree_range(&mut prepared)?;
        let indices = self.node.object_indices();
        let layout =
            prepared.capture(self.tree().chain([route(&indices, "decisionTreeLayout")]))?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    fn header_words(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        for (path, name, range) in [
            (
                "/data/payload/continue_playback_code",
                "musicSwitchContinuePlayback",
                self.continue_playback.as_ref(),
            ),
            (
                "/data/payload/group_count",
                GROUP_COUNT,
                self.group_count.as_ref(),
            ),
        ] {
            let evidence = prepared.capture(
                self.transitions
                    .following(&self.node)
                    .chain(range.map(|range| field(range, name, SourceUseRole::Payload))),
            )?;
            prepared.field(path, evidence)?;
        }
        for (path, name, range) in [
            (
                "/data/payload/tree_size",
                TREE_SIZE,
                self.tree_size.as_ref(),
            ),
            (
                "/data/payload/tree_mode_code",
                "musicSwitchTreeMode",
                self.tree_mode.as_ref(),
            ),
        ] {
            let evidence = prepared.capture(
                self.groups()
                    .chain(range.map(|range| field(range, name, SourceUseRole::Payload))),
            )?;
            prepared.field(path, evidence)?;
        }
        Ok(())
    }

    fn tree_range(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let indices = self.node.object_indices();
        let start =
            prepared.capture(self.groups().chain([route(&indices, "decisionTreeStart")]))?;
        if self.tree_range.is_some() {
            let end = prepared.capture(self.tree())?;
            prepared.range("/data/payload/tree_range", &start, &end)
        } else {
            prepared.field("/data/payload/tree_range", start)
        }
    }

    pub(super) fn group(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        group: &BankMusicSwitchGroup,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.node.indices(ordinal);
        let position = prepared.capture(self.groups().chain([route(&indices, "groupRouting")]))?;
        self.node.row_identity(&mut prepared, &position)?;
        let id = span(&group.id.range);
        let evidence = prepared.capture(self.groups().chain([
            route(&indices, "groupRouting"),
            field(&id, "musicSwitchGroupId", SourceUseRole::Payload),
        ]))?;
        prepared.field("/data/payload/group_id", evidence)?;
        let code = group.type_code.as_ref().map(|word| span(&word.range));
        let evidence = prepared.capture(
            self.groups()
                .chain([route(&indices, "groupRouting")])
                .chain(
                    code.as_ref()
                        .map(|range| field(range, "musicSwitchGroupType", SourceUseRole::Payload)),
                ),
        )?;
        prepared.field("/data/payload/type_code", evidence)?;
        Ok(prepared.finish())
    }

    pub(super) fn physical_node(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        node: &BankMusicSwitchNode,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.node.indices(ordinal);
        let position =
            prepared.capture(self.tree().chain([route(&indices, "physicalNodeRouting")]))?;
        self.node.row_identity(&mut prepared, &position)?;
        prepared.range("/data/payload/node_range", &position, &position)?;
        for (path, name, bytes) in [
            (
                "/data/payload/key",
                "musicSwitchNodeKey",
                span(&node.key.range),
            ),
            (
                "/data/payload/link_word",
                "musicSwitchNodeLink",
                span(&node.link.range),
            ),
            (
                "/data/payload/weight",
                "musicSwitchNodeWeight",
                span(&node.weight.range),
            ),
            (
                "/data/payload/probability",
                "musicSwitchNodeProbability",
                span(&node.probability.range),
            ),
        ] {
            let evidence = prepared.capture(self.tree().chain([
                route(&indices, "physicalNodeRouting"),
                field(&bytes, name, SourceUseRole::Payload),
            ]))?;
            prepared.field(path, evidence)?;
        }
        Ok(prepared.finish())
    }

    fn groups(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.transitions.following(&self.node).chain(
            self.group_count
                .as_ref()
                .map(|span| field(span, GROUP_COUNT, SourceUseRole::Selector)),
        )
    }

    fn tree(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.groups()
            .chain(
                self.tree_size
                    .as_ref()
                    .map(|span| field(span, TREE_SIZE, SourceUseRole::Selector)),
            )
            .chain(
                self.tree_range
                    .as_ref()
                    .map(|span| field(span, "musicSwitchTreeExtent", SourceUseRole::Selector)),
            )
    }
}

const fn route<'a>(indices: &'a [u64], name: &'static str) -> EvidenceSite<'a> {
    (
        OriginSite::new(NAMESPACE, indices, name),
        SourceUseRole::Selector,
    )
}
