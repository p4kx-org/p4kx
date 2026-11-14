// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use p4kx_extraction::{Archive, Entry};
use serde_json::{Value, json};

use super::original::{NAME_BYTES, Original};
use super::{Check, wire};

const PROBE_SLOTS: usize = 8;

pub(super) fn compare(
    scene: &Value,
    stream: &Value,
    source: &Value,
    layers: [&Archive; 2],
    parent: &Entry,
    original: &Original<'_>,
) -> Check<bool> {
    let range = &original.name_ranges[0];
    let names = original
        .bytes
        .get(range.clone())
        .ok_or("original model path table")?;
    let mut probes: Vec<_> = names
        .as_chunks::<NAME_BYTES>()
        .0
        .iter()
        .take(PROBE_SLOTS)
        .enumerate()
        .map(|(ordinal, bytes)| Probe::new(ordinal, bytes))
        .collect();
    for (depth, layer) in layers.into_iter().enumerate() {
        apply_layer(layer, depth, &mut probes);
    }
    let scope = Scope {
        sources: wire::array(&scene["sourceInventory"], "sources")?,
        uses: wire::array(&scene["sourceInventory"], "uses")?,
        source,
        parent,
    };
    let references = wire::array(stream, "geometry")?;
    let mut matched = false;
    for probe in &probes {
        let reference = references
            .iter()
            .find(|row| {
                row["chunkOrdinal"] == original.ordinal && row["modelOrdinal"] == probe.ordinal
            })
            .ok_or("physical model slot geometry reference")?;
        let start = original.header.offset() + (range.start + probe.ordinal * NAME_BYTES) as u64;
        scope.compare(
            &reference["target"],
            probe,
            original.ordinal,
            start..start + NAME_BYTES as u64,
        )?;
        matched |= probe
            .matches
            .as_ref()
            .is_some_and(|matches| !matches.entries.is_empty());
    }
    Ok(matched)
}

struct Probe {
    ordinal: usize,
    keys: Option<[String; 2]>,
    matches: Option<Matches>,
}

impl Probe {
    fn new(ordinal: usize, bytes: &[u8; NAME_BYTES]) -> Self {
        let keys = path(bytes).map(|path| {
            let key = normalize(path);
            [format!("data/{key}"), key]
        });
        Self {
            ordinal,
            keys,
            matches: None,
        }
    }
}

struct Matches {
    depth: usize,
    kind: &'static str,
    entries: Vec<usize>,
}

fn path(bytes: &[u8; NAME_BYTES]) -> Option<&str> {
    let end = bytes.iter().position(|&byte| byte == 0)?;
    let text = std::str::from_utf8(bytes.get(..end)?)
        .ok()?
        .trim()
        .trim_start_matches(['/', '\\']);
    if text.is_empty()
        || text.split(['/', '\\']).any(|part| {
            matches!(part, "" | "." | "..") || part.chars().any(|c| c.is_control() || c == ':')
        })
    {
        return None;
    }
    Some(text)
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn apply_layer(archive: &Archive, depth: usize, probes: &mut [Probe]) {
    let mut keys: HashMap<String, Vec<usize>> = probes
        .iter()
        .filter(|row| row.matches.is_none())
        .filter_map(|row| row.keys.as_ref())
        .flatten()
        .map(|key| (key.clone(), Vec::new()))
        .collect();
    if keys.is_empty() {
        return;
    }
    for entry in archive.entries() {
        if let Some(matches) = keys.get_mut(normalize(entry.path()).as_str()) {
            matches.push(entry.index());
        }
    }
    for probe in probes.iter_mut().filter(|row| row.matches.is_none()) {
        let Some([prefixed, relative]) = &probe.keys else {
            continue;
        };
        for (kind, key) in [("dataPrefixed", prefixed), ("relative", relative)] {
            if let Some(entries) = keys.get(key.as_str()).filter(|entries| !entries.is_empty()) {
                probe.matches = Some(Matches {
                    depth,
                    kind,
                    entries: entries.clone(),
                });
                break;
            }
        }
    }
}

struct Scope<'a> {
    sources: &'a [Value],
    uses: &'a [Value],
    source: &'a Value,
    parent: &'a Entry,
}

