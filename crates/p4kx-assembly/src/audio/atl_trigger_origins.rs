// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUseRole, SourceUses,
};

use super::{AtlTrigger, AudioIndex, WwiseEvent};

/// Original ATL definition coordinates bound to an Archive Entry.
pub struct AtlTriggerOrigins<'a> {
    trigger: &'a AtlTrigger,
    uses: &'a SourceUses,
    payload: Option<SourceScope>,
    complete: bool,
}

impl<'a> AtlTriggerOrigins<'a> {
    /// Binds the selected trigger to its physical source in the original index.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn open(
        archive: &Archive,
        index: &AudioIndex,
        uses: &'a SourceUses,
        trigger: &'a AtlTrigger,
    ) -> Result<Self, SourceEvidenceError> {
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

    /// Registers the authored ATL trigger name in the role used by the caller.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn name(&self, role: SourceUseRole) -> Result<FieldOrigin, SourceEvidenceError> {
        self.payload(
            SourceLocation::xml_attribute(self.trigger.name_position(), "atl_name"),
            role,
        )
    }

    /// Registers the name of an event belonging to this trigger.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn event_name(
        &self,
        event: &WwiseEvent,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        if !self.owns_event(event) {
            return Ok(self.uses.origin(&[], false));
        }
        self.payload(
            SourceLocation::xml_attribute(event.name_position(), "wwise_name"),
            role,
        )
    }

    /// Registers the localisation attribute or its original owning element.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn event_localised(&self, event: &WwiseEvent) -> Result<FieldOrigin, SourceEvidenceError> {
        if !self.owns_event(event) {
            return Ok(self.uses.origin(&[], false));
        }
        let position = event.localised_position().map_or_else(
            || SourceLocation::xml_element(event.position(), "WwiseEvent"),
            |position| SourceLocation::xml_attribute(position, "wwise_localised"),
        );
        self.payload(position, SourceUseRole::Conversion)
    }

    /// Registers the original trigger element defining the event set.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn events(&self) -> Result<FieldOrigin, SourceEvidenceError> {
        self.payload(
            SourceLocation::xml_element(self.trigger.position(), "ATLTrigger"),
            SourceUseRole::Selector,
        )
    }

    /// Registers the original definition path from the Archive directory.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the source.
    pub fn path(&self) -> Result<FieldOrigin, SourceEvidenceError> {
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

    fn owns_event(&self, event: &WwiseEvent) -> bool {
        let original = self.trigger.events();
        let Some(offset) = std::ptr::from_ref(event)
            .addr()
            .checked_sub(original.as_ptr().addr())
        else {
            return false;
        };
        original
            .get(offset / size_of_val(event))
            .is_some_and(|candidate| std::ptr::eq(candidate, event))
    }

    fn payload(
        &self,
        location: Option<SourceLocation>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let Some((location, scope)) = location.zip(self.payload.as_ref()) else {
            return Ok(self.uses.origin(&[], false));
        };
        let site = scope.at(location, role)?;
        Ok(self.uses.origin(&[site], self.complete))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};

    use super::{Archive, AtlTriggerOrigins, AudioIndex, SourceUseRole, SourceUses};

    #[test]
    fn another_triggers_event_cannot_supply_a_source_coordinate() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("Data.p4k");
        let definition = |trigger| {
            TestAtl::new(trigger)
                .trigger(trigger, &["Play_Shared"])
                .to_xml()
        };
        ArchiveBuilder::new()
            .entry(TestEntry::stored(
                "Data/Libs/GameAudio/First.xml",
                definition("Fire_First").into_bytes(),
            ))
            .entry(TestEntry::stored(
                "Data/Libs/GameAudio/Second.xml",
                definition("Fire_Second").into_bytes(),
            ))
            .write_to(&path)?;
        let archive = Archive::open(path)?;
        let index = AudioIndex::build(&archive);
        let trigger = index.trigger("Fire_First").ok_or("selected trigger")?;
        let other = index.trigger("Fire_Second").ok_or("other trigger")?;
        let event = trigger.events().first().ok_or("selected event")?;
        let foreign = other.events().first().ok_or("foreign event")?;
        let uses = SourceUses::with_compact_use_refs();
        let origins = AtlTriggerOrigins::open(&archive, &index, &uses, trigger)?;
        let own = origins.event_name(event, SourceUseRole::Payload)?;
        assert!(
            own.source_set_complete(),
            "the original event has an exact source"
        );
        assert_eq!(
            own.source_uses_refs().len(),
            1,
            "one original authored name"
        );
        for rejected in [
            origins.event_name(foreign, SourceUseRole::Payload)?,
            origins.event_localised(foreign)?,
        ] {
            assert!(
                !rejected.source_set_complete(),
                "equal spelling does not prove ownership"
            );
            assert!(
                rejected.source_uses_refs().is_empty(),
                "the foreign declaration supplies no uses"
            );
        }
        Ok(())
    }
}
