// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_extraction::{
    EntryDirectoryOrigins, EntryDirectoryOriginsError, FieldOrigin, FieldOrigins, FieldPath,
    SourceInventory, SourceUses,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::Comparison;
use crate::report::{ChecksumReport, EntryChangeReport};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = historical_optional)]
pub(super) struct EntryOriginFields {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

fn historical_optional(schema: &mut schemars::Schema) {
    if let Some(properties) = schema.as_object_mut() {
        properties.remove("required");
    }
}

#[derive(Debug)]
pub(super) struct EntryOrigins(Result<EntryOriginFields, EntryDirectoryOriginsError>);

impl Serialize for EntryOrigins {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .as_ref()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl EntryOrigins {
    pub(super) fn capture(comparison: &Comparison<'_>, changes: &[EntryChangeReport<'_>]) -> Self {
        Self(capture(comparison, changes))
    }
}

fn capture(
    comparison: &Comparison<'_>,
    changes: &[EntryChangeReport<'_>],
) -> Result<EntryOriginFields, EntryDirectoryOriginsError> {
    let uses = SourceUses::with_compact_use_refs();
    let old = comparison.old_side().owner();
    let new = comparison.new_side().owner();
    let mut fields = FieldOrigins::new();
    for (index, (change, row)) in comparison.entry_changes().iter().zip(changes).enumerate() {
        let path = format!("/entries/changes/{index}");
        if let Some((entry, state)) = change.old_entry().zip(row.old.as_ref()) {
            let origin = observe(
                old,
                entry,
                state.checksum.as_ref(),
                &uses,
                &mut fields,
                &format!("{path}/old"),
            )?;
            if change.new_entry().is_none() {
                put(&mut fields, &format!("{path}/path"), origin)?;
            }
        }
        if let Some((entry, state)) = change.new_entry().zip(row.new.as_ref()) {
            let origin = observe(
                new,
                entry,
                state.checksum.as_ref(),
                &uses,
                &mut fields,
                &format!("{path}/new"),
            )?;
            put(&mut fields, &format!("{path}/path"), origin)?;
        }
    }
    Ok(EntryOriginFields {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

fn observe(
    owner: &Archive,
    entry: &Entry,
    checksum: Option<&ChecksumReport>,
    uses: &SourceUses,
    fields: &mut FieldOrigins,
    side: &str,
) -> Result<FieldOrigin, EntryDirectoryOriginsError> {
    let observed = EntryDirectoryOrigins::observe(owner, entry, uses.clone())?;
    put(fields, &format!("{side}/size"), observed.size_origin()?)?;
    if let Some(checksum) = checksum {
        let field = checksum.field_name();
        put(
            fields,
            &format!("{side}/{field}"),
            observed.checksum_origin()?,
        )?;
    }
    Ok(observed.path_origin().clone())
}

fn put(
    fields: &mut FieldOrigins,
    path: &str,
    origin: FieldOrigin,
) -> Result<(), EntryDirectoryOriginsError> {
    Ok(fields.insert(FieldPath::parse(path)?, origin)?)
}
