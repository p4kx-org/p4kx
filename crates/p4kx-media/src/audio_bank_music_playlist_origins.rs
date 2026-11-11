// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircObject, BankMusicPlaylistFields, BankMusicPlaylistItem};

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

const NAMESPACE: &str = "bnkHircMusicPlaylist";
const ITEM_COUNT: &str = "musicPlaylistItemCount";

pub(super) struct PlaylistSites<'a> {
    node: MusicSites<'a>,
    transitions: MusicTransitionSites,
    item_count: Option<[u64; 2]>,
}

impl<'a> PlaylistSites<'a> {
    pub(super) fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: &'a BankMusicPlaylistFields,
    ) -> Self {
        Self {
            node: MusicSites::for_node(object, version, &fields.node, NAMESPACE),
            transitions: MusicTransitionSites::new(
                &fields.transitions,
                TransitionNames {
                    count: "musicPlaylistTransitionRuleCount",
                    extent: "musicPlaylistTransitionRuleExtent",
                },
            ),
            item_count: fields.item_count.as_ref().map(|word| span(&word.range)),
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
        let items = prepared.capture(
            self.after_transitions().chain(
                self.item_count
                    .as_ref()
                    .map(|range| field(range, ITEM_COUNT, SourceUseRole::Payload)),
            ),
        )?;
        prepared.field("/data/payload/item_count", items)?;
        let indices = self.node.object_indices();
        let layout = prepared.capture(
            self.item_list()
                .chain([route(&indices, "playlistLayoutDecision")]),
        )?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    pub(super) fn item(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        item: &BankMusicPlaylistItem,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.node.indices(ordinal);
        let position = prepared.capture(
            self.item_list()
                .chain([route(&indices, "playlistItemRouting")]),
        )?;
        self.node.row_identity(&mut prepared, &position)?;
        prepared.range("/data/payload/item_range", &position, &position)?;
        for word in authored(item).into_iter().chain(interpreted(item)) {
            let bytes = span(word.range);
            let evidence = prepared.capture(self.item_list().chain([
                route(&indices, "playlistItemRouting"),
                field(&bytes, word.name, word.role),
            ]))?;
            prepared.field(word.path, evidence)?;
        }
        Ok(prepared.finish())
    }

    fn after_transitions(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.transitions.following(&self.node)
    }

    fn item_list(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.after_transitions().chain(
            self.item_count
                .as_ref()
                .map(|range| field(range, ITEM_COUNT, SourceUseRole::Selector)),
        )
    }
}

struct ItemWord<'a> {
    path: &'static str,
    name: &'static str,
    range: &'a Range<u64>,
    role: SourceUseRole,
}

impl<'a> ItemWord<'a> {
    const fn payload(path: &'static str, name: &'static str, range: &'a Range<u64>) -> Self {
        Self {
            path,
            name,
            range,
            role: SourceUseRole::Payload,
        }
    }

    const fn conversion(path: &'static str, name: &'static str, range: &'a Range<u64>) -> Self {
        Self {
            path,
            name,
            range,
            role: SourceUseRole::Conversion,
        }
    }
}

const fn authored(item: &BankMusicPlaylistItem) -> [ItemWord<'_>; 11] {
    [
        ItemWord::payload(
            "/data/payload/segment_id",
            "musicPlaylistSegmentId",
            &item.segment_id.range,
        ),
        ItemWord::payload(
            "/data/payload/item_id",
            "musicPlaylistItemId",
            &item.item_id.range,
        ),
        ItemWord::payload(
            "/data/payload/children",
            "musicPlaylistItemChildCount",
            &item.children.range,
        ),
        ItemWord::payload(
            "/data/payload/play_mode_code",
            "musicPlaylistPlayMode",
            &item.play_mode.range,
        ),
        ItemWord::payload(
            "/data/payload/loops",
            "musicPlaylistLoopCount",
            &item.loops.range,
        ),
        ItemWord::payload(
            "/data/payload/loop_min",
            "musicPlaylistLoopMin",
            &item.loop_min.range,
        ),
        ItemWord::payload(
            "/data/payload/loop_max",
            "musicPlaylistLoopMax",
            &item.loop_max.range,
        ),
        ItemWord::payload(
            "/data/payload/weight",
            "musicPlaylistWeight",
            &item.weight.range,
        ),
        ItemWord::payload(
            "/data/payload/avoid_repeat_count",
            "musicPlaylistAvoidRepeatCount",
            &item.avoid_repeat_count.range,
        ),
        ItemWord::payload(
            "/data/payload/uses_weight_code",
            "musicPlaylistUsesWeight",
            &item.uses_weight_code.range,
        ),
        ItemWord::payload(
            "/data/payload/shuffle_code",
            "musicPlaylistShuffle",
            &item.shuffle_code.range,
        ),
    ]
}

const fn interpreted(item: &BankMusicPlaylistItem) -> [ItemWord<'_>; 3] {
    [
        ItemWord::conversion(
            "/data/payload/play_mode",
            "musicPlaylistPlayMode",
            &item.play_mode.range,
        ),
        ItemWord::conversion(
            "/data/payload/uses_weight",
            "musicPlaylistUsesWeight",
            &item.uses_weight_code.range,
        ),
        ItemWord::conversion(
            "/data/payload/shuffle",
            "musicPlaylistShuffle",
            &item.shuffle_code.range,
        ),
    ]
}

const fn route<'a>(indices: &'a [u64], name: &'static str) -> EvidenceSite<'a> {
    (
        OriginSite::new(NAMESPACE, indices, name),
        SourceUseRole::Selector,
    )
}
