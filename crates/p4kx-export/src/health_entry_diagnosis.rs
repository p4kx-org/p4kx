// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;

use super::OriginalDatum;
use super::health_entry_receipt::EntryReceipt;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::{CatalogError, HealthText};

/// The operation's technical diagnosis, separate from its original Entry identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct EntryDiagnosis {
    pub(crate) code: Option<ExportDiagnosticCode>,
    field: EntryText,
    source_type: EntryText,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum EntryText {
    #[default]
    Absent,
    Constant(&'static str),
    Path,
}

impl EntryDiagnosis {
    pub(crate) const fn technical(
        code: ExportDiagnosticCode,
        field: &'static str,
        source_type: &'static str,
    ) -> Self {
        Self {
            code: Some(code),
            field: EntryText::Constant(field),
            source_type: EntryText::Constant(source_type),
        }
    }

    pub(crate) const fn field(code: ExportDiagnosticCode, field: &'static str) -> Self {
        Self {
            code: Some(code),
            field: EntryText::Constant(field),
            source_type: EntryText::Absent,
        }
    }

    pub(crate) const fn loadout(code: Option<ExportDiagnosticCode>) -> Self {
        Self {
            code,
            field: EntryText::Path,
            source_type: EntryText::Constant("LoadoutFile"),
        }
    }

    pub(super) fn retain(
        &self,
        entry: &EntryReceipt,
    ) -> Result<[OriginalDatum<HealthText>; 2], CatalogError> {
        Ok([self.field.retain(entry)?, self.source_type.retain(entry)?])
    }
}

impl EntryText {
    fn retain(&self, entry: &EntryReceipt) -> Result<OriginalDatum<HealthText>, CatalogError> {
        match self {
            Self::Path => entry.path_fact(),
            Self::Absent => Ok(OriginalDatum::observed(
                SourceValue::Absent,
                Vec::new(),
                true,
            )),
            Self::Constant(value) => Ok(OriginalDatum::observed(
                SourceValue::Present(HealthText::new(value)),
                Vec::new(),
                true,
            )),
        }
    }
}

#[cfg(test)]
impl EntryDiagnosis {
    pub(crate) fn accounting_field<'entry>(&'entry self, path: &'entry str) -> Option<&'entry str> {
        match self.field {
            EntryText::Absent => None,
            EntryText::Constant(value) => Some(value),
            EntryText::Path => Some(path),
        }
    }
}
