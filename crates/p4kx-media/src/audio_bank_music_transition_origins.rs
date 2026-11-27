// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::BankMusicTransitionFields;

use crate::MediaError;
use crate::audio_bank_music_segment::MusicSites;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_capture::OriginSite;

pub(crate) struct TransitionNames {
    pub(crate) count: &'static str,
    pub(crate) extent: &'static str,
}

pub(crate) struct MusicTransitionSites {
    names: TransitionNames,
    count: Option<[u64; 2]>,
    extent: Option<[u64; 2]>,
}

impl MusicTransitionSites {
    pub(crate) fn new(fields: &BankMusicTransitionFields, names: TransitionNames) -> Self {
        Self {
            names,
            count: fields.rule_count.as_ref().map(|word| span(&word.range)),
            extent: fields.rules_range.as_ref().map(span),
        }
    }

    pub(crate) fn write(
        &self,
        node: &MusicSites<'_>,
        prepared: &mut BankOrigins<'_, '_, '_>,
    ) -> Result<(), MediaError> {
        let count = prepared.capture(
            node.after_node().chain(
                self.count
                    .as_ref()
                    .map(|range| field(range, self.names.count, SourceUseRole::Payload)),
            ),
        )?;
        prepared.field("/data/payload/transition_rule_count", count)?;
        let start = prepared.capture(
            node.after_node().chain(
                self.count
                    .as_ref()
                    .map(|range| field(range, self.names.count, SourceUseRole::Selector)),
            ),
        )?;
        if self.extent.is_some() {
            let end = prepared.capture(self.following(node))?;
            prepared.range("/data/payload/transition_rules_range", &start, &end)
        } else {
            prepared.field("/data/payload/transition_rules_range", start)
        }
    }

    pub(crate) fn following<'a>(
        &'a self,
        node: &'a MusicSites<'_>,
    ) -> impl Iterator<Item = EvidenceSite<'a>> + Clone {
        node.after_node()
            .chain(
                self.count
                    .as_ref()
                    .map(|range| field(range, self.names.count, SourceUseRole::Selector)),
            )
            .chain(
                self.extent
                    .as_ref()
                    .map(|range| field(range, self.names.extent, SourceUseRole::Selector)),
            )
    }
}

pub(crate) const fn span(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}

pub(crate) const fn field<'a>(
    range: &'a [u64; 2],
    name: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'a> {
    (OriginSite::new("bnkHircField", range, name), role)
}
