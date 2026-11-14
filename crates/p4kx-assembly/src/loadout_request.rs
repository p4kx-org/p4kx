// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutPath;

/// The current caller's genuine inputs, separate from cached document operations.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum LoadoutRequest {
    /// A Record, Include or explicit configuration requests an XML Loadout.
    Xml {
        /// The whole actual requested path.
        path: LoadoutPath,
    },
    /// An Entity or explicit configuration requests a vehicle and its Modification.
    Vehicle {
        /// The actual definition getter.
        definition: LoadoutPath,
        /// The actual Modification getter.
        modification: LoadoutPath,
        /// The actual Record name before the class-name fallback, when observed.
        class_name: Option<LoadoutPath>,
    },
}
