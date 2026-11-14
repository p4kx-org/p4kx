// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::audio::AudioIndex;
use p4kx_extraction::Archive;
use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::sound::wwise_id;

use super::TestResult;
use super::support::require;

#[derive(Default)]
pub(super) struct Triggers {
    by_name: HashMap<String, Original>,
}

struct Original {
    position: usize,
    path: String,
    name: String,
    events: Vec<Event>,
}

struct Event {
    name: String,
    localised: bool,
}

impl Triggers {
    pub(super) fn observe(&mut self, path: &str, position: usize, root: SourceElement<'_>) {
        for trigger in root
            .elements()
            .filter(|list| list.name() == "AudioTriggers")
            .flat_map(SourceElement::elements)
            .filter(|element| element.name() == "ATLTrigger")
        {
            let Some(name) = nonempty(trigger.attribute("atl_name")) else {
                continue;
            };
            let slot = self.by_name.entry(name.to_ascii_lowercase());
            match slot {
                std::collections::hash_map::Entry::Occupied(mut slot) => {
                    let existing = slot.get();
                    if path_order(path, &existing.path)
                        .then(position.cmp(&existing.position))
                        .is_lt()
                    {
                        slot.insert(Original::new(path, position, name, trigger));
                    }
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(Original::new(path, position, name, trigger));
                }
            }
        }
    }

    pub(super) fn verify(&self, archive: &Archive, index: &AudioIndex) -> TestResult {
        require(
            !self.by_name.is_empty(),
            "positive installed root trigger coverage",
        )?;
        require(
            self.by_name.len() == index.triggers().len(),
            "unique admitted triggers reconcile",
        )?;
        for trigger in index.triggers() {
            let original = self
                .by_name
                .get(&trigger.name().to_ascii_lowercase())
                .ok_or("admitted trigger has an original definition")?;
            require(
                trigger.name() == original.name,
                "the original trigger spelling survives",
            )?;
            require(
                index.definition_entry_position(trigger) == Some(original.position),
                "the winning trigger comes from its exact physical source",
            )?;
            require(
                archive
                    .entries()
                    .get(original.position)
                    .map(p4kx_extraction::Entry::path)
                    == index.definition_path(trigger),
                "physical definition path agrees",
            )?;
            require(
                trigger.events().len() == original.events.len(),
                "all original events are admitted",
            )?;
            for (actual, expected) in trigger.events().iter().zip(&original.events) {
                require(
                    actual.name() == expected.name
                        && actual.id() == wwise_id(&expected.name)
                        && actual.is_localised() == expected.localised,
                    "event spelling, order, Wwise identity and localisation match",
                )?;
            }
        }
        Ok(())
    }
}

impl Original {
    fn new(path: &str, position: usize, name: &str, trigger: SourceElement<'_>) -> Self {
        let events = trigger
            .elements()
            .filter(|event| event.name() == "WwiseEvent")
            .filter_map(|event| {
                Some(Event {
                    name: nonempty(event.attribute("wwise_name"))?.into(),
                    localised: event
                        .attribute("wwise_localised")
                        .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                })
            })
            .collect();
        Self {
            position,
            path: path.into(),
            name: name.into(),
            events,
        }
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn path_order(left: &str, right: &str) -> std::cmp::Ordering {
    fn canonical(bytes: &str) -> impl Iterator<Item = u8> + '_ {
        bytes.bytes().map(|byte| match byte {
            b'/' => b'\\',
            other => other.to_ascii_lowercase(),
        })
    }
    canonical(left).cmp(canonical(right))
}
