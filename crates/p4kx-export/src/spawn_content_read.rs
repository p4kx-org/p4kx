// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use super::{Reader, Spawn};
use crate::record_read::ReadError;
use crate::spawn_model::LocationCount;
use crate::template_property_kind::PropertyValueKind;

impl<'source> Reader<'_, 'source> {
    /// Reads the existing content recipe, returning no content for another source variant.
    ///
    /// # Errors
    /// Returns original context, strict-field, unsupported-condition or serialization failures.
    pub(crate) fn spawn(
        &self,
        value: &StructValue<'source>,
    ) -> Result<Option<Spawn<'source>>, ReadError> {
        Ok(Some(
            match PropertyValueKind::read(self.fields(Some(value)).type_name()?) {
                Some(PropertyValueKind::ShipSpawnDescriptions) => Spawn::Ships(self.ships(value)?),
                Some(PropertyValueKind::NpcSpawnDescriptions) => Spawn::Npcs(self.npcs(value)?),
                Some(PropertyValueKind::EntitySpawnDescriptions) => {
                    Spawn::Entities(self.entities(value)?)
                }
                Some(PropertyValueKind::Location) => {
                    Spawn::Location(self.location(value, LocationCount::One)?)
                }
                Some(PropertyValueKind::Locations) => {
                    Spawn::Location(self.location(value, LocationCount::Many)?)
                }
                _ => return Ok(None),
            },
        ))
    }
}
