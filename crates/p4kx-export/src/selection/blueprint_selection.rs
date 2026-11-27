// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

use super::{Kind, is_leftover};

/// What the blueprint selection makes of one Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlueprintSelection {
    /// Not a blueprint of the catalog, or one the Filter does not match.
    Outside,
    /// A blueprint of the catalog.
    Kept,
    /// A blueprint left out as a development leftover.
    Leftover,
}

/// Returns what the blueprint selection makes of `record`: the one rule behind the `blueprints`
/// kind of the catalog and the blueprints of [`read_crafting`](crate::read_crafting).
///
/// # Errors
///
/// Returns the error of the Record's path, type or name when it cannot be read.
pub(crate) fn selects_blueprint(
    record: &Record<'_>,
    filter: Option<&Filter>,
) -> Result<BlueprintSelection, DataForgeError> {
    let rule = Kind::Blueprints.records();
    let path = record.record_path()?;
    if !rule.matches_path(path)
        || record.type_name()? != rule.type_name
        || filter.is_some_and(|filter| !filter.matches(path))
    {
        return Ok(BlueprintSelection::Outside);
    }
    Ok(if is_leftover(path, record.name()?) {
        BlueprintSelection::Leftover
    } else {
        BlueprintSelection::Kept
    })
}
