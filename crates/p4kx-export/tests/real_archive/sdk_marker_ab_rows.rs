// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Write as _};

use p4kx_extraction::{FieldOrigin, SourceInventory};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{ProbeMode, Workload};
use crate::real_archive_catalog_table_parity::{TestResult, update_json_digest, wire_row};

#[path = "sdk_marker_ab_encoding.rs"]
mod encoding;
use encoding::{CountedSnapshot, SnapshotSizes};

const ENCRYPTED: &str = "containsEncryptedSource";
const REFS: &str = "sourceUsesRefs";
const COMPLETE: &str = "sourceSetComplete";

pub(crate) struct Rows {
    mode: ProbeMode,
    count: usize,
    values: Sha256,
    markers: MarkerEvidence,
    typed_complete: Sha256,
    snapshot: Option<CountedSnapshot>,
    max_owner_source_uses: usize,
    max_owner_physical_sources: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Evidence {
    values_sha256: String,
    tri_state_sha256: String,
    typed_completeness_sha256: String,
    scope_coverage_sha256: String,
    encrypted_markers: usize,
    unencrypted_markers: usize,
    unknown_markers: usize,
    wire_source_ref_occurrences: usize,
    max_owner_source_uses: usize,
    max_owner_physical_sources: usize,
    #[serde(flatten)]
    snapshot: SnapshotSizes,
}

impl Rows {
    pub(crate) fn new(mode: ProbeMode, workload: Workload) -> TestResult<Self> {
        let snapshot = match workload {
            Workload::Verification => None,
            Workload::CountedSnapshot => Some(CountedSnapshot::new()?),
        };
        Ok(Self {
            mode,
            count: 0,
            values: Sha256::new(),
            markers: MarkerEvidence::default(),
            typed_complete: Sha256::new(),
            snapshot,
            max_owner_source_uses: 0,
            max_owner_physical_sources: 0,
        })
    }

    pub(super) const fn count(&self) -> usize {
        self.count
    }

    pub(super) fn observe_completeness(&mut self, origin: &FieldOrigin) {
        self.typed_complete
            .update([u8::from(origin.source_set_complete())]);
    }

    pub(crate) fn observe_inventory(&mut self, inventory: &SourceInventory) -> TestResult {
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if inventory.provenance_mode() != self.mode.sdk() {
            return Err("an SDK inventory has a different typed provenance mode".into());
        }
        match self.mode {
            ProbeMode::Full if uses == 0 || sources == 0 => {
                return Err("a Full SDK inventory lacks original source evidence".into());
            }
            ProbeMode::MarkersOnly if uses != 0 || sources != 0 => {
                return Err("a MarkersOnly SDK inventory retains source evidence".into());
            }
            _ => {}
        }
        self.max_owner_source_uses = self.max_owner_source_uses.max(uses);
        self.max_owner_physical_sources = self.max_owner_physical_sources.max(sources);
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.inventory(inventory)?;
        }
        Ok(())
    }

    pub(crate) fn observe_row(&mut self, row: &impl Serialize) -> TestResult {
        self.count = self.count.checked_add(1).ok_or("SDK row count overflow")?;
        self.typed_complete.update([2]);
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.row(row)?;
        }
        let mut value =
            wire_row(row).map_err(|_| "serialize one SDK row for technical comparison")?;
        self.markers.hash.update(b"r");
        self.markers.hash.update(self.count.to_le_bytes());
        self.markers
            .observe(&mut value, self.mode, &mut Vec::new())?;
        record_digest(&mut self.values, &value)
    }

    pub(crate) fn finish(self, terminal: &Value) -> TestResult<Evidence> {
        let refs = self.markers.source_refs;
        if self.markers.encrypted + self.markers.unencrypted + self.markers.unknown == 0
            || (matches!(self.mode, ProbeMode::Full) && refs == 0)
        {
            return Err(
                "the SDK probe did not verify its expected field markers or Full references".into(),
            );
        }
        let mut coverage = Sha256::new();
        record_digest(&mut coverage, terminal)?;
        let snapshot = self.snapshot.map_or_else(
            || Ok(SnapshotSizes::none()),
            |snapshot| snapshot.finish(terminal),
        )?;
        Ok(Evidence {
            values_sha256: finish_lower(self.values)?,
            tri_state_sha256: finish_lower(self.markers.hash)?,
            typed_completeness_sha256: finish_lower(self.typed_complete)?,
            scope_coverage_sha256: finish_lower(coverage)?,
            encrypted_markers: self.markers.encrypted,
            unencrypted_markers: self.markers.unencrypted,
            unknown_markers: self.markers.unknown,
            wire_source_ref_occurrences: refs,
            max_owner_source_uses: self.max_owner_source_uses,
            max_owner_physical_sources: self.max_owner_physical_sources,
            snapshot,
        })
    }
}

impl Evidence {
    pub(crate) fn value_marker_digests(&self) -> [&str; 3] {
        [
            &self.values_sha256,
            &self.tri_state_sha256,
            &self.scope_coverage_sha256,
        ]
    }
}

