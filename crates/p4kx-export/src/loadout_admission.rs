// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, Value};

const DEFAULT_LOADOUT: &str = "SEntityComponentDefaultLoadoutParams";
const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";

/// The authored loadout or port declaration survey for one Item.
pub(crate) struct PortAdmission {
    pub(crate) wanted: bool,
    pub(crate) incomplete: bool,
}

impl PortAdmission {
    /// Reads only the component slots and original port arrays that select this Item.
    pub(crate) fn read(tree: &RecordTree<'_>) -> Self {
        let Some(components) = slots(tree.field("Components")) else {
            return Self {
                wanted: false,
                incomplete: true,
            };
        };
        let mut wanted = false;
        let mut incomplete = false;
        for value in components {
            let component = component(value);
            wanted |= component.wanted();
            match component {
                Component::Ports(ports) => {
                    incomplete |= ports
                        .iter()
                        .any(|port| !matches!(port, Value::Struct(_) | Value::Null));
                }
                Component::Unavailable => incomplete = true,
                Component::Absent | Component::DefaultLoadout => {}
            }
        }
        Self { wanted, incomplete }
    }
}

/// Returns whether the original Item declares a Loadout or names at least one port slot.
pub(crate) fn carries_ports(tree: &RecordTree<'_>) -> bool {
    slots(tree.field("Components"))
        .is_some_and(|components| components.iter().any(|value| component(value).wanted()))
}

enum Component<'tree, 'source> {
    Absent,
    DefaultLoadout,
    Ports(&'tree [Value<'source>]),
    Unavailable,
}

impl Component<'_, '_> {
    const fn wanted(&self) -> bool {
        match self {
            Self::DefaultLoadout => true,
            Self::Ports(ports) => !ports.is_empty(),
            Self::Absent | Self::Unavailable => false,
        }
    }
}

fn component<'tree, 'source>(value: &'tree Value<'source>) -> Component<'tree, 'source> {
    match value {
        Value::Null => Component::Absent,
        Value::Struct(component) => match component.type_name() {
            DEFAULT_LOADOUT => Component::DefaultLoadout,
            PORT_CONTAINER => {
                slots(component.field("Ports")).map_or(Component::Unavailable, Component::Ports)
            }
            _ => Component::Absent,
        },
        _ => Component::Unavailable,
    }
}

fn slots<'tree, 'source>(value: Option<&'tree Value<'source>>) -> Option<&'tree [Value<'source>]> {
    match value {
        None | Some(Value::Null) => Some(&[]),
        Some(Value::Array(values)) => Some(values),
        Some(_) => None,
    }
}
