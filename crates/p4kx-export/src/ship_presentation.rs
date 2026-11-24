// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{
    OriginalArraySelection, OriginalFlowFields, OriginalValuePart,
};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

#[path = "ship_manufacturers.rs"]
mod manufacturers;

pub use self::manufacturers::{ShipAttachmentManufacturer, ShipVehicleManufacturer};

pub(crate) struct ShipPresentation<'source> {
    pub(crate) vehicle_manufacturer: ShipVehicleManufacturer<'source>,
    pub(crate) attachment_manufacturer: ShipAttachmentManufacturer<'source>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) role: SourcedValue<PublicNameRef>,
    pub(crate) career: SourcedValue<PublicNameRef>,
    pub(crate) size: SourcedValue<crate::resource_number::ResourceNumber>,
}

struct ShipTexts {
    name: SourcedValue<PublicNameRef>,
    description: SourcedValue<GameTextRef>,
    role: SourcedValue<PublicNameRef>,
    career: SourcedValue<PublicNameRef>,
}

pub(super) fn read<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    vehicle: Option<&Value<'source>>,
    selection: &OriginalArraySelection,
    attachment: Option<&Value<'source>>,
    attachment_selection: &OriginalArraySelection,
) -> Result<(ShipPresentation<'source>, Vec<FieldError>), CatalogError> {
    let originals = fields.originals;
    let mut notes = Vec::new();
    let texts = match vehicle {
        Some(selected) => read_component_texts(
            fields.at_owner(originals.fact(selected)?),
            selection,
            projection,
            &mut notes,
        )?,
        None => ShipTexts {
            name: missing_component(originals, selection),
            description: missing_component(originals, selection),
            role: missing_component(originals, selection),
            career: missing_component(originals, selection),
        },
    };
    let vehicle_manufacturer = manufacturers::vehicle(fields, vehicle, selection, &mut notes)?;
    let attachment =
        manufacturers::attachment(fields, attachment, attachment_selection, &mut notes)?;
    Ok((
        ShipPresentation {
            vehicle_manufacturer,
            attachment_manufacturer: attachment.manufacturer,
            name: texts.name,
            description: texts.description,
            role: texts.role,
            career: texts.career,
            size: attachment.size,
        },
        notes,
    ))
}

pub(super) fn read_exosuit_name<'source>(
    attachment: Observed<'_, '_, '_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
    read_owned_name(attachment, "AttachDef", notes, |definition, notes| {
        read_owned_name(definition, "Localization", notes, |localization, notes| {
            localization.public_name("Name", projection, notes)
        })
    })
}

fn read_owned_name<'reader, 'scope, 'read, 'source>(
    owner: Observed<'reader, 'scope, 'read, 'source>,
    field: &str,
    notes: &mut Vec<FieldError>,
    read: impl FnOnce(
        Observed<'reader, 'scope, 'read, 'source>,
        &mut Vec<FieldError>,
    ) -> Result<SourcedValue<PublicNameRef>, CatalogError>,
) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
    let mut name = None;
    let parent = owner.declared_owned(field, notes, |child, notes| {
        name = Some(read(child, notes)?);
        Ok(())
    })?;
    match name {
        Some(name) => Ok(owner
            .originals
            .sources()
            .add_value_origins(name, &[parent.origin()])?),
        None => parent.try_map(|()| Err(ExportAccountingError::ConflictingSourceDatum.into())),
    }
}

fn missing_component<T>(
    originals: &OriginalFlowFields<'_, '_, '_>,
    selection: &OriginalArraySelection,
) -> SourcedValue<T> {
    let state = if selection.complete {
        SourceValue::Absent
    } else {
        SourceValue::Unreadable
    };
    originals
        .sources()
        .value(state, &selection.uses, selection.complete)
}

fn read_component_texts<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selection: &OriginalArraySelection,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ShipTexts, CatalogError> {
    Ok(ShipTexts {
        name: fields.ship_locale("vehicleName", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
        description: fields.ship_locale("vehicleDescription", selection, notes, |fact| {
            projection.protected_locale_fact(fact)
        })?,
        role: fields.ship_locale("vehicleRole", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
        career: fields.ship_locale("vehicleCareer", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
    })
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    fn ship_locale<T>(
        &self,
        name: &str,
        selection: &OriginalArraySelection,
        notes: &mut Vec<FieldError>,
        retain: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let sources = self.originals.sources();
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            let mut uses = selection.uses.clone();
            uses.push(self.originals.survey(self.owner)?);
            return Ok(sources.value(SourceValue::Absent, &uses, selection.complete));
        };
        let (state, role) = match fact.value() {
            Value::Locale("") => (SourceValue::Absent, SourceUseRole::Selector),
            Value::Locale(_) => (
                SourceValue::Present(retain(fact)?),
                SourceUseRole::Conversion,
            ),
            Value::Null => (SourceValue::Null, SourceUseRole::Selector),
            value => (
                self.failure(name, value, "a nullable Locale", notes)?,
                SourceUseRole::Selector,
            ),
        };
        let mut cell = fact.bind(OriginalValuePart::Value, state, role, selection.complete)?;
        cell.contribute(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        for source in &selection.uses {
            cell.contribute(source.clone());
        }
        Ok(cell.into_cell(sources))
    }
}
