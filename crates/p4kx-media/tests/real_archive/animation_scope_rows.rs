// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_media::{Identity, MediaRecord};
use serde_json::Value;

use super::sources::Document;
use super::{TestResult, clips, index_field, manifests, require};

pub(super) fn observe(
    base: &Path,
    row: &MediaRecord,
    documents: &mut BTreeMap<usize, Document<'_>>,
) -> TestResult {
    let Identity::Entry { entry, .. } = &row.source.identity else {
        require(
            row.subresource.namespace == "animation-definition-event-binding",
            "recognized non-document animation row",
        )?;
        let payload = row.data.get("payload").ok_or("event binding payload")?;
        for (field, family) in [
            ("/parameter_entry_index", "chrparams"),
            ("/event_database_entry_index", "animevents"),
        ] {
            let index = index_field(payload, field).ok_or("event binding physical input")?;
            let input = documents
                .get(&index)
                .ok_or("binding source outside selected closure")?;
            require(
                input.family() == family,
                "binding has exact selected source roles",
            )?;
        }
        return Ok(());
    };
    let source = documents
        .get_mut(&entry.entry_index)
        .ok_or("unselected physical document")?;
    require(
        entry.path == source.entry.path(),
        "original physical Entry path",
    )?;
    if row.subresource.namespace == clips::NAMESPACE {
        return source.clips.observe(row);
    }
    source.actual.observe(&row.subresource.namespace);
    if row.subresource.namespace == "animation-definition-document" {
        require(
            row.data.pointer("/payload/family").and_then(Value::as_str) == Some(source.family()),
            "source extension and emitted definition family agree",
        )?;
        let raw = row
            .source
            .raw
            .as_ref()
            .ok_or("retained original definition")?;
        let decoded = manifests::check_payload(
            base,
            &raw.path,
            raw.size,
            &raw.digest,
            source.bytes.len() as u64,
        )?;
        require(
            decoded == source.bytes,
            "retained definition equals its verified original bytes",
        )?;
    }
    Ok(())
}
