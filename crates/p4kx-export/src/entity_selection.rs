// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

use crate::Kind;
use crate::selection::is_kind_leftover;

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
    let Some(kind) = Kind::by_record(path, record.type_name()?).filter(|kind| owns(*kind)) else {
        return Ok(None);
    };
    Ok(Some(if is_kind_leftover(kind, path, record.name()?) {
        Selected::Leftover(kind)
    } else {
        Selected::Entity(kind)
    }))
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
