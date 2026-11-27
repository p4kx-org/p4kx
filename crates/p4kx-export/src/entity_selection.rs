// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record, SourceUseRole};

use crate::Kind;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::{Exosuit, admit_exosuit, is_kind_leftover};

/// What the entity kinds make of a Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selected {
    /// The Record is an entity of the kind.
    Entity(Kind),
    /// The Record is a development leftover of the kind.
    Leftover(Kind),
}

/// Selects generic catalog Records by their declared type and filtered owning path.
/// Dedicated readers select their own source scopes.
pub(crate) fn select(
    record: &Record<'_>,
    filter: Option<&Filter>,
) -> Result<Option<Selected>, DataForgeError> {
    select_observed(record, filter, |_, _| {})
}

/// Classifies one original Record while observing each header input that selection reaches.
pub(crate) fn select_observed(
    record: &Record<'_>,
    filter: Option<&Filter>,
    mut observe: impl FnMut(RecordHeaderInput, SourceUseRole),
) -> Result<Option<Selected>, DataForgeError> {
    let path = observed(
        RecordHeaderInput::NormalizedPath,
        record.record_path(),
        &mut observe,
    )?;
    if filter.is_some_and(|filter| !filter.matches(path)) {
        return Ok(None);
    }
    let kind_input = RecordHeaderInput::Field(RecordHeaderField::SourceType);
    let source_type = observed(kind_input, record.type_name(), &mut observe)?;
    select_header(record, path, source_type, &mut observe)
}

/// Reads only the header fields that can affect Item admission.
pub(crate) fn select_item_observed(
    record: &Record<'_>,
    observe: impl FnMut(RecordHeaderInput, SourceUseRole),
) -> Result<Option<Selected>, DataForgeError> {
    select_family_observed(record, Kind::Items, observe)
}

/// Reads only the original header fields that can affect one Entity family's admission.
pub(crate) fn select_family_observed(
    record: &Record<'_>,
    family: Kind,
    mut observe: impl FnMut(RecordHeaderInput, SourceUseRole),
) -> Result<Option<Selected>, DataForgeError> {
    let rule = family.records();
    let path = observed(
        RecordHeaderInput::NormalizedPath,
        record.record_path(),
        &mut observe,
    );
    if path
        .as_ref()
        .is_ok_and(|path| !family.matches_record_path(path))
    {
        return Ok(None);
    }
    let source_type = observed(
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        record.type_name(),
        &mut observe,
    );
    if source_type
        .as_ref()
        .is_ok_and(|name| *name != rule.type_name)
    {
        return Ok(None);
    }
    select_header(record, path?, source_type?, &mut observe)
}

fn select_header(
    record: &Record<'_>,
    path: &str,
    source_type: &str,
    observe: &mut impl FnMut(RecordHeaderInput, SourceUseRole),
) -> Result<Option<Selected>, DataForgeError> {
    let Some(kind) = Kind::by_record(path, source_type).filter(|kind| owns(*kind)) else {
        return Ok(None);
    };
    let name_input = RecordHeaderInput::Field(RecordHeaderField::RecordName);
    let name = observed(name_input, record.name(), observe)?;
    if kind == Kind::Ships
        && Exosuit::by_path(path).is_some()
        && !Exosuit::from_header(path, name, source_type)
            .is_some_and(|exosuit| admit_exosuit(exosuit, record))
    {
        return Ok(None);
    }
    Ok(Some(if is_kind_leftover(kind, path, name) {
        Selected::Leftover(kind)
    } else {
        Selected::Entity(kind)
    }))
}

fn observed<T>(
    field: RecordHeaderInput,
    value: Result<T, DataForgeError>,
    observe: &mut impl FnMut(RecordHeaderInput, SourceUseRole),
) -> Result<T, DataForgeError> {
    let role = if value.is_ok() {
        SourceUseRole::Selector
    } else {
        SourceUseRole::Attempted
    };
    observe(field, role);
    value
}

const fn owns(kind: Kind) -> bool {
    !kind.has_dedicated_reader()
        && !matches!(
            kind,
            Kind::Contracts
                | Kind::MissionLocationTemplates
                | Kind::ContractTemplates
                | Kind::MissionBrokerEntries
        )
}
