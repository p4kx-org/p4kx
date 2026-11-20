// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{SocPhysicsField, SocSourceValue};
use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};

use super::{EntrySourceXml, attribute_location};
use crate::CatalogError;

impl EntrySourceXml<'_> {
    pub(crate) fn physics_field<T>(
        &self,
        field: SocPhysicsField<'_, T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let uses = self.physics_field_uses(&field, SourceUseRole::Payload)?;
        let value = match field.into_value() {
            SocSourceValue::Absent => SourceValue::Absent,
            SocSourceValue::Present(value) => SourceValue::Present(value),
            _ => SourceValue::Invalid,
        };
        Ok(self.project(value, &uses))
    }

    pub(crate) fn physics_selector(
        &self,
        field: &SocPhysicsField<'_, &str>,
    ) -> Result<Vec<SourceUse>, CatalogError> {
        self.physics_field_uses(field, SourceUseRole::Selector)
    }

    fn physics_field_uses<T>(
        &self,
        field: &SocPhysicsField<'_, T>,
        role: SourceUseRole,
    ) -> Result<Vec<SourceUse>, CatalogError> {
        let location = self.node_location(field.element(), field.name())?;
        let mut uses = vec![self.entry.payload_at(location, role)?];
        for attribute in field.attributes() {
            let location = attribute_location(attribute.position(), attribute.name())?;
            uses.push(self.entry.payload_at(location, role)?);
        }
        Ok(uses)
    }
}
