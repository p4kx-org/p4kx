// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Reference, StructValue, Value as SourceValue, ValueError};
use serde_json::Value;

use super::OracleResult;
use super::error::{self, Failure, SourceFault};
use super::source;

const fn fault(category: SourceFault) -> Failure {
    Failure::SourceCompare(category)
}

pub(super) fn check(source: &[SourceValue<'_>], actual: &Value, owner: u32) -> OracleResult<usize> {
    let actual = actual
        .as_array()
        .ok_or(fault(SourceFault::PropertyArrayShape))?;
    require_equal!(
        actual.len(),
        source.len(),
        fault(SourceFault::DeclarationCount)
    );
    let mut scoped = 0_usize;
    let declarations = source.iter().zip(actual);
    for (source, actual) in declarations {
        let Some(source) = source::structure(source)? else {
            error::require(actual.is_null(), fault(SourceFault::NullDeclaration))?;
            continue;
        };
        for (field, key) in [
            ("missionVariableName", "variable"),
            ("extendedTextToken", "token"),
        ] {
            match source.field(field) {
                None | Some(SourceValue::Null) => {
                    error::require(
                        actual[key].is_null(),
                        fault(SourceFault::DeclarationTextValue),
                    )?;
                }
                Some(SourceValue::String(text)) => {
                    require_equal!(actual[key], *text, fault(SourceFault::DeclarationTextValue));
                }
                _ => return Err(fault(SourceFault::DeclarationTextShape)),
            }
        }
        match source
            .field("value")
            .map(source::structure)
            .transpose()?
            .flatten()
        {
            None => error::require(
                actual["value"].is_null() && actual["valueType"].is_null(),
                fault(SourceFault::ValuePresence),
            )?,
            Some(value) => {
                let source_type = value
                    .type_name()
                    .strip_prefix("MissionPropertyValue_")
                    .ok_or(fault(SourceFault::ValueType))?;
                require_equal!(
                    actual["valueType"],
                    source_type,
                    fault(SourceFault::ValueType)
                );
                if source_type == "ScopedLocation" {
                    check_scoped(value, &actual["value"], owner)?;
                    scoped += 1;
                }
            }
        }
    }
    Ok(scoped)
}

fn check_scoped(source: &StructValue<'_>, actual: &Value, owner: u32) -> OracleResult {
    require_equal!(
        actual["type"],
        "scopedLocation",
        fault(SourceFault::ScopedType)
    );
    for field in [
        "factionFilter",
        "locationClassFilter",
        "locationSizeFilter",
        "locationFunctionFilter",
        "otherLocationFilter",
        "relativeLocationFilter",
    ] {
        let actual = actual
            .get(field)
            .ok_or(fault(SourceFault::FilterFieldMissing))?;
        let source = source
            .field(field)
            .map(source::structure)
            .transpose()?
            .flatten();
        let Some(source) = source else {
            error::require(actual.is_null(), fault(SourceFault::FilterPresence))?;
            continue;
        };
        let suffix = source
            .type_name()
            .strip_prefix("LocationCondition_")
            .ok_or(fault(SourceFault::FilterType))?;
        let mut tag = suffix.chars();
        let first = tag
            .next()
            .ok_or(fault(SourceFault::FilterType))?
            .to_ascii_lowercase();
        require_equal!(
            actual["type"],
            format!("{first}{}", tag.as_str()),
            fault(SourceFault::FilterType)
        );
        for payload in source.fields() {
            check_payload(
                payload.value(),
                actual
                    .get(payload.name())
                    .ok_or(fault(SourceFault::FilterPayloadFieldMissing))?,
                payload.name(),
                owner,
            )?;
        }
    }
    Ok(())
}

fn check_payload(
    source: &SourceValue<'_>,
    actual: &Value,
    field: &str,
    owner: u32,
) -> OracleResult {
    match source {
        SourceValue::Null => error::require(actual.is_null(), fault(SourceFault::PayloadNull))?,
        SourceValue::Enum(name) => require_equal!(actual, *name, fault(SourceFault::PayloadEnum)),
        SourceValue::Array(values) => {
            let actual = actual.as_array().ok_or(fault(SourceFault::PayloadArray))?;
            require_equal!(actual.len(), values.len(), fault(SourceFault::PayloadCount));
            let targets = values.iter().zip(actual);
            for (source, actual) in targets {
                check_payload(source, actual, field, owner)?;
            }
        }
        SourceValue::Reference(target) => check_reference(target, actual, field, owner)?,
        SourceValue::Error(error) => check_error(*error, actual, field)?,
        _ => return Err(fault(SourceFault::UnsupportedPayload)),
    }
    Ok(())
}

fn check_reference(
    target: &Reference<'_>,
    actual: &Value,
    field: &str,
    owner: u32,
) -> OracleResult {
    let weak = matches!(field, "location" | "locations");
    let key = if weak { "recordGuid" } else { "guid" };
    require_equal!(
        actual[key],
        target.guid().to_string(),
        fault(SourceFault::TargetIdentity)
    );
    require_equal!(
        actual["recordPath"],
        target.record_path(),
        fault(SourceFault::TargetPath)
    );
    require_equal!(
        actual["targetType"],
        target.type_name(),
        fault(SourceFault::TargetType)
    );
    require_equal!(
        actual["targetRecordIndex"],
        target.record_index(),
        fault(SourceFault::TargetIndex)
    );
    if !weak {
        return Ok(());
    }
    match target.location() {
        Some(location) => require_equal!(
            actual["location"],
            location.as_str(),
            fault(SourceFault::TargetLocation)
        ),
        None => error::require(
            actual["location"].is_null(),
            fault(SourceFault::TargetLocation),
        )?,
    }
    if target.record_index() != owner {
        require_equal!(
            actual["status"],
            "outsideOwner",
            fault(SourceFault::OutsideOwnerStatus)
        );
        error::require(
            actual["variable"].is_null(),
            fault(SourceFault::OutsideOwnerVariable),
        )?;
    }
    Ok(())
}

fn check_error(error: ValueError, actual: &Value, field: &str) -> OracleResult {
    match error {
        ValueError::DanglingReference { guid } => {
            let key = if matches!(field, "location" | "locations") {
                "recordGuid"
            } else {
                "guid"
            };
            require_equal!(
                actual[key],
                guid.to_string(),
                fault(SourceFault::TargetIdentity)
            );
        }
        ValueError::OutsideRecord {
            struct_index,
            instance_index,
        }
        | ValueError::InvalidPointer {
            struct_index,
            instance_index,
        } => {
            let pointer = actual
                .get("sourcePointer")
                .ok_or(fault(SourceFault::SourcePointer))?;
            require_equal!(
                pointer["structIndex"],
                struct_index,
                fault(SourceFault::PointerStructIndex)
            );
            require_equal!(
                pointer["instanceIndex"],
                instance_index,
                fault(SourceFault::PointerInstanceIndex)
            );
        }
        _ => return Err(fault(SourceFault::UnreadablePayload)),
    }
    Ok(())
}
