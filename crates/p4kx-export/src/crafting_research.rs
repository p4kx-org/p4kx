// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, FieldStorage, StructValue, Value, ValueError};

pub(crate) const RESEARCH_TYPE: &str = "CraftingResearch";
pub(crate) const COSTS_BASE: &str = "CraftingRecipeCosts_Base";
pub(crate) const UNLOCK_BASE: &str = "CraftingResearchUnlock_Base";

/// The actual original research envelope of one tier.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Research<'source> {
    /// The tier declares no research field.
    Absent,
    /// The tier stores a null research pointer.
    Null,
    /// The tier stores a readable `CraftingResearch` envelope.
    Present(ResearchBlock<'source>),
    /// The tier stores a shape outside the known envelope.
    Unsupported(Option<&'source str>),
    /// The declared research value could not be decoded.
    Unreadable(ValueError),
}

/// Both original members of the known research envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResearchBlock<'source> {
    /// The original research cost declaration and value state.
    pub research_costs: ResearchMember<'source>,
    /// The original unlock requirement declaration and value state.
    pub unlock_requirements: ResearchMember<'source>,
}

/// One original research member, including an empty pointer declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResearchMember<'source> {
    /// The actual field storage and target word if the envelope declares this member.
    pub declaration: Option<FieldStorage>,
    /// The actual readable or unsupported state.
    pub value: ResearchValue<'source>,
}

/// Supported states of a research member whose concrete gameplay payload is not established.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResearchValue<'source> {
    /// The source does not declare this field.
    Absent,
    /// The original pointer is empty.
    Null,
    /// The actual populated pointer holds the known fieldless declared base.
    Present,
    /// The original member stores a populated payload outside the known contract.
    Unsupported(Option<&'source str>),
    /// The declared original value could not be decoded.
    Unreadable(ValueError),
}

pub(crate) fn read<'source>(tier: &StructValue<'source>) -> Research<'source> {
    match tier.field("research") {
        None => Research::Absent,
        Some(Value::Null) => Research::Null,
        Some(Value::Error(error)) => Research::Unreadable(*error),
        Some(Value::Struct(source)) if source.type_name() == RESEARCH_TYPE => {
            Research::Present(ResearchBlock {
                research_costs: member(source, "researchCosts", COSTS_BASE),
                unlock_requirements: member(source, "unlockRequirements", UNLOCK_BASE),
            })
        }
        Some(value) => Research::Unsupported(source_type(value)),
    }
}

fn member<'source>(
    source: &StructValue<'source>,
    name: &str,
    expected: &str,
) -> ResearchMember<'source> {
    let Some(field) = source.fields().iter().find(|field| field.name() == name) else {
        return ResearchMember {
            declaration: None,
            value: ResearchValue::Absent,
        };
    };
    let storage = field.storage();
    let supported = storage.data_type() == DataType::StrongPointer && !storage.is_array();
    let value = if supported {
        member_value(field.value(), expected)
    } else {
        match field.value() {
            Value::Error(error) => ResearchValue::Unreadable(*error),
            value => ResearchValue::Unsupported(source_type(value)),
        }
    };
    ResearchMember {
        declaration: Some(storage),
        value,
    }
}

pub(crate) fn member_value<'source>(
    value: &Value<'source>,
    expected: &str,
) -> ResearchValue<'source> {
    match value {
        Value::Null => ResearchValue::Null,
        Value::Error(error) => ResearchValue::Unreadable(*error),
        Value::Struct(source) if source.type_name() == expected && source.fields().is_empty() => {
            ResearchValue::Present
        }
        value => ResearchValue::Unsupported(source_type(value)),
    }
}

const fn source_type<'source>(value: &Value<'source>) -> Option<&'source str> {
    match value {
        Value::Struct(source) => Some(source.type_name()),
        _ => None,
    }
}
