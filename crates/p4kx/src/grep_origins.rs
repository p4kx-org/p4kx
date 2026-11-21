// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use anyhow::{Context as _, Result};
use p4kx_extraction::{
    DataForgeOrigin, FieldOrigins, FieldPath, InputScope, RecordJsonLocation,
    RecordJsonRecordField, RecordJsonSource, SourceCoordinateOmission, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    TextEncoding,
};
use p4kx_search::{EntryHit, EntryMatchSource, LocalizationMatchSource, ObservedSearch, RecordHit};

struct Origins {
    uses: SourceUses,
    fields: FieldOrigins,
}

pub(super) fn capture(search: &ObservedSearch<'_>) -> Result<(FieldOrigins, SourceInventory)> {
    let mut result = Origins {
        uses: SourceUses::with_compact_use_refs(),
        fields: FieldOrigins::new(),
    };
    let report = search.report();
    if !report.records.is_empty() {
        let input = record_input(search.record_origin());
        let complete = !matches!(input, SourceInput::Unknown);
        let scope = result.uses.observe(input, InputScope::Payload)?;
        for (position, hit) in report.records.iter().enumerate() {
            result.record(
                hit,
                position,
                search.record_structure(position),
                &scope,
                complete,
            )?;
        }
    }
    for position in 0..report.localization.len() {
        let input = search
            .localization_source(position)
            .context("localization hit source is absent")?;
        result.localization(position, input)?;
    }
    for (position, hit) in report.entries.iter().enumerate() {
        let input = search
            .entry_source(position)
            .context("Entry hit source is absent")?;
        result.entry(hit, position, input)?;
    }
    Ok((result.fields, result.uses.freeze()?))
}

impl Origins {
    fn record(
        &mut self,
        hit: &RecordHit,
        position: usize,
        structure: Option<u32>,
        scope: &SourceScope,
        complete: bool,
    ) -> Result<()> {
        let row = format!("/records/{position}");
        for (output, field) in [
            ("guid", RecordJsonRecordField::Guid),
            ("name", RecordJsonRecordField::Name),
            ("record_path", RecordJsonRecordField::FileName),
        ] {
            let actual = record_json_use(
                scope,
                RecordJsonSource::Record {
                    index: hit.record_index,
                    field,
                },
            )?;
            self.put(&row, &[output], slice::from_ref(&actual), complete)?;
        }
        let field = record_field(scope, hit, &hit.field, SourceUseRole::Selector)?;
        self.put(&row, &["field"], slice::from_ref(&field), complete)?;
        let selector = record_json_use(
            scope,
            RecordJsonSource::Record {
                index: hit.record_index,
                field: RecordJsonRecordField::Struct,
            },
        )?;
        if let Some(structure) = structure {
            let name = record_json_use(scope, RecordJsonSource::StructName { index: structure })?;
            self.put(&row, &["type"], &[selector, name], complete)?;
        } else {
            self.put(&row, &["type"], slice::from_ref(&selector), false)?;
        }
        let original = record_field(scope, hit, &hit.field, SourceUseRole::Conversion)?;
        self.excerpt(&row, &original, complete)
    }

    fn localization(&mut self, position: usize, source: LocalizationMatchSource<'_>) -> Result<()> {
        let input = source
            .origin
            .map_or(SourceInput::Unknown, SourceInput::Archive);
        let scope = self.uses.observe(input, InputScope::Payload)?;
        let complete = source.origin.is_some();
        let row = format!("/loc/{position}");
        let language = directory_path(&scope, SourceUseRole::Conversion)?;
        self.put(&row, &["language"], slice::from_ref(&language), complete)?;
        let key = scope.at(
            subresource("localizationIniLine", vec![source.line_index], "key"),
            SourceUseRole::Conversion,
        )?;
        self.put(&row, &["key"], slice::from_ref(&key), complete)?;
        let value = scope.at(
            subresource("localizationIniLine", vec![source.line_index], "value"),
            SourceUseRole::Conversion,
        )?;
        self.excerpt(&row, &value, complete)
    }

