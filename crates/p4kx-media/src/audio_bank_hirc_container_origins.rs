// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankContainerChild, BankContainerFields, BankHircField, BankHircObject,
};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct ContainerSites {
    object: ObjectSites,
    version: [u64; 1],
    parent: Option<[u64; 2]>,
    effects: Option<[u64; 2]>,
    metadata: Option<[u64; 2]>,
    mode: Option<[u64; 2]>,
    count: Option<[u64; 2]>,
    node_range: Option<[u64; 2]>,
    admitted: bool,
    suffix: Option<[u64; 2]>,
}

impl ContainerSites {
    pub(super) fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: &BankContainerFields,
    ) -> Self {
        Self {
            object: ObjectSites::new(object),
            version: [version.body_offset],
            parent: fields.node.parent_id.as_ref().map(word),
            effects: fields.node.effect_count.as_ref().map(word),
            metadata: fields.node.metadata_count.as_ref().map(word),
            mode: fields.mode.as_ref().map(word),
            count: fields.child_count.as_ref().map(word),
            node_range: fields.node_range.as_ref().map(coordinates),
            admitted: fields.children.is_some(),
            suffix: fields.suffix.as_ref().map(coordinates),
        }
    }

    pub(super) const fn object_indices(&self) -> [u64; 2] {
        self.object.indices()
    }

    pub(super) const fn child_indices(&self, ordinal: u64) -> [u64; 3] {
        let [section, object] = self.object_indices();
        [section, object, ordinal]
    }

    pub(super) fn header(
        &self,
        input: &MediaOriginInput,
        has_issue: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let object = prepared.capture([self.object.route()])?;
        self.object.indices_origin(&mut prepared, object)?;
        self.header_words(&mut prepared)?;
        let admitted = prepared.capture(self.admission())?;
        self.header_ranges(&mut prepared, &admitted)?;
        let layout = prepared.capture(self.admission().chain(self.suffix_site()))?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    fn header_words(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let parent =
            prepared.capture(
                self.base()
                    .into_iter()
                    .chain(self.effects.as_ref().map(|span| {
                        original(span, "containerEffectCount", SourceUseRole::Selector)
                    }))
                    .chain(self.metadata.as_ref().map(|span| {
                        original(span, "containerMetadataCount", SourceUseRole::Selector)
                    }))
                    .chain(
                        self.parent.as_ref().map(|span| {
                            original(span, "containerParentId", SourceUseRole::Payload)
                        }),
                    ),
            )?;
        prepared.field("/data/payload/parent_id", parent)?;
        for (path, source, label, role) in [
            (
                "/data/payload/mode_code",
                &self.mode,
                "containerMode",
                SourceUseRole::Payload,
            ),
            (
                "/data/payload/container_kind",
                &self.mode,
                "containerMode",
                SourceUseRole::Conversion,
            ),
            (
                "/data/payload/child_count",
                &self.count,
                "containerChildCount",
                SourceUseRole::Payload,
            ),
        ] {
            let evidence = prepared.capture(
                self.node_route()
                    .chain(source.as_ref().map(|span| original(span, label, role))),
            )?;
            prepared.field(path, evidence)?;
        }
        Ok(())
    }

    fn header_ranges(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        admitted: &OriginContributions,
    ) -> Result<(), MediaError> {
        let node_start = prepared.capture(self.base())?;
        let node_end = prepared.capture(self.node_route())?;
        if self.node_range.is_some() {
            prepared.range("/data/payload/node_range", &node_start, &node_end)?;
        } else {
            prepared.field("/data/payload/node_range", node_end)?;
        }
        if self.admitted {
            prepared.range("/data/payload/children_range", admitted, admitted)?;
        } else {
            prepared.field("/data/payload/children_range", admitted.clone())?;
        }
        if self.suffix.is_some() {
            prepared.range("/data/payload/suffix_range", admitted, admitted)
        } else {
            prepared.field("/data/payload/suffix_range", admitted.clone())
        }
    }

    pub(super) fn child(
        &self,
        input: &MediaOriginInput,
        child: &BankContainerChild,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let indices = self.child_indices(u64::from(child.ordinal));
        let route = (
            OriginSite::new("bnkHircContainer", &indices, "childRouting"),
            SourceUseRole::Selector,
        );
        let selection = self.admission().chain([route]);
        let position = prepared.capture(selection.clone())?;
        self.object
            .indices_origin(&mut prepared, position.clone())?;
        let object = prepared.capture([self.object.route()])?;
        prepared.field("/subresource/indices/1", object)?;
        prepared.field("/data/payload/ordinal", position.clone())?;
        prepared.range("/data/payload/id_range", &position, &position)?;
        let coordinates = coordinates(&child.child_id.range);
        let value = prepared.capture(selection.chain([original(
            &coordinates,
            "containerChildId",
            SourceUseRole::Payload,
        )]))?;
        prepared.field("/data/payload/child_id", value)?;
        Ok(prepared.finish())
    }

    const fn base(&self) -> [EvidenceSite<'_>; 5] {
        let [route, choice, kind, version] = self.object.versioned(&self.version);
        [
            route,
            choice,
            kind,
            version,
            self.object.extent(SourceUseRole::Selector),
        ]
    }

    fn node_route(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.base()
            .into_iter()
            .chain(self.node_range.as_ref().map(|span| {
                (
                    OriginSite::new("bnkHircContainer", span, "checkedNodeRange"),
                    SourceUseRole::Selector,
                )
            }))
    }

    fn admission(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.node_route().chain(
            self.count
                .as_ref()
                .map(|span| original(span, "containerChildCount", SourceUseRole::Selector)),
        )
    }

    fn suffix_site(&self) -> Option<EvidenceSite<'_>> {
        self.suffix.as_ref().map(|range| {
            (
                OriginSite::new("bnkHircContainer", range, "checkedSuffixLayout"),
                SourceUseRole::Selector,
            )
        })
    }
}

const fn word<T>(field: &BankHircField<T>) -> [u64; 2] {
    coordinates(&field.range)
}

const fn coordinates(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}

const fn original<'a>(
    span: &'a [u64; 2],
    label: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'a> {
    (OriginSite::new("bnkHircField", span, label), role)
}
