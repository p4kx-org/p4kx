// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

use crate::Kind;
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
    let path = record.record_path()?;
    if filter.is_some_and(|filter| !filter.matches(path)) {
        return Ok(None);
    }
    let source_type = record.type_name()?;
    let Some(kind) = Kind::by_record(path, source_type).filter(|kind| owns(*kind)) else {
        return Ok(None);
    };
    let name = record.name()?;
    if kind == Kind::Ships && !admits_ship(record, path, name, source_type) {
        return Ok(None);
    }
    Ok(Some(if is_kind_leftover(kind, path, name) {
        Selected::Leftover(kind)
    } else {
        Selected::Entity(kind)
    }))
}

pub(crate) fn admits_ship(record: &Record<'_>, path: &str, name: &str, source_type: &str) -> bool {
    Exosuit::by_path(path).is_none()
        || Exosuit::from_header(path, name, source_type)
            .is_some_and(|exosuit| admit_exosuit(exosuit, record))
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
