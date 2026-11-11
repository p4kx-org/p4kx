// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, Value};

use crate::CatalogError;
use crate::broker_values::{Month, RepeatType};
use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_context_value::{TemplateVocabulary, TemplateVocabularyProjection};
use crate::template_extras::EndReason;
use crate::template_issues::Unknowns;
use crate::template_modifier::{LicenseType, Reaction};
use crate::template_objective::{CommsTiming, ObjectiveCategory, ObjectiveState};
use crate::template_objective_plan::TemplateObjectiveSelection;
use crate::template_source_members::TemplateMembers;

#[derive(Clone, Copy)]
enum Vocabulary {
    State,
    Category,
    Timing,
    End,
    Reaction,
    License,
    Month,
    RepeatType,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    objectives: &[TemplateObjectiveSelection<'fields, 'scope, 'read, 'source>],
    unknowns: &Unknowns,
    owner: FlowOwnerKind,
) -> Result<Vec<TemplateVocabularyProjection>, CatalogError> {
    let mut rows = Vec::new();
    let mut reader = VocabularyReader {
        members,
        unknowns,
        rows: &mut rows,
    };
    let fields = members.fields();
    if matches!(owner, FlowOwnerKind::ContractTemplate) {
        if let Some((_, class)) = fields.field(None, "contractClass")?
            && let Some(class) = fields.first_struct(class)?
            && let Some(finish) = fields.selected_struct(class, "autoFinishSettings")?
            && let Some(deadline) = fields.selected_struct(finish, "contractDeadline")?
        {
            reader.field(
                deadline,
                "missionResultAfterTimerEnd",
                "timer end result",
                Vocabulary::State,
            )?;
        }
        if let Some((_, comms)) = fields.field(None, "contractEndCommsNotifications")? {
            for comms in fields.structs(comms)? {
                reader.field(comms, "reason", "end comms reason", Vocabulary::End)?;
            }
        }
    }
    for objective in objectives.iter().filter(|objective| objective.selected) {
        reader.objective(objective.fact)?;
    }
    if matches!(owner, FlowOwnerKind::ContractTemplate)
        && let Some((_, modifiers)) = fields.field(None, "modifiers")?
    {
        for modifier in fields.structs(modifiers)? {
            reader.modifier(modifier)?;
        }
    }
    Ok(rows)
}

pub(crate) fn broker_prefix(
    members: &TemplateMembers<'_, '_, '_, '_>,
    unknowns: &Unknowns,
) -> Result<Vec<TemplateVocabularyProjection>, CatalogError> {
    let mut rows = Vec::new();
    let mut reader = VocabularyReader {
        members,
        unknowns,
        rows: &mut rows,
    };
    let fields = members.fields();
    if let Some((_, schedule)) = fields.field(None, "availableDateSchedule")? {
        for window in fields.structs(schedule)? {
            for moment in ["start", "end"] {
                if let Some(moment) = fields.selected_struct(window, moment)?
                    && let Some(date) = fields.selected_struct(moment, "date")?
                {
                    reader.field(date, "month", "month", Vocabulary::Month)?;
                }
            }
            if let Some(date) = fields.selected_struct(window, "repeatEnd")? {
                reader.field(date, "month", "month", Vocabulary::Month)?;
            }
            reader.field(window, "repeatType", "repeat type", Vocabulary::RepeatType)?;
        }
    }
    if let Some((_, deadline)) = fields.field(None, "missionDeadline")?
        && let Some(deadline) = fields.first_struct(deadline)?
    {
        reader.field(
            deadline,
            "missionResultAfterTimerEnd",
            "timer end result",
            Vocabulary::State,
        )?;
    }
    Ok(rows)
}

pub(crate) fn broker_modifier<'fields, 'scope, 'read, 'source>(
    members: &TemplateMembers<'fields, 'scope, 'read, 'source>,
    fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    unknowns: &Unknowns,
    rows: &mut Vec<TemplateVocabularyProjection>,
) -> Result<(), CatalogError> {
    VocabularyReader {
        members,
        unknowns,
        rows,
    }
    .modifier(fact)
}

struct VocabularyReader<'reader, 'fields, 'scope, 'read, 'source> {
    members: &'reader TemplateMembers<'fields, 'scope, 'read, 'source>,
    unknowns: &'reader Unknowns,
    rows: &'reader mut Vec<TemplateVocabularyProjection>,
}

