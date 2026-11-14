// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::cryxml::{
    self, SourceAttributePosition, SourceDocument, SourceElement, SourceElementPosition,
    SourceReadError, SourceReadOptions,
};
use p4kx_extraction::{Archive, EntryOrigin, find_entry};

use crate::action_map::{
    ACTION_MAP_PROFILE_PATH, Action, ActionMap, ActionMapError, ActionMaps, Binding, Device,
    MAX_PROFILE_LEN,
};

/// Action maps and the physical coordinates from the same profile parse.
#[derive(Debug)]
pub struct ActionMapProfile {
    maps: ActionMaps,
    pub(crate) origin: Option<EntryOrigin>,
    pub(crate) root: SourceElementPosition,
    pub(crate) sites: Vec<MapSites>,
}

impl ActionMapProfile {
    /// Reads the same selected profile as the action-map exporter.
    ///
    /// # Errors
    /// Returns an error if the profile is missing, exceeds the input limit or cannot be read.
    pub fn read(archive: &Archive) -> Result<Self, ActionMapError> {
        let entry = find_entry(archive, ACTION_MAP_PROFILE_PATH).ok_or(ActionMapError::NotFound)?;
        let size = entry.uncompressed_size();
        if size > MAX_PROFILE_LEN {
            return Err(ActionMapError::TooLarge { size });
        }
        let mut bytes = Vec::new();
        archive
            .open_entry(entry)
            .map_err(ActionMapError::Open)?
            .take(MAX_PROFILE_LEN)
            .read_to_end(&mut bytes)
            .map_err(ActionMapError::Read)?;
        let mut profile = Self::parse(&bytes)?;
        profile.origin = archive.entry_origin(entry);
        Ok(profile)
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ActionMapError> {
        let options =
            SourceReadOptions::default().with_open_text_depth_limit(cryxml::MAX_TEXT_DEPTH);
        let document = SourceDocument::parse(bytes, options).map_err(|error| match error {
            SourceReadError::CryXml(error) => ActionMapError::Xml(error),
            other => ActionMapError::Text(other.to_string()),
        })?;
        let root = document.root();
        let mut maps = Vec::new();
        let mut sites = Vec::new();
        for map in named(root, "actionmap") {
            if let Some((parsed, evidence)) = read_map(map, bytes) {
                maps.push(parsed);
                sites.push(evidence);
            }
        }
        Ok(Self {
            maps: ActionMaps { maps },
            origin: None,
            root: root.position(),
            sites,
        })
    }

    /// Borrows the parsed maps and bindings.
    #[must_use]
    pub const fn maps(&self) -> &ActionMaps {
        &self.maps
    }

    pub(crate) fn into_maps(self) -> ActionMaps {
        self.maps
    }
}

#[derive(Debug)]
pub(crate) struct MapSites {
    pub(crate) node: SourceElementPosition,
    pub(crate) name: AttributeSite,
    pub(crate) label: Option<AttributeSite>,
    pub(crate) actions: Vec<ActionSites>,
}

#[derive(Debug)]
pub(crate) struct ActionSites {
    pub(crate) node: SourceElementPosition,
    pub(crate) name: AttributeSite,
    pub(crate) label: Option<AttributeSite>,
    pub(crate) mode: Option<AttributeSite>,
    pub(crate) bindings: Vec<BindingSites>,
}

#[derive(Debug)]
pub(crate) struct BindingSites {
    pub(crate) input: AttributeSite,
    pub(crate) device: DeviceSite,
    pub(crate) mode: Option<AttributeSite>,
    pub(crate) inherited: bool,
}

#[derive(Debug)]
pub(crate) enum DeviceSite {
    Attribute,
    Element(SourceElementPosition),
}

#[derive(Debug, Clone)]
pub(crate) struct AttributeSite {
    pub(crate) position: SourceAttributePosition,
    pub(crate) converted: bool,
}

struct Attribute<'xml> {
    value: &'xml str,
    site: AttributeSite,
}

fn named<'xml>(
    node: SourceElement<'xml>,
    name: &'static str,
) -> impl Iterator<Item = SourceElement<'xml>> {
    node.elements()
        .filter(move |child| child.name().eq_ignore_ascii_case(name))
}

