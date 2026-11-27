// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{
    DataForgeOrigin, FieldOrigins, FieldPath, RecordExportProvenance, RecordExportReport,
    SourceInventory,
};
use serde::Serialize;

use super::report::reason;

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum CountOrigin {
    Bound {
        #[serde(rename = "fieldOrigins")]
        fields: FieldOrigins,
        provenance: SourceInventory,
    },
    Failed {
        #[serde(rename = "provenanceError")]
        error: String,
    },
}

impl CountOrigin {
    pub(super) const fn failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub(crate) fn converted(report: &RecordExportReport) -> Option<Self> {
        report.written_provenance().map(|origin| {
            let result = origin
                .map_err(anyhow::Error::new)
                .and_then(|proof| Self::bound(proof.clone()));
            result.unwrap_or_else(|error| Self::failure(error.as_ref()))
        })
    }

    fn bound(proof: RecordExportProvenance) -> Result<Self> {
        let (written, provenance) = proof.into_parts();
        let mut fields = FieldOrigins::new();
        fields.insert(FieldPath::parse("/exported")?, written)?;
        Ok(Self::Bound { fields, provenance })
    }

    fn failure(error: &dyn std::error::Error) -> Self {
        Self::Failed {
            error: reason(error),
        }
    }
}

pub(super) fn capture(
    source: Option<&DataForgeOrigin>,
    indices: Option<Vec<u32>>,
    exported: usize,
    failed_before_tracking: bool,
) -> Option<CountOrigin> {
    if indices.is_none() && failed_before_tracking {
        return None;
    }
    let result = RecordExportProvenance::capture_count(source, indices, exported)
        .map_err(anyhow::Error::new)
        .and_then(CountOrigin::bound);
    Some(result.unwrap_or_else(|error| CountOrigin::failure(error.as_ref())))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::capture;

    #[test]
    fn incomplete_accounting_reports_an_error_instead_of_assigning_origins() -> anyhow::Result<()> {
        let summary = capture(None, Some(vec![4]), 2, false)
            .ok_or("count origin was requested")
            .map_err(anyhow::Error::msg)?;
        assert!(
            summary.failed(),
            "mismatched accounting fails the origin proof"
        );
        assert_eq!(
            serde_json::to_value(summary)?,
            json!({"provenanceError": "the committed definition list does not match the exported Record count"}),
            "the failure carries no fabricated origin inventory"
        );
        Ok(())
    }
}
