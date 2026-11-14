// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_objective_plan::TemplateObjectiveSelection;
use crate::template_property_plan::SelectedProperty;
use crate::{CatalogError, ExportAccountingError};

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;

/// Known permissive text lookups selected by the existing Template context algorithms.
pub(crate) fn selected<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    objectives: &[TemplateObjectiveSelection<'_, 'scope, 'read, 'source>],
    properties: &[SelectedProperty<'_, 'scope, 'read, 'source>],
    kind: FlowOwnerKind,
) -> Result<BTreeSet<u64>, CatalogError> {
    let mut selected = BTreeSet::new();
    if matches!(kind, FlowOwnerKind::MissionBrokerEntry) {
        for name in [
            "title",
            "titleHUD",
            "description",
            "missionGiver",
            "commsChannelName",
        ] {
            if let Some((_, fact)) = fields.field(None, name)? {
                selected.insert(fact.ordinal());
            }
        }
        add(
            fields,
            child(fields, None, "missionDeadline")?,
            &["missionEndReason"],
            &mut selected,
        )?;
    }
    for objective in objectives.iter().filter(|objective| objective.selected) {
        fields.check_cancel()?;
        let owner = Some(objective.fact);
        display(fields, child(fields, owner, "displayInfo")?, &mut selected)?;
        add(
            fields,
            child(fields, owner, "overrideMissionDetailsDisplayInfo")?,
            &["titleOverride", "descriptionOverride"],
            &mut selected,
        )?;
        if let Some(handler) = child(fields, owner, "objectiveHandler")? {
            handler_texts(fields, handler, &mut selected)?;
        }
    }
    for property in properties {
        fields.check_cancel()?;
        let Some(value) = property.value else {
            continue;
        };
        if matches!(value.value(), p4kx_extraction::Value::Struct(source)
            if matches!(crate::template_property_kind::PropertyValueKind::read(source.type_name()), Some(crate::template_property_kind::PropertyValueKind::AiName)))
        {
            add(
                fields,
                Some(value),
                &[
                    "characterGivenName",
                    "characterGivenLastName",
                    "characterGivenNickName",
                ],
                &mut selected,
            )?;
        }
    }
    Ok(selected)
}

fn handler_texts<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    handler: &OriginalValueFact<'scope, 'read, 'source>,
    selected: &mut BTreeSet<u64>,
) -> Result<(), CatalogError> {
    for name in ["travelObjectiveInfo", "returnObjectiveInfo"] {
        display(fields, child(fields, Some(handler), name)?, selected)?;
    }
    add(
        fields,
        child(fields, Some(handler), "navPointSpawnInfo")?,
        &["name"],
        selected,
    )?;
    if fields
        .field(Some(handler), "instanceTagIdentifier")?
        .is_some()
    {
        add(
            fields,
            Some(handler),
            &["instanceHUDObjectiveText"],
            selected,
        )?;
    }
    let settings = child(fields, Some(handler), "objectiveSettings")?;
    for name in [
        "andObjective",
        "orObjective",
        "collectObjective",
        "collectAndDeliverObjective",
        "resourceDeliverObjective",
        "itemDeliverObjective",
        "listItemObjective",
    ] {
        display(fields, child(fields, settings, name)?, selected)?;
    }
    Ok(())
}

fn display<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    source: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    selected: &mut BTreeSet<u64>,
) -> Result<(), CatalogError> {
    add(
        fields,
        source,
        &[
            "objectiveMarkerLabel",
            "shortDescription",
            "longDescription",
        ],
        selected,
    )
}

fn add<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    source: Option<&OriginalValueFact<'scope, 'read, 'source>>,
    names: &[&str],
    selected: &mut BTreeSet<u64>,
) -> Result<(), CatalogError> {
    let Some(source) = source else {
        return Ok(());
    };
    for name in names {
        fields.check_cancel()?;
        if let Some((_, fact)) = fields.field(Some(source), name)? {
            selected.insert(fact.ordinal());
        }
    }
    Ok(())
}

fn child<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    source: Option<Fact<'fields, 'scope, 'read, 'source>>,
    name: &str,
) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let Some(source) = source else {
        return Ok(None);
    };
    if !matches!(source.value(), p4kx_extraction::Value::Struct(_)) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    fields
        .field(Some(source), name)?
        .map(|(_, fact)| fields.first_struct(fact))
        .transpose()
        .map(Option::flatten)
}
