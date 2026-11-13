// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::CryXmlError;
use p4kx_extraction::{Archive, ArchiveError};
use thiserror::Error;

use crate::action_map_profile::ActionMapProfile;

/// The path of the default profile that holds the action maps.
pub const ACTION_MAP_PROFILE_PATH: &str = "Data\\Libs\\Config\\defaultProfile.xml";

pub(crate) const MAX_PROFILE_LEN: u64 = 64 << 20;

/// An input device a default binding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Device {
    /// The keyboard.
    Keyboard,
    /// The mouse.
    Mouse,
    /// The gamepad.
    Gamepad,
    /// The joystick.
    Joystick,
}

impl Device {
    /// Every device, in the order bindings are listed.
    pub const ALL: [Self; 4] = [Self::Keyboard, Self::Mouse, Self::Gamepad, Self::Joystick];

    /// Returns the name the profile and the exports use for the device.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Gamepad => "gamepad",
            Self::Joystick => "joystick",
        }
    }
}

/// One default input of an action on one device, such as `lalt+mouse1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The device the input belongs to.
    pub device: Device,
    /// The input as the profile writes it; `+` joins the keys of a combination.
    pub input: String,
    /// The activation mode of this binding: its own, else the one of the action.
    pub activation_mode: Option<String>,
}

/// An action of an action map with its default bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// The action name, such as `v_toggle_mining_mode`.
    pub name: String,
    /// The localisation key of the label, with its leading `@`; `None` when the action has none.
    pub label_key: Option<String>,
    /// The activation mode the action names.
    pub activation_mode: Option<String>,
    /// The default bindings that are set, in the order keyboard, mouse, gamepad, joystick and
    /// within a device in document order.
    pub bindings: Vec<Binding>,
}

/// An action map: a named group of actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionMap {
    /// The action map name, such as `spaceship_general`.
    pub name: String,
    /// The localisation key of the label, with its leading `@`; `None` when the map has none.
    pub label_key: Option<String>,
    /// The actions in document order.
    pub actions: Vec<Action>,
}

/// The action maps of a default profile, in document order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionMaps {
    /// The action maps.
    pub maps: Vec<ActionMap>,
}

impl ActionMaps {
    /// Returns the number of actions in all maps.
    #[must_use]
    pub fn action_count(&self) -> usize {
        self.maps.iter().map(|map| map.actions.len()).sum()
    }
}

/// The action maps cannot be read.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ActionMapError {
    /// The Archive holds no default profile.
    #[error("the Archive holds no {ACTION_MAP_PROFILE_PATH}")]
    NotFound,
    /// The default profile is larger than the limit.
    #[error("{ACTION_MAP_PROFILE_PATH} holds {size} bytes, over the limit of {MAX_PROFILE_LEN}")]
    TooLarge {
        /// The size of the Entry in bytes.
        size: u64,
    },
    /// The default profile cannot be opened, for example because it is encrypted and the key is
    /// missing or wrong.
    #[error("cannot open {ACTION_MAP_PROFILE_PATH}")]
    Open(#[source] ArchiveError),
    /// The default profile cannot be read.
    #[error("cannot read {ACTION_MAP_PROFILE_PATH}")]
    Read(#[source] io::Error),
    /// The default profile is neither text XML nor `CryXml`.
    #[error("{ACTION_MAP_PROFILE_PATH} is not valid XML")]
    Xml(#[source] CryXmlError),
    /// The default profile is text XML that does not parse.
    #[error("{ACTION_MAP_PROFILE_PATH} is not well-formed text XML: {0}")]
    Text(String),
}

/// Reads the action maps of the default profile of `archive`.
///
/// # Errors
///
/// Returns [`ActionMapError`] when the profile is missing, too large, unreadable or not XML.
pub fn read_action_maps(archive: &Archive) -> Result<ActionMaps, ActionMapError> {
    Ok(ActionMapProfile::read(archive)?.into_maps())
}

/// Parses the action maps of the profile `bytes`, text XML or `CryXml`.
///
/// Only the actions inside an `actionmap` element count; an `actionmap` or an `action` without a
/// name is left out. An input that is empty or blank is no
/// binding, and the same device, input and activation mode of one action appear once.
///
/// # Errors
///
/// Returns [`ActionMapError::Xml`] or [`ActionMapError::Text`] when `bytes` are no XML.
pub fn parse_action_maps(bytes: &[u8]) -> Result<ActionMaps, ActionMapError> {
    Ok(ActionMapProfile::parse(bytes)?.into_maps())
}
