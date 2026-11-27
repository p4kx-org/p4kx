// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::StructValue;
use p4kx_extraction::SourceLocation;

use crate::EntityNode;
use crate::read::text;

const ANIMATED_CHARACTER: &str = "SAnimatedCharacterComponentParams";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnimationDatabase {
    authored: Arc<str>,
    record: u32,
    component: usize,
}

impl AnimationDatabase {
    pub(super) fn location(&self) -> SourceLocation {
        SourceLocation::DataForge {
            record_index: self.record,
            field: format!("Components[{}].animDatabase3P", self.component),
        }
    }

    pub(super) fn authored(&self) -> &str {
        &self.authored
    }
}

pub(crate) fn database_of<'tree, 'forge: 'tree>(
    record: u32,
    mut components: impl Iterator<Item = (usize, &'tree StructValue<'forge>)>,
) -> Option<AnimationDatabase> {
    let (component, value) =
        components.find(|(_, value)| value.type_name() == ANIMATED_CHARACTER)?;
    let authored = text(value, "animDatabase3P");
    (!authored.trim().is_empty()).then(|| AnimationDatabase {
        authored: Arc::from(authored),
        record,
        component,
    })
}

impl EntityNode {
    /// Returns the path of the Mannequin animation database (`.adb`) the Entity's
    /// `SAnimatedCharacterComponentParams` name in `animDatabase3P`, or `None` when it names none.
    #[must_use]
    pub fn animation_database(&self) -> Option<&str> {
        self.animation_database
            .as_ref()
            .map(|database| database.authored().trim())
    }
}
