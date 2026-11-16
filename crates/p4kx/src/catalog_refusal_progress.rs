// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use p4kx_export::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, EntryOperation, ExportDiagnostic, ExportDiagnosticCode,
    ExportDiagnosticSeverity, ExportDiagnosticStage, ExportHealth, ExportSourceRole,
    FailedExportEntry, Kind,
};
use p4kx_extraction::SourceValueState;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TechnicalLabels<'a> {
    field: Option<&'a str>,
    source_type: Option<&'a str>,
}

impl<'a> TechnicalLabels<'a> {
    fn new(field: Option<&'a str>, source_type: Option<&'a str>) -> Option<Self> {
        (field.is_some() || source_type.is_some()).then_some(Self { field, source_type })
    }

    fn write(self, output: &mut impl Write) -> io::Result<()> {
        writeln!(
            output,
            " field={:?} source_type={:?}",
            self.field.unwrap_or("-"),
            self.source_type.unwrap_or("-"),
        )
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EntryKey {
    kind: &'static str,
    role: ExportSourceRole,
    operation: EntryOperation,
    code: ExportDiagnosticCode,
    stage: ExportDiagnosticStage,
    entry_status: SourceValueState,
}

impl EntryKey {
    const fn from_entry(entry: &FailedExportEntry) -> Self {
        Self {
            kind: entry.kind().name(),
            role: entry.role(),
            operation: entry.operation(),
            code: entry.code(),
            stage: entry.stage(),
            entry_status: entry.entry_index().state(),
        }
    }

    fn write(&self, output: &mut impl Write) -> io::Result<()> {
        write!(
            output,
            "kind={} role={:?} operation={:?} code={:?} stage={:?} entry_status={:?}",
            self.kind, self.role, self.operation, self.code, self.stage, self.entry_status,
        )
    }
}

#[derive(Default)]
struct EntryGroup<'a> {
    count: usize,
    examples: BTreeSet<TechnicalLabels<'a>>,
}

impl<'a> EntryGroup<'a> {
    fn observe(&mut self, entry: &'a FailedExportEntry) {
        self.count += 1;
        let field = if entry_field_is_path(entry.operation()) {
            None
        } else {
            entry.field_text().value()
        };
        if let Some(labels) = TechnicalLabels::new(field, entry.source_type_text().value()) {
            self.examples.insert(labels);
            if self.examples.len() > EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT {
                let _removed = self.examples.pop_last();
            }
        }
    }
}

/// Writes structural details retained by the refused export.
pub(super) fn report(health: &ExportHealth, output: &mut impl Write) -> io::Result<()> {
    for diagnostic in health
        .diagnostics()
        .iter()
        .filter(|row| row.severity != ExportDiagnosticSeverity::Information)
    {
        report_record_samples(diagnostic, output)?;
    }
    report_entry_groups(health.failed_entries(), output)
}

fn report_record_samples(diagnostic: &ExportDiagnostic, output: &mut impl Write) -> io::Result<()> {
    let kind = diagnostic.kind.map_or("catalog", Kind::name);
    let field_is_path = diagnostic_field_is_path(diagnostic);
    let examples: BTreeSet<_> = diagnostic
        .examples()
        .iter()
        .filter_map(|example| {
            let field = if field_is_path {
                None
            } else {
                example.field_text().value()
            };
            TechnicalLabels::new(field, example.source_type_text().value())
        })
        .collect();
    for labels in examples {
        write!(
            output,
            "catalog refusal example: kind={kind} role={:?} stage={:?} code={:?}",
            diagnostic.source_role, diagnostic.stage, diagnostic.code,
        )?;
        labels.write(output)?;
    }
    Ok(())
}

fn report_entry_groups(entries: &[FailedExportEntry], output: &mut impl Write) -> io::Result<()> {
    let mut groups = BTreeMap::<EntryKey, EntryGroup<'_>>::new();
    for entry in entries {
        groups
            .entry(EntryKey::from_entry(entry))
            .or_default()
            .observe(entry);
    }
    for (key, group) in groups {
        write!(output, "catalog refusal entry: ")?;
        key.write(output)?;
        writeln!(output, " count={}", group.count)?;
        for labels in group.examples {
            write!(output, "catalog refusal entry example: ")?;
            key.write(output)?;
            labels.write(output)?;
        }
    }
    Ok(())
}

const fn entry_field_is_path(operation: EntryOperation) -> bool {
    matches!(
        operation,
        EntryOperation::XmlLoadout
            | EntryOperation::XmlLoadoutBytes
            | EntryOperation::VehicleImplementation
            | EntryOperation::VehicleImplementationBytes
            | EntryOperation::VehiclePatch
            | EntryOperation::VehiclePatchBytes
    )
}

fn diagnostic_field_is_path(diagnostic: &ExportDiagnostic) -> bool {
    diagnostic.kind == Some(Kind::Loadouts)
        && diagnostic.code == ExportDiagnosticCode::DanglingReference
        && matches!(
            diagnostic.stage,
            ExportDiagnosticStage::SourceRead | ExportDiagnosticStage::DerivedProjection
        )
}

#[cfg(test)]
mod tests {
    use p4kx_export::EntryOperation;

    use super::{TechnicalLabels, entry_field_is_path};

    #[test]
    fn refusal_labels_keep_original_fields_and_escape_control_characters() -> anyhow::Result<()> {
        let mut output = Vec::new();
        let labels = TechnicalLabels::new(Some("Unknown field\n\t\""), Some("Type\u{1b}"))
            .ok_or_else(|| anyhow::anyhow!("source labels are present"))?;
        labels.write(&mut output)?;
        assert_eq!(
            String::from_utf8(output)?,
            " field=\"Unknown field\\n\\t\\\"\" source_type=\"Type\\u{1b}\"\n",
            "the structural text stays on one line with escaped controls",
        );
        let long_field = "syntheticLongField".repeat(16);
        let labels = TechnicalLabels::new(Some(&long_field), None)
            .ok_or_else(|| anyhow::anyhow!("the complete source label is present"))?;
        assert_eq!(
            labels.field,
            Some(long_field.as_str()),
            "field length does not strip or truncate diagnostic text",
        );
        Ok(())
    }

    #[test]
    fn refusal_excludes_loadout_field_carriers_that_are_paths() {
        for operation in [
            EntryOperation::XmlLoadout,
            EntryOperation::XmlLoadoutBytes,
            EntryOperation::VehicleImplementation,
            EntryOperation::VehicleImplementationBytes,
            EntryOperation::VehiclePatch,
            EntryOperation::VehiclePatchBytes,
        ] {
            assert!(
                entry_field_is_path(operation),
                "the path carrier for {operation:?} is withheld",
            );
        }
        assert!(
            !entry_field_is_path(EntryOperation::ShopInventory),
            "the ShopInventory carrier holds a technical field",
        );
    }
}
