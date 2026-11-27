// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{PaletteRecordSelection, PaletteSelection, PaletteSource};
use p4kx_extraction::{
    DataForgeOrigin, FieldOrigin, InputScope, SourceCoordinateOmission, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRef, SourceUseRole, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::PackageResult;
use crate::package_palette_provenance::error;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Named,
    MaterialDefault,
    IncludedPath,
    SocEntity,
    Unknown,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Selection {
    kind: Kind,
    selector: FieldOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    lookup: Option<Lookup>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lookup {
    record: Record,
    origin: FieldOrigin,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record {
    index: u32,
    guid: String,
    guid_origin: FieldOrigin,
}

impl Record {
    pub(super) fn capture(
        sources: &SourceUses,
        record: PaletteRecordSelection,
        scope: Option<&SourceScope>,
    ) -> PackageResult<Self> {
        let used = record_field(scope, record.index, "guid", SourceUseRole::Payload)?;
        Ok(Self {
            index: record.index,
            guid: record.guid.to_string(),
            guid_origin: sources.origin(used.as_slice(), used.is_some()),
        })
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(super) struct SelectionKey {
    kind: Kind,
    selector: Option<SourceUseRef>,
    lookup: Option<LookupKey>,
}

#[derive(PartialEq, Eq, Hash)]
struct LookupKey {
    index: u32,
    guid: String,
    selector: Option<SourceUseRef>,
}

impl Selection {
    pub(super) fn key(&self) -> SelectionKey {
        SelectionKey {
            kind: self.kind,
            selector: self.selector.source_uses_refs().first().cloned(),
            lookup: self.lookup.as_ref().map(|lookup| LookupKey {
                index: lookup.record.index,
                guid: lookup.record.guid.clone(),
                selector: lookup.origin.source_uses_refs().first().cloned(),
            }),
        }
    }

    pub(super) fn capture(
        sources: &SourceUses,
        source: &PaletteSource,
        palette_scope: Option<&SourceScope>,
    ) -> PackageResult<Self> {
        let (kind, used) = match source.selection() {
            PaletteSelection::Named { location } => (
                Kind::Named,
                selected(scope(sources, source.named_origin())?, location.as_ref())?,
            ),
            PaletteSelection::MaterialDefault { library } => {
                let scope = observed(sources, library.as_deref().map(SourceInput::Archive))?;
                (
                    Kind::MaterialDefault,
                    selected(scope, source.material_default_location())?,
                )
            }
            PaletteSelection::IncludedPath => (Kind::IncludedPath, None),
            PaletteSelection::SocEntity => (Kind::SocEntity, None),
            _ => (Kind::Unknown, None),
        };
        let lookup = source
            .records()
            .selector
            .map(|record| Lookup::capture(sources, record, palette_scope))
            .transpose()?;
        Ok(Self {
            kind,
            selector: sources.origin(used.as_slice(), used.is_some()),
            lookup,
        })
    }
}

impl Lookup {
    fn capture(
        sources: &SourceUses,
        record: PaletteRecordSelection,
        scope: Option<&SourceScope>,
    ) -> PackageResult<Self> {
        let used = record_field(scope, record.index, "fileName", SourceUseRole::Selector)?;
        Ok(Self {
            record: Record::capture(sources, record, scope)?,
            origin: sources.origin(used.as_slice(), used.is_some()),
        })
    }
}

fn record_field(
    scope: Option<&SourceScope>,
    record_index: u32,
    field: &str,
    role: SourceUseRole,
) -> PackageResult<Option<SourceUse>> {
    scope
        .map(|scope| {
            scope.at(
                SourceLocation::DataForge {
                    record_index,
                    field: field.to_owned(),
                },
                role,
            )
        })
        .transpose()
        .map_err(error)
}

fn selected(
    scope: Option<SourceScope>,
    location: Option<&SourceLocation>,
) -> PackageResult<Option<SourceUse>> {
    let Some(scope) = scope else {
        return Ok(None);
    };
    let location = location.cloned().unwrap_or(SourceLocation::Opaque {
        reason: SourceCoordinateOmission::Unavailable,
    });
    scope
        .at(location, SourceUseRole::Selector)
        .map(Some)
        .map_err(error)
}

pub(super) fn scope(
    sources: &SourceUses,
    origin: Option<&DataForgeOrigin>,
) -> PackageResult<Option<SourceScope>> {
    let input = match origin {
        Some(DataForgeOrigin::Archive(entry)) => Some(SourceInput::Archive(entry)),
        Some(DataForgeOrigin::File(file)) => Some(SourceInput::File(file)),
        _ => None,
    };
    observed(sources, input)
}

fn observed(
    sources: &SourceUses,
    input: Option<SourceInput<'_>>,
) -> PackageResult<Option<SourceScope>> {
    input
        .map(|input| sources.observe(input, InputScope::Payload))
        .transpose()
        .map_err(error)
}
