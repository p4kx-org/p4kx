// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{
    MAX_TEXT_DEPTH, SourceAttribute, SourceAttributePosition, SourceDocument, SourceElement,
    SourceElementPosition, SourceReadOptions,
};

use super::{AudioFileError, MAX_ATL_NODES, wwise_id};

/// A Wwise event an ATL trigger posts: its name and its short ID.
///
/// This is the definition's reference to the event by name; the Wwise event of the glossary is
/// the Event object in a soundbank's `HIRC` section that the short ID finds.
#[derive(Debug, Clone)]
pub struct WwiseEvent {
    name: Box<str>,
    id: u32,
    localised: bool,
    position: SourceElementPosition,
    name_position: SourceAttributePosition,
    localised_position: Option<SourceAttributePosition>,
}

impl WwiseEvent {
    /// Returns the event name as the ATL definition writes it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the event's Wwise short ID, the [`wwise_id`] of its name.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns whether the definition marks the event as localised (`wwise_localised`).
    #[must_use]
    pub const fn is_localised(&self) -> bool {
        self.localised
    }

    /// Returns the event element's coordinate in the owning trigger's definition Entry.
    #[must_use]
    pub const fn position(&self) -> &SourceElementPosition {
        &self.position
    }

    /// Returns the exact original `wwise_name` attribute coordinate.
    #[must_use]
    pub const fn name_position(&self) -> &SourceAttributePosition {
        &self.name_position
    }

    /// Returns the original `wwise_localised` coordinate when that attribute is present.
    #[must_use]
    pub const fn localised_position(&self) -> Option<&SourceAttributePosition> {
        self.localised_position.as_ref()
    }
}

impl PartialEq for WwiseEvent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.id == other.id && self.localised == other.localised
    }
}

impl Eq for WwiseEvent {}

/// An audio trigger of an ATL definition: its name, the Wwise events it posts and the Entry that
/// defines it.
#[derive(Debug, Clone)]
pub struct AtlTrigger {
    pub(crate) name: Box<str>,
    pub(crate) events: Box<[WwiseEvent]>,
    pub(crate) file: u32,
    position: SourceElementPosition,
    name_position: SourceAttributePosition,
}

impl AtlTrigger {
    /// Returns the trigger name (`atl_name`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the Wwise events the trigger posts, in definition order.
    #[must_use]
    pub fn events(&self) -> &[WwiseEvent] {
        &self.events
    }

    /// Returns the trigger element's coordinate in its admitted definition Entry.
    #[must_use]
    pub const fn position(&self) -> &SourceElementPosition {
        &self.position
    }

    /// Returns the exact original `atl_name` attribute coordinate.
    #[must_use]
    pub const fn name_position(&self) -> &SourceAttributePosition {
        &self.name_position
    }
}

impl PartialEq for AtlTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.events == other.events && self.file == other.file
    }
}

impl Eq for AtlTrigger {}

/// Reads the audio triggers of an ATL definition in text XML or CryXml.
pub(crate) fn parse(bytes: &[u8], file: u32) -> Result<Vec<AtlTrigger>, AudioFileError> {
    let options = SourceReadOptions::default()
        .with_text_node_limit(MAX_ATL_NODES)
        .with_open_text_depth_limit(MAX_TEXT_DEPTH);
    let source = SourceDocument::parse(bytes, options).map_err(|error| AudioFileError::Atl {
        reason: error.to_string(),
    })?;
    Ok(triggers(source.root(), file))
}

fn attribute<'source>(
    element: SourceElement<'source>,
    name: &str,
) -> Option<SourceAttribute<'source>> {
    element
        .attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
}

fn triggers(root: SourceElement<'_>, file: u32) -> Vec<AtlTrigger> {
    if root.name() != "ATLConfig" {
        return Vec::new();
    }
    root.elements()
        .filter(|list| list.name() == "AudioTriggers")
        .flat_map(SourceElement::elements)
        .filter(|trigger| trigger.name() == "ATLTrigger")
        .filter_map(|trigger| {
            let name = attribute(trigger, "atl_name").filter(|name| !name.value().is_empty())?;
            Some(AtlTrigger {
                name: name.value().into(),
                events: events(trigger),
                file,
                position: trigger.position(),
                name_position: name.position().clone(),
            })
        })
        .collect()
}

fn events(trigger: SourceElement<'_>) -> Box<[WwiseEvent]> {
    trigger
        .elements()
        .filter(|event| event.name() == "WwiseEvent")
        .filter_map(|event| {
            let name = attribute(event, "wwise_name").filter(|name| !name.value().is_empty())?;
            let localised = attribute(event, "wwise_localised");
            Some(WwiseEvent {
                name: name.value().into(),
                id: wwise_id(name.value()),
                localised: localised
                    .as_ref()
                    .is_some_and(|flag| flag.value().eq_ignore_ascii_case("true")),
                position: event.position(),
                name_position: name.position().clone(),
                localised_position: localised.map(|flag| flag.position().clone()),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "atl_position_tests.rs"]
mod position_tests;
