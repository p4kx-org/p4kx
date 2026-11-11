// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{BankHircField, BankHircObject};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, section_routing};
use crate::audio_bank_payload::BankField;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter};

#[derive(Clone, Copy)]
pub(crate) enum NodeKind {
    State,
    Rtpc,
    ActorMixer,
    Properties,
    Fx,
    Aux,
    Positioning,
    AdvancedSettings,
    ActionProperties,
    RandomSequence,
}

impl NodeKind {
    const fn namespace(self) -> &'static str {
        match self {
            Self::State => "bnkHircState",
            Self::Rtpc => "bnkHircRtpc",
            Self::ActorMixer => "bnkHircActorMixer",
            Self::Properties => "bnkHircNodeProperties",
            Self::Fx => "bnkHircNodeFx",
            Self::Aux => "bnkHircNodeAux",
            Self::Positioning => "bnkHircNodePositioning",
            Self::AdvancedSettings => "bnkHircNodeAdvancedSettings",
            Self::ActionProperties => "bnkHircActionProperties",
            Self::RandomSequence => "bnkHircRandomSequence",
        }
    }

    const fn position(self) -> &'static str {
        match self {
            Self::State => "stateRowPosition",
            Self::Rtpc => "rtpcRowPosition",
            Self::ActorMixer => "actorMixerRowPosition",
            Self::Properties => "nodePropertyRowPosition",
            Self::Fx => "nodeFxSlotPosition",
            Self::Aux => "nodeAuxRowPosition",
            Self::Positioning => "nodePositioningRowPosition",
            Self::AdvancedSettings => "nodeAdvancedRowPosition",
            Self::ActionProperties => "actionPropertyPosition",
            Self::RandomSequence => "randomSequencePlaylistPosition",
        }
    }

    const fn extent(self) -> &'static str {
        match self {
            Self::State => "stateExtent",
            Self::Rtpc => "rtpcExtent",
            Self::ActorMixer => "actorMixerExtent",
            Self::Properties => "nodePropertyExtent",
            Self::Fx => "nodeFxExtent",
            Self::Aux => "nodeAuxExtent",
            Self::Positioning => "nodePositioningExtent",
            Self::AdvancedSettings => "nodeAdvancedExtent",
            Self::ActionProperties => "actionPropertyExtent",
            Self::RandomSequence => "randomSequenceExtent",
        }
    }
}

pub(crate) struct NodeSites {
    object: ObjectSites,
    header: [u64; 1],
    kind: NodeKind,
}

impl NodeSites {
    pub(crate) const fn new(object: &BankHircObject, version: VersionSite, kind: NodeKind) -> Self {
        Self {
            object: ObjectSites::new(object),
            header: [version.body_offset],
            kind,
        }
    }

    pub(crate) const fn indices(&self) -> [u64; 2] {
        self.object.indices()
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
                self.kind.position(),
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

    pub(crate) fn issues(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        range: &Range<u64>,
    ) -> Result<(), MediaError> {
        let evidence = self.selection(prepared, Some(range))?;
        prepared.subtree("/issues", evidence)
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

    pub(crate) fn scalar<T: Copy>(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        name: &str,
        field: Option<&BankHircField<T>>,
        original: &'static str,
    ) -> Result<Option<BankField<T>>, MediaError> {
        field
            .map(|field| {
                self.field(prepared, name, &field.range, original)?;
                Ok(field.into())
            })
            .transpose()
    }

    pub(crate) fn range(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        name: &str,
        extent: &Range<u64>,
    ) -> Result<(), MediaError> {
        let evidence = self.observed(
            prepared,
            extent,
            self.kind.extent(),
            SourceUseRole::Selector,
        )?;
        prepared.subtree(&format!("/data/payload/{name}"), evidence)
    }

    pub(crate) fn converted(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        name: &str,
        range: &Range<u64>,
        original: &'static str,
    ) -> Result<(), MediaError> {
        let evidence = self.observed(prepared, range, original, SourceUseRole::Conversion)?;
        prepared.field(&format!("/data/payload/{name}"), evidence)
    }

    fn selection(
        &self,
        prepared: &BankOrigins<'_, '_, '_>,
        extent: Option<&Range<u64>>,
    ) -> Result<OriginContributions, MediaError> {
        extent.map_or_else(
            || prepared.capture(self.object.versioned(&self.header)),
            |range| self.observed(prepared, range, self.kind.extent(), SourceUseRole::Selector),
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
        prepared.capture(self.object.versioned(&self.header).into_iter().chain([(
            OriginSite::new(self.kind.namespace(), &indices, original),
            role,
        )]))
    }
}
