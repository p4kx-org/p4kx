// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AtlTrigger, AudioIndex, WwiseEvent};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, SourceInput, SourceLocation, SourceScope, SourceUseRole,
    SourceUses,
};

use crate::forge_origins::EvidenceError;

pub(super) struct Definition<'a> {
    trigger: &'a AtlTrigger,
    uses: &'a SourceUses,
    payload: Option<SourceScope>,
    complete: bool,
}

impl<'a> Definition<'a> {
    pub(super) fn open(
        archive: &Archive,
        index: &AudioIndex,
        uses: &'a SourceUses,
        trigger: &'a AtlTrigger,
    ) -> Result<Self, EvidenceError> {
        let original = index.definition_source(archive, trigger);
        let complete = original.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let payload = original
            .map(|entry| uses.observe(SourceInput::ArchiveView(entry), InputScope::Payload))
            .transpose()?;
        Ok(Self {
            trigger,
            uses,
            payload,
            complete,
        })
    }

    pub(super) fn name(&self, role: SourceUseRole) -> Result<FieldOrigin, EvidenceError> {
        self.payload(
            SourceLocation::xml_attribute(self.trigger.name_position(), "atl_name"),
            role,
        )
    }

    pub(super) fn event_name(
        &self,
        event: &WwiseEvent,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, EvidenceError> {
        self.payload(
            SourceLocation::xml_attribute(event.name_position(), "wwise_name"),
            role,
        )
    }

    pub(super) fn event_localised(&self, event: &WwiseEvent) -> Result<FieldOrigin, EvidenceError> {
        let position = event.localised_position().map_or_else(
            || SourceLocation::xml_element(event.position(), "WwiseEvent"),
            |position| SourceLocation::xml_attribute(position, "wwise_localised"),
        );
        self.payload(position, SourceUseRole::Conversion)
    }

    pub(super) fn events(&self) -> Result<FieldOrigin, EvidenceError> {
        self.payload(
            SourceLocation::xml_element(self.trigger.position(), "ATLTrigger"),
            SourceUseRole::Selector,
        )
    }

    pub(super) fn path(&self) -> Result<FieldOrigin, EvidenceError> {
        let Some(payload) = &self.payload else {
            return Ok(self.uses.origin(&[], false));
        };
        let directory = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
        let path = directory.at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        Ok(self.uses.origin(&[path], self.complete))
    }

    fn payload(
        &self,
        location: Option<SourceLocation>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, EvidenceError> {
        let Some((location, scope)) = location.zip(self.payload.as_ref()) else {
            return Ok(self.uses.origin(&[], false));
        };
        let site = scope.at(location, role)?;
        Ok(self.uses.origin(&[site], self.complete))
    }
}
