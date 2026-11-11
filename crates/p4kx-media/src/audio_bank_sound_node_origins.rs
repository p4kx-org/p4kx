// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankSoundNodeBase, BankSoundNodeFields};

use crate::MediaError;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::audio_bank_sound_origins::{self, Scalar, SoundSites};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;

const PLUGIN_LENGTH: &str = "/data/payload/continuation/plugin_parameter_length";
const PLUGIN_PARAMETERS: &str = "/data/payload/continuation/plugin_parameters_range";
const NODE: &str = "/data/payload/continuation/node_base";
const CONSUMED: &str = "/data/payload/continuation/node_base/consumed_range";
const TRAILING: &str = "/data/payload/continuation/node_base/trailing_range";

struct NodeSites<'a> {
    sound: &'a SoundSites,
    parameter_length: Scalar,
}

pub(crate) fn project(
    sound: &SoundSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    node: &BankSoundNodeFields,
) -> Result<OriginContributions, MediaError> {
    let sites = NodeSites {
        sound,
        parameter_length: Scalar::new(
            node.plugin_parameter_length.as_ref(),
            "soundPluginParameterLength",
        ),
    };
    sites.plugin_parameters(prepared, node)?;
    if let Some(node) = &node.base {
        return sites.node_base(prepared, node);
    }
    let layout = prepared.capture(sites.route())?;
    prepared.field(NODE, layout.clone())?;
    Ok(layout)
}

impl NodeSites<'_> {
    fn plugin_parameters(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        node: &BankSoundNodeFields,
    ) -> Result<(), MediaError> {
        let length = prepared.capture(
            self.plugin_branch()
                .chain(self.parameter_length.site(SourceUseRole::Payload)),
        )?;
        prepared.field(PLUGIN_LENGTH, length)?;
        let end = prepared.capture(
            self.plugin_branch()
                .chain(self.parameter_length.site(SourceUseRole::Conversion)),
        )?;
        if node.plugin_parameters.is_some() {
            let start = prepared.capture(self.plugin_branch())?;
            prepared.range(PLUGIN_PARAMETERS, &start, &end)
        } else {
            prepared.field(PLUGIN_PARAMETERS, end)
        }
    }

    fn node_base(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        node: &BankSoundNodeBase,
    ) -> Result<OriginContributions, MediaError> {
        self.parent_fields(prepared, node)?;
        let start = prepared.capture(self.route())?;
        let consumed = audio_bank_sound_origins::coordinates(&node.consumed);
        let layout_site = (
            OriginSite::new("bnkHircSoundNode", &consumed, "checkedLayout"),
            SourceUseRole::Selector,
        );
        let layout = prepared.capture(self.route().chain([layout_site]))?;
        prepared.range(CONSUMED, &start, &layout)?;
        if node.trailing.is_some() {
            prepared.range(TRAILING, &layout, &layout)?;
        } else {
            prepared.field(TRAILING, layout.clone())?;
        }
        Ok(layout)
    }

    fn parent_fields(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        node: &BankSoundNodeBase,
    ) -> Result<(), MediaError> {
        let fields = &node.fields;
        let effect = Scalar::new(fields.effect_count.as_ref(), "soundNodeEffectCount");
        let metadata = Scalar::new(fields.metadata_count.as_ref(), "soundNodeMetadataCount");
        let pre_parent = Scalar::new(fields.pre_parent_word.as_ref(), "soundNodePreParentWord");
        let parent = Scalar::new(fields.parent_id.as_ref(), "soundNodeParentId");
        for (path, word, selectors) in [
            (
                "/data/payload/continuation/node_base/effect_count",
                &effect,
                [None, None],
            ),
            (
                "/data/payload/continuation/node_base/metadata_count",
                &metadata,
                [Some(&effect), None],
            ),
            (
                "/data/payload/continuation/node_base/pre_parent_word",
                &pre_parent,
                [Some(&effect), Some(&metadata)],
            ),
            (
                "/data/payload/continuation/node_base/parent_id",
                &parent,
                [Some(&effect), Some(&metadata)],
            ),
        ] {
            let inputs = self
                .route()
                .chain(
                    selectors
                        .into_iter()
                        .flatten()
                        .filter_map(|word| word.site(SourceUseRole::Selector)),
                )
                .chain(word.site(SourceUseRole::Payload));
            let evidence = prepared.capture(inputs)?;
            prepared.field(path, evidence)?;
        }
        Ok(())
    }

    fn plugin_branch(&self) -> impl Iterator<Item = EvidenceSite<'_>> {
        self.sound
            .base()
            .into_iter()
            .chain([self.sound.extent()])
            .chain(self.sound.plugin.site(SourceUseRole::Selector))
    }

    fn route(&self) -> impl Iterator<Item = EvidenceSite<'_>> {
        self.plugin_branch()
            .chain(self.parameter_length.site(SourceUseRole::Selector))
    }
}
