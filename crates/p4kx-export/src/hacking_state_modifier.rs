// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use super::STATE;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::WeakLink;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The authored interaction states selected by three hacking outcomes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct HackingStateModifier<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    on_hacking_error_state: WeakLink<'source>,
    on_hacking_reset_state: WeakLink<'source>,
    on_hacking_success_state: WeakLink<'source>,
}

impl<'source> HackingStateModifier<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let on_hacking_error_state = source.weak_link_to("onHackingErrorState", STATE, notes)?;
        let on_hacking_reset_state = source.weak_link_to("onHackingResetState", STATE, notes)?;
        let on_hacking_success_state =
            source.weak_link_to("onHackingSuccessState", STATE, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("onHackingErrorState", on_hacking_error_state.state()),
                ("onHackingResetState", on_hacking_reset_state.state()),
                ("onHackingSuccessState", on_hacking_success_state.state()),
            ],
        )?;
        Ok(Self {
            source,
            on_hacking_error_state,
            on_hacking_reset_state,
            on_hacking_success_state,
        })
    }
}
