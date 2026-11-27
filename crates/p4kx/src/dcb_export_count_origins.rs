// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_extraction::{
    DataForgeOrigin, FieldOrigins, FieldPath, RecordExportProvenance, SourceInventory,
};
use serde::Serialize;

use super::report::reason;

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum CountOrigin {
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
}

pub(super) fn capture(
    source: Option<&DataForgeOrigin>,
    indices: Option<Vec<u32>>,
    exported: usize,
    failed_before_tracking: bool,
) -> Option<CountOrigin> {
    match indices {
        Some(indices) => {
            Some(
                bind(source, indices, exported).unwrap_or_else(|error| CountOrigin::Failed {
                    error: reason(error.as_ref()),
                }),
            )
        }
        None if failed_before_tracking => None,
        None => Some(CountOrigin::Failed {
            error: "the Record export did not retain committed definitions".to_owned(),
        }),
    }
}

fn bind(
    source: Option<&DataForgeOrigin>,
    indices: Vec<u32>,
    exported: usize,
) -> Result<CountOrigin> {
    ensure!(
        indices.len() == exported,
        "the committed definition list does not match the exported Record count"
    );
    let (written, provenance) = RecordExportProvenance::capture(source, indices)?.into_parts();
    let mut fields = FieldOrigins::new();
    fields.insert(FieldPath::parse("/exported")?, written)?;
    Ok(CountOrigin::Bound { fields, provenance })
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
