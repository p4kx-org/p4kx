// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{CancelToken, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::parameter::{self, FlareField, FlareNodeRole};
use crate::catalog_read::authored_xml_tree::{self, AuthoredXmlElement};
use crate::catalog_read::loose_archive_path::Resolver;
use crate::catalog_read::original::EntrySourceXml;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// One authored element and its supported flare parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FlareElement {
    authored: AuthoredXmlElement,
    role: SourcedValue<FlareNodeRole>,
    fields: Vec<FlareField>,
}

impl FlareElement {
    /// Borrows the original element, including unrecognised declarations and text.
    #[must_use]
    pub const fn authored(&self) -> &AuthoredXmlElement {
        &self.authored
    }

    /// Borrows the structural classification and its source evidence.
    #[must_use]
    pub const fn role(&self) -> &SourcedValue<FlareNodeRole> {
        &self.role
    }

    /// Returns the typed fields declared by this element's structural family.
    #[must_use]
    pub fn fields(&self) -> &[FlareField] {
        &self.fields
    }
}

struct Projection {
    role: SourcedValue<FlareNodeRole>,
    fields: Vec<FlareField>,
}

pub(super) fn read(
    xml: &EntrySourceXml<'_>,
    cancel: Option<&CancelToken>,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<Vec<FlareElement>, CatalogError> {
    let mut projections: Vec<Projection> = Vec::new();
    let authored = authored_xml_tree::visit(xml, cancel, |source, element| {
        let role = classify(xml, source, element, &projections)?;
        let fields = parameter::read(xml, element, &role, resolver)?;
        projections.push(Projection { role, fields });
        Ok(())
    })?;
    Ok(authored
        .into_iter()
        .zip(projections)
        .map(|(authored, Projection { role, fields })| FlareElement {
            authored,
            role,
            fields,
        })
        .collect())
}

fn classify(
    xml: &EntrySourceXml<'_>,
    source: SourceElement<'_>,
    element: &AuthoredXmlElement,
    projections: &[Projection],
) -> Result<SourcedValue<FlareNodeRole>, CatalogError> {
    let parent = element
        .parent()
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let sources = xml.sources();
    let name_origin = element.name().origin();
    let placement_origin = element.parent().origin();
    let Some(index) = parent else {
        return Ok(sources.value_with_origins(
            SourceValue::Present(FlareNodeRole::Library),
            &[name_origin, placement_origin],
        )?);
    };
    let index =
        usize::try_from(*index).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    let parent = &projections
        .get(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?
        .role;
    let parent_role = parent
        .value()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let role = if source.namespace().is_none() {
        parent_role.child(source.name())
    } else {
        FlareNodeRole::Other
    };
    Ok(sources.value_with_origins(
        SourceValue::Present(role),
        &[name_origin, placement_origin, parent.origin()],
    )?)
}
