// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutError, VehicleImplementation};

use crate::guid_text::CatalogGuid;
use crate::variants::ShipKey;

const PAINTS: &str = "Paints";

/// An item port of paints, in a ship's Record or its vehicle implementation: the tags an item
/// needs to carry to fit it and the tags the port offers, as lower-case tag names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct PaintPort {
    pub(crate) port_tags: Vec<String>,
    pub(crate) required_tags: Vec<String>,
}

impl PaintPort {
    /// Returns the port with the white-space separated tag lists `port_tags` and `required_tags`.
    pub(crate) fn new(port_tags: Option<&str>, required_tags: Option<&str>) -> Self {
        Self {
            port_tags: tags(port_tags),
            required_tags: tags(required_tags),
        }
    }
}

fn tags(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

/// What the paints of a ship are derived from: the paint ports of its Record and of the vehicle
/// implementation the ship base data read.
#[derive(Debug)]
pub(crate) struct VehicleFacts {
    pub(crate) ship: CatalogGuid,
    pub(crate) tags: Vec<String>,
    pub(crate) paint_ports: Vec<PaintPort>,
    pub(crate) implementation: VehiclePorts,
    pub(crate) variant_key: Option<ShipKey>,
}

/// The paint ports of a vehicle implementation and why it was not read in full, if it was not. A
/// file the Archive lacks is a fact of the game data, not an unread file.
#[derive(Debug, Clone, Default)]
pub(crate) struct VehiclePorts {
    pub(crate) ports: Vec<PaintPort>,
    pub(crate) unreadable: Option<String>,
}

/// Returns whether `issue` only says that the Archive lacks a vehicle implementation file or a
/// patch file.
const fn is_missing_file(issue: &LoadoutError) -> bool {
    matches!(issue, LoadoutError::MissingVehicleImplementation { .. })
}

/// Returns the first of `issues` that is more than a file the Archive lacks.
fn first_unread(issues: &[LoadoutError]) -> Option<&LoadoutError> {
    issues.iter().find(|issue| !is_missing_file(issue))
}

impl VehiclePorts {
    /// Returns the paint ports of `vehicle`, the implementation read for a ship with the
    /// Modification applied: its item ports that take `Paints`, with the required tags the patch
    /// leaves them. `vehicle` is `None` when the ship names no implementation or it could not be
    /// read; `definition` is the file the ship names, if any, and `read_issues` are the issues of
    /// reading it, which tell a file the Archive lacks from one that cannot be read.
    pub(crate) fn of(
        vehicle: Option<&VehicleImplementation>,
        definition: Option<&str>,
        read_issues: &[LoadoutError],
    ) -> Self {
        let Some(implementation) = vehicle else {
            let own = || {
                read_issues.iter().filter(|issue| {
                    matches!(
                        issue,
                        LoadoutError::VehicleImplementation { .. }
                            | LoadoutError::MissingVehicleImplementation { .. }
                    )
                })
            };
            let lacks_file = own().next().is_some() && own().all(is_missing_file);
            return Self {
                ports: Vec::new(),
                unreadable: definition.filter(|_| !lacks_file).map(|path| {
                    own().next().map_or_else(
                        || format!("the vehicle implementation {path}: it cannot be read"),
                        ToString::to_string,
                    )
                }),
            };
        };
        Self {
            ports: implementation
                .item_ports()
                .filter_map(|part| part.item_port())
                .filter(|port| {
                    port.types()
                        .iter()
                        .any(|kind| kind.eq_ignore_ascii_case(PAINTS))
                })
                .map(|port| PaintPort::new(Some(port.port_tags()), Some(port.required_tags())))
                .collect(),
            unreadable: first_unread(implementation.issues()).map(ToString::to_string),
        }
    }
}