#[derive(Default)]
struct MarkerEvidence {
    hash: Sha256,
    encrypted: usize,
    unencrypted: usize,
    unknown: usize,
    source_refs: usize,
}

impl MarkerEvidence {
    fn observe(&mut self, value: &mut Value, mode: ProbeMode, path: &mut Vec<u8>) -> TestResult {
        match value {
            Value::Object(fields) => {
                self.remove_proof(fields, mode, path)?;
                for (key, child) in fields {
                    let position = path.len();
                    path.push(b'k');
                    path.extend_from_slice(&key.len().to_le_bytes());
                    path.extend_from_slice(key.as_bytes());
                    self.observe(child, mode, path)?;
                    path.truncate(position);
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter_mut().enumerate() {
                    let position = path.len();
                    path.push(b'i');
                    path.extend_from_slice(&index.to_le_bytes());
                    self.observe(child, mode, path)?;
                    path.truncate(position);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn remove_proof(
        &mut self,
        fields: &mut Map<String, Value>,
        mode: ProbeMode,
        path: &[u8],
    ) -> TestResult {
        let Some(encryption) = fields.remove(ENCRYPTED) else {
            if fields.contains_key(REFS) || fields.contains_key(COMPLETE) {
                return Err("an origin proof has no direct tri-state encryption marker".into());
            }
            return Ok(());
        };
        let refs = fields.remove(REFS);
        let complete = fields.remove(COMPLETE);
        match (mode, refs, complete) {
            (ProbeMode::Full, Some(Value::Array(refs)), Some(Value::Bool(_)))
                if refs.iter().all(Value::is_string) =>
            {
                self.source_refs = self
                    .source_refs
                    .checked_add(refs.len())
                    .ok_or("SDK wire reference count overflow")?;
            }
            (ProbeMode::MarkersOnly, None, None) => {}
            _ => return Err("a sourced SDK cell has the wrong mode-specific proof shape".into()),
        }
        let marker = match encryption {
            Value::Bool(true) => {
                self.encrypted += 1;
                b't'
            }
            Value::Bool(false) => {
                self.unencrypted += 1;
                b'f'
            }
            Value::Null => {
                self.unknown += 1;
                b'n'
            }
            _ => return Err("an SDK encryption marker is outside its tri-state domain".into()),
        };
        self.hash.update(b"m");
        self.hash.update(path.len().to_le_bytes());
        self.hash.update(path);
        self.hash.update([marker]);
        Ok(())
    }
}

fn record_digest(hash: &mut Sha256, value: &impl Serialize) -> TestResult {
    update_json_digest(hash, value).map_err(|_| "hash one technical SDK value")?;
    hash.update([0]);
    Ok(())
}

pub(crate) fn finish_lower(hash: Sha256) -> Result<String, fmt::Error> {
    let digest = hash.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}")?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ProbeMode, Rows, TestResult, Value, Workload};

    #[test]
    fn typed_ab_trims_proof_and_detects_a_marker_move_with_the_same_counts() -> TestResult {
        let full = json!([
            {"state":"present","value":17,"containsEncryptedSource":true,"sourceUsesRefs":["technical"],"sourceSetComplete":true},
            {"state":"absent","containsEncryptedSource":null,"sourceUsesRefs":[],"sourceSetComplete":false}
        ]);
        let markers = json!([
            {"state":"present","value":17,"containsEncryptedSource":true},
            {"state":"absent","containsEncryptedSource":null}
        ]);
        let moved = json!([
            {"state":"present","value":17,"containsEncryptedSource":null},
            {"state":"absent","containsEncryptedSource":true}
        ]);
        let full = projected(ProbeMode::Full, &full)?;
        let markers = projected(ProbeMode::MarkersOnly, &markers)?;
        let moved = projected(ProbeMode::MarkersOnly, &moved)?;
        assert_eq!(
            full.values_sha256, markers.values_sha256,
            "only proof was omitted"
        );
        assert_eq!(
            full.tri_state_sha256, markers.tri_state_sha256,
            "direct markers were preserved"
        );
        assert_eq!(
            markers.values_sha256, moved.values_sha256,
            "the payload values stayed fixed"
        );
        assert_ne!(
            markers.tri_state_sha256, moved.tri_state_sha256,
            "marker positions are binding"
        );
        Ok(())
    }

    #[test]
    fn typed_ab_rejects_full_proof_in_a_marker_row() {
        let full_cell =
            json!({"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true});
        assert!(
            projected(ProbeMode::MarkersOnly, &full_cell).is_err(),
            "even an empty Full proof member is mode-specific"
        );
    }

    fn projected(mode: ProbeMode, payload: &Value) -> TestResult<super::Evidence> {
        let mut rows = Rows::new(mode, Workload::Verification)?;
        rows.observe_row(payload)?;
        rows.finish(&json!({"rows":1}))
    }
}
