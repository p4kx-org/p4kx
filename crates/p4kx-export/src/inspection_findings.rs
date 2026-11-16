// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::ExportAccountingError;
use crate::catalog_number_health::NumberRequirement;
use crate::catalog_read::original::ReferenceObservation;
use crate::export_diagnostic::ExportSourceRole;
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::selection::Kind;

/// Exceptional field findings retained only for inspections that observed them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectionFindings(Option<Box<FindingLanes>>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindingLanes {
    diagnostics: Vec<DiagnosticObservation>,
    number_requirements: Vec<NumberRequirement>,
    unresolved_references: Vec<ReferenceObservation>,
}

impl InspectionFindings {
    pub(crate) fn new(
        diagnostics: Vec<DiagnosticObservation>,
        number_requirements: Vec<NumberRequirement>,
        unresolved_references: Vec<ReferenceObservation>,
    ) -> Self {
        let empty = diagnostics.is_empty()
            && number_requirements.is_empty()
            && unresolved_references.is_empty();
        Self((!empty).then(|| {
            Box::new(FindingLanes {
                diagnostics,
                number_requirements,
                unresolved_references,
            })
        }))
    }

    pub(crate) fn diagnostics(&self) -> &[DiagnosticObservation] {
        self.0
            .as_deref()
            .map_or(&[], |lanes| lanes.diagnostics.as_slice())
    }

    /// Conditional failures until the exact original value has an emitted lossless row.
    pub(crate) fn number_requirements(&self) -> &[NumberRequirement] {
        self.0
            .as_deref()
            .map_or(&[], |lanes| lanes.number_requirements.as_slice())
    }

    pub(crate) fn unresolved_references(&self) -> &[ReferenceObservation] {
        self.0
            .as_deref()
            .map_or(&[], |lanes| lanes.unresolved_references.as_slice())
    }

    pub(super) fn contextualize(&mut self, kind: Option<Kind>, role: ExportSourceRole) {
        if let Some(lanes) = &mut self.0 {
            for diagnostic in &mut lanes.diagnostics {
                diagnostic.kind = kind;
                diagnostic.source_role = Some(role);
            }
            for reference in &mut lanes.unresolved_references {
                reference.contextualize(kind, role);
            }
        }
    }

    /// Merges number bodies after the inspections' value shapes have matched.
    pub(super) fn merge_numbers(&mut self, other: &mut Self) -> Result<(), ExportAccountingError> {
        if let (Some(retained), Some(incoming)) = (&mut self.0, &mut other.0) {
            for (required, other) in retained
                .number_requirements
                .iter_mut()
                .zip(std::mem::take(&mut incoming.number_requirements))
            {
                required.merge(other)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{FindingLanes, InspectionFindings};

    #[test]
    fn clean_inspection_findings_need_no_allocation_and_one_inline_word() {
        let clean = InspectionFindings::new(Vec::new(), Vec::new(), Vec::new());
        assert!(clean.0.is_none(), "no allocation for a clean inspection");
        assert_eq!(
            clean,
            InspectionFindings::default(),
            "both empty constructors agree"
        );
        assert_eq!(
            size_of::<InspectionFindings>(),
            size_of::<usize>(),
            "the receipt contains only the optional pointer"
        );
        assert_eq!(
            size_of::<FindingLanes>() - size_of::<InspectionFindings>(),
            size_of::<[usize; 8]>(),
            "three inline vector headers are replaced by one pointer"
        );
    }
}