fn text<'xml>(node: SourceElement<'xml>, name: &str, bytes: &[u8]) -> Option<Attribute<'xml>> {
    let attribute = node
        .attributes()
        .find(|attribute| attribute.name().eq_ignore_ascii_case(name))?;
    let value = attribute.value().trim();
    if value.is_empty() {
        return None;
    }
    let normalized = match attribute.position() {
        SourceAttributePosition::Text(range) => bytes
            .get(range.clone())
            .and_then(|raw| std::str::from_utf8(raw).ok())
            .and_then(|raw| raw.split_once('='))
            .map(|(_, raw)| raw.trim_start())
            .and_then(|raw| raw.get(1..raw.len().saturating_sub(1)))
            .is_some_and(|raw| raw != attribute.value()),
        SourceAttributePosition::CryXml { .. } => true,
        _ => false,
    };
    Some(Attribute {
        value,
        site: AttributeSite {
            position: attribute.position().clone(),
            converted: normalized || value != attribute.value(),
        },
    })
}

fn read_map(node: SourceElement<'_>, bytes: &[u8]) -> Option<(ActionMap, MapSites)> {
    let name = text(node, "name", bytes)?;
    let label = text(node, "UILabel", bytes).or_else(|| text(node, "UILable", bytes));
    let (actions, action_sites) = named(node, "action")
        .filter_map(|action| read_action(action, bytes))
        .unzip();
    Some((
        ActionMap {
            name: name.value.to_owned(),
            label_key: label.as_ref().map(|attribute| attribute.value.to_owned()),
            actions,
        },
        MapSites {
            node: node.position(),
            name: name.site,
            label: label.map(|attribute| attribute.site),
            actions: action_sites,
        },
    ))
}

fn read_action(node: SourceElement<'_>, bytes: &[u8]) -> Option<(Action, ActionSites)> {
    let name = text(node, "name", bytes)?;
    let mode = text(node, "activationMode", bytes);
    let label = text(node, "UILabel", bytes);
    let (bindings, binding_sites) = read_bindings(node, mode.as_ref(), bytes);
    Some((
        Action {
            name: name.value.to_owned(),
            label_key: label.as_ref().map(|attribute| attribute.value.to_owned()),
            activation_mode: mode.as_ref().map(|attribute| attribute.value.to_owned()),
            bindings,
        },
        ActionSites {
            node: node.position(),
            name: name.site,
            label: label.map(|attribute| attribute.site),
            mode: mode.map(|attribute| attribute.site),
            bindings: binding_sites,
        },
    ))
}

fn read_bindings(
    node: SourceElement<'_>,
    mode: Option<&Attribute<'_>>,
    bytes: &[u8],
) -> (Vec<Binding>, Vec<BindingSites>) {
    let (mut bindings, mut binding_sites) = (Vec::new(), Vec::new());
    for device in Device::ALL {
        let mut push =
            |input: Attribute<'_>, device_site, mode: Option<&Attribute<'_>>, inherited| {
                let binding = Binding {
                    device,
                    input: input.value.to_owned(),
                    activation_mode: mode.map(|mode| mode.value.to_owned()),
                };
                if !bindings.contains(&binding) {
                    bindings.push(binding);
                    binding_sites.push(BindingSites {
                        input: input.site,
                        device: device_site,
                        mode: mode.map(|mode| mode.site.clone()),
                        inherited,
                    });
                }
            };
        if let Some(input) = text(node, device.name(), bytes) {
            push(input, DeviceSite::Attribute, mode, false);
        }
        for element in named(node, device.name()) {
            let override_mode = text(element, "activationMode", bytes);
            let chosen_mode = override_mode.as_ref().or(mode);
            if let Some(input) = text(element, "input", bytes) {
                push(
                    input,
                    DeviceSite::Element(element.position()),
                    chosen_mode,
                    override_mode.is_none(),
                );
            }
            for data in named(element, "inputdata") {
                if let Some(input) = text(data, "input", bytes) {
                    push(
                        input,
                        DeviceSite::Element(element.position()),
                        chosen_mode,
                        override_mode.is_none(),
                    );
                }
            }
        }
    }
    (bindings, binding_sites)
}
