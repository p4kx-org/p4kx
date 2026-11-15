// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result, ensure};
use p4kx_assembly::{PrefabFile, PrefabSet};
use p4kx_extraction::model::{PrefabLibraryNameSource, PrefabSource};
use p4kx_extraction::{
    Archive, Entry, FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceCoordinateOmission,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUseRole, SourceUses,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Origins {
    pub(super) field_origins: FieldOrigins,
    pub(super) provenance: SourceInventory,
}

pub(super) fn index<'set>(
    archive: &Archive,
    set: &'set PrefabSet<'_>,
) -> Result<(Vec<&'set str>, Origins)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut field_origins = FieldOrigins::new();
    field_origins.insert(FieldPath::parse("/archive")?, uses.origin(&[], true))?;
    let surveyed = uses.observe_root_directory_paths(archive)?;
    let complete = surveyed.is_some();
    let inputs: Vec<_> = surveyed.into_iter().collect();
    let mut path = FieldPath::parse("/libraries")?;
    field_origins.insert(path.clone(), uses.origin(&inputs, complete))?;
    let mut names = Vec::with_capacity(set.library_entries().len());
    for (index, (name, entry)) in set.library_entries().enumerate() {
        path.push_index(index)?;
        field_origins.insert(path.clone(), name_origin(archive, entry, &uses)?)?;
        path.pop();
        names.push(name);
    }
    Ok((
        names,
        Origins {
            field_origins,
            provenance: uses.freeze()?,
        },
    ))
}

pub(super) fn detail(archive: &Archive, set: &PrefabSet<'_>, file: &PrefabFile) -> Result<Origins> {
    let entry = set
        .source_entry(file)
        .context("original prefab library read")?;
    let uses = SourceUses::with_compact_use_refs();
    let (scope, complete) = input(archive, entry, &uses)?;
    let mut projection = Detail {
        scope,
        uses,
        complete,
        fields: FieldOrigins::new(),
    };
    projection.base(file)?;
    for (index, _) in file.library().prefabs.iter().enumerate() {
        let row = file.source().and_then(|source| source.prefabs.get(index));
        projection.row(index, row)?;
    }
    Ok(Origins {
        field_origins: projection.fields,
        provenance: projection.uses.freeze()?,
    })
}

struct Detail {
    scope: SourceScope,
    uses: SourceUses,
    complete: bool,
    fields: FieldOrigins,
}

impl Detail {
    fn base(&mut self, file: &PrefabFile) -> Result<()> {
        self.fields
            .insert(FieldPath::parse("/archive")?, self.uses.origin(&[], true))?;
        self.fields.insert(
            FieldPath::parse("/file")?,
            directory_name(&self.scope, self.complete, &self.uses)?,
        )?;
        let source = file.source();
        ensure!(
            source.is_none_or(|source| source.prefabs.len() == file.library().prefabs.len()),
            "prefab source positions differ from the parsed rows"
        );
        self.put("/library", library_name(file), SourceUseRole::Payload)?;
        self.put(
            "/prefabs",
            source.and_then(|source| SourceLocation::xml_element(&source.root, "Prefab")),
            SourceUseRole::Selector,
        )
    }

    fn row(&mut self, index: usize, row: Option<&PrefabSource>) -> Result<()> {
        let path = format!("/prefabs/{index}");
        for (field, original, position) in [
            ("name", "Name", row.and_then(|row| row.name.as_ref())),
            ("id", "Id", row.and_then(|row| row.id.as_ref())),
        ] {
            let (location, role) = position.map_or_else(
                || {
                    let scope =
                        row.and_then(|row| SourceLocation::xml_element(&row.element, original));
                    (scope, SourceUseRole::Conversion)
                },
                |position| {
                    let attribute = SourceLocation::xml_attribute(position, original);
                    (attribute, SourceUseRole::Payload)
                },
            );
            self.put(&format!("{path}/{field}"), location, role)?;
        }
        let subtree = row.and_then(|row| {
            SourceLocation::xml_element(row.objects.as_ref().unwrap_or(&row.element), "Objects")
        });
        let contributors = self.origin(subtree, SourceUseRole::Conversion)?;
        for field in ["objects", "nested", "left_out"] {
            self.fields.insert(
                FieldPath::parse(&format!("{path}/{field}"))?,
                contributors.clone(),
            )?;
        }
        Ok(())
    }

    fn origin(&self, at: Option<SourceLocation>, role: SourceUseRole) -> Result<FieldOrigin> {
        let usage = match at {
            Some(at) => self.scope.at(at, role)?,
            None => self
                .scope
                .at_opaque(role, SourceCoordinateOmission::Unavailable)?,
        };
        Ok(self.uses.origin(&[usage], self.complete))
    }

    fn put(&mut self, path: &str, at: Option<SourceLocation>, role: SourceUseRole) -> Result<()> {
        let origin = self.origin(at, role)?;
        self.fields.insert(FieldPath::parse(path)?, origin)?;
        Ok(())
    }
}

fn name_origin(archive: &Archive, entry: &Entry, uses: &SourceUses) -> Result<FieldOrigin> {
    let (scope, complete) = input(archive, entry, uses)?;
    directory_name(&scope, complete, uses)
}

pub(super) fn input(
    archive: &Archive,
    entry: &Entry,
    uses: &SourceUses,
) -> Result<(SourceScope, bool)> {
    let observed = archive
        .observed_entry(entry)
        .context("physical prefab library Entry")?;
    let complete = observed.ancestors().all(|step| step.is_some());
    let scope = uses.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
    Ok((scope, complete))
}

pub(super) fn directory_name(
    scope: &SourceScope,
    complete: bool,
    uses: &SourceUses,
) -> Result<FieldOrigin> {
    let path = scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Conversion,
    )?;
    Ok(uses.origin(&[path], complete))
}

pub(super) fn library_name(file: &PrefabFile) -> Option<SourceLocation> {
    match &file.source()?.name {
        PrefabLibraryNameSource::Attribute(position) => {
            SourceLocation::xml_attribute(position, "Name")
        }
        PrefabLibraryNameSource::Element(position) => SourceLocation::xml_element(position, "name"),
        _ => None,
    }
}
