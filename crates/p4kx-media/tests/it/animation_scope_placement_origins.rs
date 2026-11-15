// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_media::MediaRecord;
use serde_json::{Value, json};

use super::TestResult;

pub(super) fn verify(
    base: &Path,
    occurrences: &[(usize, &MediaRecord)],
    dataforge: &str,
) -> TestResult {
    let evidence = Evidence {
        fields: rows(base, "field-origins.jsonl")?,
        uses: rows(base, "origin-uses.jsonl")?,
        sources: rows(base, "origin-sources.jsonl")?,
        dataforge,
    };
    verify_ports(occurrences, &evidence)?;
    verify_gear(occurrences, &evidence)
}

fn verify_ports(occurrences: &[(usize, &MediaRecord)], evidence: &Evidence<'_>) -> TestResult {
    for path in ["sharedLeft", "sharedRight"] {
        let ordinal = port(occurrences, path)?;
        let field = evidence.field(ordinal, "port_path")?;
        assert_eq!(
            field["containsEncryptedSource"],
            Value::Null,
            "unproved {path} cannot be marked plain"
        );
        assert_eq!(
            field["sourceSetComplete"], false,
            "unproved {path} remains incomplete"
        );
        assert!(
            references(field)?.is_empty(),
            "unobserved name has no proof"
        );
    }
    let ordinal = port(occurrences, "defaultEmpty")?;
    evidence.exact(ordinal, "port_path", "0", "Components[1].Ports[0].Name")
}

fn verify_gear(occurrences: &[(usize, &MediaRecord)], evidence: &Evidence<'_>) -> TestResult {
    let (ordinal, gear) = occurrences
        .iter()
        .find(|(_, row)| row.data.pointer("/payload/placement") == Some(&json!("landing-gear")))
        .ok_or("landing gear receipt")?;
    assert_eq!(
        gear.data.pointer("/payload/placement_detail"),
        Some(&json!({"kind":"landing-gear", "owner_path":"", "ordinal":0, "bone":"strut"})),
        "original gear declaration belongs to the root occurrence",
    );
    let owner_path = evidence.field(*ordinal, "owner_path")?;
    assert_eq!(
        owner_path["sourceSetComplete"], true,
        "root ownership is established"
    );
    assert_eq!(
        owner_path["containsEncryptedSource"], false,
        "the empty root path is independent"
    );
    assert!(
        references(owner_path)?.is_empty(),
        "root has no port segment"
    );
    evidence.exact(*ordinal, "ordinal", "5", "gears[0]")?;
    evidence.exact(*ordinal, "bone", "5", "gears[0].bone")
}

fn port(occurrences: &[(usize, &MediaRecord)], path: &str) -> Result<usize, Box<dyn Error>> {
    let expected = json!({"kind":"attachment", "port_path":path});
    occurrences
        .iter()
        .find(|(_, row)| row.data.pointer("/payload/placement_detail") == Some(&expected))
        .map(|(ordinal, _)| *ordinal)
        .ok_or_else(|| format!("missing exact {path} occurrence").into())
}

struct Evidence<'source> {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
    dataforge: &'source str,
}

impl Evidence<'_> {
    fn field(&self, ordinal: usize, name: &str) -> Result<&Value, Box<dyn Error>> {
        let path = format!("/data/payload/placement_detail/{name}");
        let ordinal = ordinal.to_string();
        self.fields
            .iter()
            .find(|row| {
                row.pointer("/scope/stream/kind").and_then(Value::as_str)
                    == Some("animation-definition")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(path.as_str())
            })
            .ok_or_else(|| format!("missing occurrence field evidence for {path}").into())
    }

    fn exact(&self, ordinal: usize, name: &str, record: &str, member: &str) -> TestResult {
        let field = self.field(ordinal, name)?;
        assert_eq!(
            field["containsEncryptedSource"], false,
            "the authoring DataForge for {name} is plain"
        );
        assert_eq!(
            field["sourceSetComplete"], true,
            "the actual source for {name} is known"
        );
        let [reference] = references(field)? else {
            return Err(format!("{name} requires one exact source use").into());
        };
        let used = self
            .uses
            .iter()
            .find(|entry| entry.get("id") == Some(reference))
            .ok_or("original placement source use")?;
        assert_eq!(
            used["role"], "selector",
            "{name} comes from the selected declaration"
        );
        assert_eq!(
            used["scope"], "payload",
            "{name} is evidenced by DataForge payload"
        );
        assert_eq!(
            used["location"],
            json!({"type":"subresource", "namespace":"dataforge-record", "indices":[record], "field":member}),
            "each placement value has its original DataForge coordinate",
        );
        let source = self
            .sources
            .iter()
            .find(|entry| entry.get("id") == used.get("source"))
            .ok_or("original physical DataForge Entry")?;
        assert_eq!(
            source["entryIndex"].to_string(),
            self.dataforge,
            "original DataForge Entry for {name}"
        );
        Ok(())
    }
}

fn rows(base: &Path, stream: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(base.join(stream))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn references(field: &Value) -> Result<&[Value], Box<dyn Error>> {
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "placement field source references".into())
}
