// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldViolation};
use crate::{CatalogError, ExportAccountingError};

const ACTION_BASE: &str = "MissionFlowActionBase";
pub(crate) const RANDOM_ENTRY_TYPE: &str = "MissionRandomActionEntry";

#[derive(Clone, Copy)]
pub(crate) enum ActionLayout {
    StateChange,
    TokenStateChange,
    PickRandomMissionPhase,
    MultipleActions,
    PickRandomAction,
    Unknown,
}

impl ActionLayout {
    pub(crate) fn of(name: &str) -> Self {
        match name.strip_prefix("MissionFlowAction_").unwrap_or(name) {
            "StateChange" => Self::StateChange,
            "TokenStateChange" => Self::TokenStateChange,
            "PickRandomMissionPhase" => Self::PickRandomMissionPhase,
            "MultipleActions" => Self::MultipleActions,
            "PickRandomAction" => Self::PickRandomAction,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn wrapper_field(self) -> Option<ActionField> {
        match self {
            Self::MultipleActions => Some(MULTIPLE_ACTIONS),
            Self::PickRandomAction => Some(RANDOM_ACTIONS),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ActionField {
    name: &'static str,
    data_type: DataType,
    array: bool,
    target: Option<&'static str>,
    expected: &'static str,
}

const MULTIPLE_ACTIONS: ActionField = ActionField {
    name: "actions",
    data_type: DataType::StrongPointer,
    array: true,
    target: Some(ACTION_BASE),
    expected: "an ordered StrongPointer action array",
};

const RANDOM_ACTIONS: ActionField = ActionField {
    name: "actions",
    data_type: DataType::Class,
    array: true,
    target: Some(RANDOM_ENTRY_TYPE),
    expected: "an ordered Class array of MissionRandomActionEntry",
};

pub(crate) const ENTRY_ACTION: ActionField = ActionField {
    name: "action",
    data_type: DataType::StrongPointer,
    array: false,
    target: Some(ACTION_BASE),
    expected: "a nullable StrongPointer action",
};

pub(crate) const ENTRY_PROBABILITY: ActionField = ActionField {
    name: "probability",
    data_type: DataType::Single,
    array: false,
    target: None,
    expected: "a Single probability",
};

impl ActionField {
    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) fn original<'fields, 'scope, 'read, 'source>(
        self,
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        owner: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Option<&'fields OriginalValueFact<'scope, 'read, 'source>>, CatalogError> {
        Ok(fields
            .field(Some(owner), self.name)?
            .map(|(_, field)| field))
    }

    pub(crate) fn allows<'scope, 'read, 'source>(
        self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<bool, CatalogError> {
        Ok(self.storage_matches(fact) && self.target_matches(fields, fact)?)
    }

    fn target_matches<'scope, 'read, 'source>(
        self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<bool, CatalogError> {
        self.target.map_or_else(
            || Ok(true),
            |expected| Ok(fields.declared_target_name(fact)? == Some(expected)),
        )
    }

    fn storage_matches(self, fact: &OriginalValueFact<'_, '_, '_>) -> bool {
        let storage = fact.field().storage();
        storage.data_type() == self.data_type
            && storage.is_array() == self.array
            && self.value_shape(fact.value())
    }

    const fn value_shape(self, value: &Value<'_>) -> bool {
        matches!(
            (self.array, self.data_type, value),
            (_, _, Value::Null | Value::Error(_))
                | (true, _, Value::Array(_))
                | (false, DataType::StrongPointer, Value::Struct(_))
                | (false, DataType::Single, Value::Single(_))
        )
    }

    pub(crate) fn failure<'scope, 'read, 'source>(
        self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        owner: &OriginalValueFact<'scope, 'read, 'source>,
        capture: &dyn FieldCapture<'source>,
    ) -> Result<Option<ReadError>, CatalogError> {
        let Some(field) = self.original(fields, owner)? else {
            return Ok(None);
        };
        let storage_matches = self.storage_matches(field);
        if storage_matches && self.target_matches(fields, field)? {
            return Ok(None);
        }
        let Value::Struct(source) = owner.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let path = fields.logical_path(owner)?;
        let violation = if storage_matches {
            FieldViolation::unsupported(source, &path, field.field())
        } else {
            FieldViolation::invalid(source, &path, self.name, field.value(), self.expected)
        };
        let body = capture.field_failure(&violation)?;
        Ok(Some(Box::new(violation.into_error().with_body(body))))
    }
}
