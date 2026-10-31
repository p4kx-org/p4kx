// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::StructValue;

use crate::EntityNode;
use crate::read::text;

const ANIMATED_CHARACTER: &str = "SAnimatedCharacterComponentParams";

/// Returns the animation database the components name for the third-person character
/// (`animDatabase3P`), a path below `Data`; `None` when they name none.
pub(crate) fn database_of(components: &[&StructValue<'_>]) -> Option<Arc<str>> {
    let component = components
        .iter()
        .find(|component| component.type_name() == ANIMATED_CHARACTER)?;
    let path = text(component, "animDatabase3P").trim();
    (!path.is_empty()).then(|| Arc::from(path))
}

impl EntityNode {
    /// Returns the path of the Mannequin animation database (`.adb`) the Entity's
    /// `SAnimatedCharacterComponentParams` name in `animDatabase3P`, or `None` when it names none.
    #[must_use]
    pub fn animation_database(&self) -> Option<&str> {
        self.animation_database.as_deref()
    }
}
