// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankMusicSwitchBranch, BankMusicSwitchTarget};

use crate::audio_bank_music_transition_origins::{field, span};
use crate::audio_bank_origins::BankOrigins;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

use super::{SwitchSites, route};

const CHILD_LINK: &str = "musicSwitchChildLink";
const TARGET_ID: &str = "musicSwitchTargetId";

impl SwitchSites<'_> {
    pub(in crate::audio_bank_music_switch) fn target<'branch>(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        target: &BankMusicSwitchTarget,
        path: impl Iterator<Item = &'branch BankMusicSwitchBranch>,
        writer: &MediaWriter<'_>,
    ) -> Result<(PreparedRecordOrigins, Vec<u32>), MediaError> {
        let (ancestor_nodes, links): (Vec<_>, Vec<_>) = path
            .map(|branch| (branch.node_index, span(&branch.link.range)))
            .unzip();
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.node.indices(ordinal);
        let leaf = span(&target.id.range);
        let routing = self
            .tree()
            .chain([route(&indices, "targetTraversalPrefix")]);
        let selected = routing.clone().chain(
            links
                .iter()
                .map(|link| field(link, CHILD_LINK, SourceUseRole::Selector)),
        );
        let occurrence = prepared.capture(selected.clone().chain([field(
            &leaf,
            TARGET_ID,
            SourceUseRole::Selector,
        )]))?;
        self.node.row_identity(&mut prepared, &occurrence)?;
        prepared.field("/data/payload/node_index", occurrence.clone())?;
        prepared.field("/data/payload/level", occurrence.clone())?;
        prepared.field("/data/payload/ancestor_nodes", occurrence)?;
        let evidence =
            prepared.capture(selected.chain([field(&leaf, TARGET_ID, SourceUseRole::Payload)]))?;
        prepared.field("/data/payload/target_id", evidence)?;
        for (index, link) in links.iter().enumerate() {
            let evidence = prepared.capture(routing.clone().chain([
                field(link, CHILD_LINK, SourceUseRole::Selector),
                field(&leaf, TARGET_ID, SourceUseRole::Selector),
            ]))?;
            prepared.field(&format!("/data/payload/ancestor_nodes/{index}"), evidence)?;
        }
        Ok((prepared.finish(), ancestor_nodes))
    }
}