impl<'fields, 'scope, 'read, 'source> VocabularyReader<'_, 'fields, 'scope, 'read, 'source> {
    fn field(
        &mut self,
        parent: &'fields OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
        what: &str,
        vocabulary: Vocabulary,
    ) -> Result<(), CatalogError> {
        let Some((_, fact)) = self.members.fields().field(Some(parent), name)? else {
            return Ok(());
        };
        let text = match fact.value() {
            Value::String(value) | Value::Enum(value) if !value.is_empty() => Some(*value),
            _ => None,
        };
        let meaning = match vocabulary {
            Vocabulary::State => TemplateVocabulary::ObjectiveState {
                value: self.unknowns.parse(what, text, ObjectiveState::parse),
            },
            Vocabulary::Category => TemplateVocabulary::ObjectiveCategory {
                value: self.unknowns.parse(what, text, ObjectiveCategory::parse),
            },
            Vocabulary::Timing => TemplateVocabulary::CommsTiming {
                value: self.unknowns.parse(what, text, CommsTiming::parse),
            },
            Vocabulary::End => TemplateVocabulary::EndReason {
                value: self.unknowns.parse(what, text, EndReason::parse),
            },
            Vocabulary::Reaction => TemplateVocabulary::Reaction {
                value: self.unknowns.parse(what, text, Reaction::parse),
            },
            Vocabulary::License => TemplateVocabulary::LicenseType {
                value: self.unknowns.parse(what, text, LicenseType::parse),
            },
            Vocabulary::Month => TemplateVocabulary::Month {
                value: self.unknowns.parse(what, text, Month::parse),
            },
            Vocabulary::RepeatType => TemplateVocabulary::RepeatType {
                value: self.unknowns.parse(what, text, RepeatType::parse),
            },
        };
        let readable = !matches!(fact.value(), Value::Error(_));
        let state = if readable {
            SourceValue::Present(meaning)
        } else {
            SourceValue::Unreadable
        };
        let mut value = fact.bind(
            OriginalValuePart::Value,
            state,
            if readable {
                SourceUseRole::Payload
            } else {
                SourceUseRole::Attempted
            },
            readable,
        )?;
        value.contribute(fact.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
        self.rows.push(TemplateVocabularyProjection {
            source: self.members.value_ref(fact)?,
            meaning: value.into_cell(self.members.fields().sources()),
        });
        Ok(())
    }

    fn display(
        &mut self,
        parent: &'fields OriginalValueFact<'scope, 'read, 'source>,
        name: &str,
    ) -> Result<(), CatalogError> {
        if let Some(display) = self.members.fields().selected_struct(parent, name)? {
            self.field(
                display,
                "category",
                "objective category",
                Vocabulary::Category,
            )?;
        }
        Ok(())
    }

    fn objective(
        &mut self,
        objective: &'fields OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        self.display(objective, "displayInfo")?;
        if let Some(handler) = self
            .members
            .fields()
            .selected_struct(objective, "objectiveHandler")?
        {
            self.display(handler, "travelObjectiveInfo")?;
            self.display(handler, "returnObjectiveInfo")?;
            if let Some(settings) = self
                .members
                .fields()
                .selected_struct(handler, "objectiveSettings")?
            {
                for name in [
                    "andObjective",
                    "orObjective",
                    "collectObjective",
                    "collectAndDeliverObjective",
                    "resourceDeliverObjective",
                    "itemDeliverObjective",
                    "listItemObjective",
                ] {
                    self.display(settings, name)?;
                }
            }
        }
        if let Some((_, comms)) = self
            .members
            .fields()
            .field(Some(objective), "commsNotifications")?
        {
            for comms in self.members.fields().structs(comms)? {
                self.field(comms, "trigger", "comms trigger", Vocabulary::State)?;
                self.field(comms, "commsTiming", "comms timing", Vocabulary::Timing)?;
            }
        }
        Ok(())
    }

    fn modifier(
        &mut self,
        modifier: &'fields OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        let Value::Struct(source) = modifier.value() else {
            return Ok(());
        };
        match source
            .type_name()
            .strip_prefix("MissionModifier_")
            .unwrap_or_else(|| source.type_name())
        {
            "FactionHostility" => {
                self.field(modifier, "myReaction", "reaction", Vocabulary::Reaction)?;
                self.field(modifier, "theirReaction", "reaction", Vocabulary::Reaction)?;
            }
            "LawLicense" => {
                self.field(modifier, "licenseType", "license type", Vocabulary::License)?;
            }
            _ => {}
        }
        Ok(())
    }
}