impl Scope<'_> {
    fn compare(
        &self,
        target: &Value,
        probe: &Probe,
        ordinal: usize,
        range: Range<u64>,
    ) -> Check<()> {
        let value = &target["value"];
        let expected_kind = match (&probe.keys, &probe.matches) {
            (None, _) => "invalidSelector",
            (_, None) => "missing",
            (_, Some(matched)) if matched.entries.len() == 1 => "matched",
            (_, Some(_)) => "ambiguous",
        };
        assert_eq!(target["state"], "present", "typed directory outcome");
        assert_eq!(
            value["type"], expected_kind,
            "first matching tier and cardinality"
        );
        let refs = wire::array(target, "sourceUsesRefs")?;
        assert!(
            refs.iter().any(|id| {
                self.uses.iter().any(|site| {
                    site["id"] == *id
                        && site["source"] == *self.source
                        && site["scope"] == "payload"
                        && site["role"] == "selector"
                        && site.pointer("/location/namespace").and_then(Value::as_str)
                            == Some("socIncludedObjects")
                        && site.pointer("/location/field").and_then(Value::as_str)
                            == Some("modelPath")
                        && site.pointer("/location/indices")
                            == Some(&json!([
                                ordinal.to_string(),
                                range.start.to_string(),
                                range.end.to_string()
                            ]))
                })
            }),
            "target retains its original model path byte range"
        );
        if let Some(matched) = &probe.matches {
            self.compare_matches(value, refs, matched)?;
        }
        Ok(())
    }

    fn compare_matches(&self, value: &Value, refs: &[Value], matches: &Matches) -> Check<()> {
        assert_eq!(
            value.pointer("/tier/depth"),
            Some(&json!(matches.depth)),
            "original owner tier"
        );
        assert_eq!(
            value.pointer("/tier/kind").and_then(Value::as_str),
            Some(matches.kind),
            "matching original path form"
        );
        let sources = if matches.entries.len() == 1 {
            std::slice::from_ref(&value["source"])
        } else {
            wire::array(value, "candidates")?
        };
        assert_eq!(
            sources.len(),
            matches.entries.len(),
            "every matching physical candidate"
        );
        for (source, expected) in sources.iter().zip(&matches.entries) {
            let physical = self
                .sources
                .iter()
                .find(|row| row["id"] == *source)
                .ok_or("physical geometry target")?;
            assert_eq!(
                physical["entryIndex"], *expected,
                "directory Entry ordinal from independent scan"
            );
            self.check_ancestry(physical, matches.depth)?;
            assert!(
                refs.iter().any(|id| {
                    self.uses.iter().any(|site| {
                        site["id"] == *id
                            && site["source"] == *source
                            && site["scope"] == "archiveDirectoryMetadata"
                            && site.pointer("/location/field").and_then(Value::as_str)
                                == Some("archiveEntryPath")
                    })
                }),
                "physical geometry uses the directory byte domain"
            );
        }
        Ok(())
    }

    fn check_ancestry(&self, physical: &Value, depth: usize) -> Check<()> {
        let ancestors = wire::array(physical, "ancestors")?;
        if depth == 0 {
            let [parent] = ancestors else {
                return Err("local geometry original container ancestry".into());
            };
            let source = self
                .sources
                .iter()
                .find(|row| row["id"] == *parent)
                .ok_or("geometry enclosing source")?;
            assert_eq!(
                source["entryIndex"],
                self.parent.index(),
                "local matched directory belongs to selected container"
            );
        } else {
            assert!(
                ancestors.is_empty(),
                "archive-level fallback has no container ancestor"
            );
        }
        Ok(())
    }
}
