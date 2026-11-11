// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{MAX_TEXT_DEPTH, SourceDocument, SourceElement, SourceReadOptions};

use super::{AudioFileError, MAX_ATL_NODES, wwise_id};

/// A Wwise event an ATL trigger posts: its name and its short ID.
///
/// This is the definition's reference to the event by name; the Wwise event of the glossary is
/// the Event object in a soundbank's `HIRC` section that the short ID finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WwiseEvent {
    name: Box<str>,
    id: u32,
    localised: bool,
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
}

/// An audio trigger of an ATL definition: its name, the Wwise events it posts and the Entry that
/// defines it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlTrigger {
    pub(crate) name: Box<str>,
    pub(crate) events: Box<[WwiseEvent]>,
    pub(crate) file: u32,
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
}

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

fn triggers(root: SourceElement<'_>, file: u32) -> Vec<AtlTrigger> {
    if root.name() != "ATLConfig" {
        return Vec::new();
    }
    root.elements()
        .filter(|list| list.name() == "AudioTriggers")
        .flat_map(SourceElement::elements)
        .filter(|trigger| trigger.name() == "ATLTrigger")
        .filter_map(|trigger| {
            let name = trigger
                .attribute("atl_name")
                .filter(|name| !name.is_empty())?;
            Some(AtlTrigger {
                name: name.into(),
                events: events(trigger),
                file,
            })
        })
        .collect()
}

fn events(trigger: SourceElement<'_>) -> Box<[WwiseEvent]> {
    trigger
        .elements()
        .filter(|event| event.name() == "WwiseEvent")
        .filter_map(|event| {
            let name = event
                .attribute("wwise_name")
                .filter(|name| !name.is_empty())?;
            Some(WwiseEvent {
                name: name.into(),
                id: wwise_id(name),
                localised: event
                    .attribute("wwise_localised")
                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
            })
        })
        .collect()
}