    fn entry(
        &mut self,
        hit: &EntryHit,
        position: usize,
        source: EntryMatchSource<'_>,
    ) -> Result<()> {
        let input = source
            .origin
            .map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let scope = self.uses.observe(input, InputScope::Payload)?;
        let row = format!("/entries/{position}");
        let known_input = source.origin.is_some();
        let path = directory_path(&scope, SourceUseRole::Payload)?;
        self.put(&row, &["path"], slice::from_ref(&path), known_input)?;
        let (coordinates, content, complete) = if source.encoding == TextEncoding::Utf8 {
            let line = hit.line.saturating_sub(1);
            let coords = scope.at(
                subresource("entryTextLines", vec![0, line], "lineCoordinates"),
                SourceUseRole::Conversion,
            )?;
            let value = scope.at(
                subresource("entryTextLine", vec![line], "text"),
                SourceUseRole::Conversion,
            )?;
            (coords, Some(value), known_input)
        } else {
            let unavailable = scope.at_opaque(
                SourceUseRole::Conversion,
                SourceCoordinateOmission::Unavailable,
            )?;
            (unavailable, None, false)
        };
        self.put(
            &row,
            &["line", "offset"],
            slice::from_ref(&coordinates),
            complete,
        )?;
        self.excerpt(&row, content.as_ref().unwrap_or(&coordinates), complete)
    }

    fn excerpt(&mut self, row: &str, content: &SourceUse, complete: bool) -> Result<()> {
        self.put(
            row,
            &["text", "match", "match/0", "match/1"],
            slice::from_ref(content),
            complete,
        )
    }

    fn put(
        &mut self,
        row: &str,
        fields: &[&str],
        actual: &[SourceUse],
        complete: bool,
    ) -> Result<()> {
        let origin = self.uses.origin(actual, complete);
        for field in fields {
            self.fields
                .insert(FieldPath::parse(&format!("{row}/{field}"))?, origin.clone())?;
        }
        Ok(())
    }
}

const fn record_input(original: Option<&DataForgeOrigin>) -> SourceInput<'_> {
    match original {
        Some(DataForgeOrigin::Archive(entry)) => SourceInput::Archive(entry),
        Some(DataForgeOrigin::File(file)) => SourceInput::File(file),
        _ => SourceInput::Unknown,
    }
}

fn record_json_use(scope: &SourceScope, source: RecordJsonSource<'_>) -> Result<SourceUse> {
    let observed = RecordJsonLocation::of(source).context("original Record JSON coordinate")?;
    Ok(scope.at(observed.location, observed.role)?)
}

fn record_field(
    scope: &SourceScope,
    hit: &RecordHit,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::DataForge {
            record_index: hit.record_index,
            field: field.to_owned(),
        },
        role,
    )?)
}

fn directory_path(scope: &SourceScope, role: SourceUseRole) -> Result<SourceUse> {
    Ok(scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        role,
    )?)
}

fn subresource(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_extraction::{Archive, DataForgeEntry};
    use p4kx_search::{Needle, SearchOptions, search_observed_with_dataforge};
    use p4kx_testkit::{
        ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    };

    use super::capture;

    #[test]
    fn original_loose_file_retains_exact_fields_and_unknown_encryption()
    -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let archive_path = dir.path().join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\unused", b"directory".to_vec()))
            .write_to(&archive_path)?;
        let archive = Archive::open(&archive_path)?;
        let forge_path = dir.path().join("loose.dcb");
        let data = DataForgeBuilder::new()
            .structure(TestStruct::new("Gear").field("name", TestType::String))
            .record(
                TestRecord::new(1, "Gear", "Gear.Test", "libs/gear.xml")
                    .value("name", TestValue::text("match")),
            )
            .build();
        std::fs::write(&forge_path, data)?;
        let entry = DataForgeEntry::read_file(&forge_path)?;
        let results = search_observed_with_dataforge(
            &archive,
            Some(&entry),
            &Needle::new("match")?,
            &SearchOptions::default(),
            &|| false,
        )?
        .ok_or("search cancelled")?;
        let (fields, inventory) = capture(&results)?;
        let fields = serde_json::to_value(fields)?;
        let origin = &fields["/records/0/text"];
        assert_eq!(
            origin["sourceSetComplete"], true,
            "original loose input is known"
        );
        assert_eq!(
            origin["containsEncryptedSource"],
            serde_json::Value::Null,
            "loose archive ancestry is unknown"
        );
        assert_eq!(inventory.source_count(), 1, "one physical loose database");
        Ok(())
    }
}
