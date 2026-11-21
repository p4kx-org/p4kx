// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::ModelNodePropertySource;

#[derive(Default)]
pub(crate) struct NodeProperties {
    pub(crate) values: Vec<(String, String)>,
    pub(crate) sites: Vec<ModelNodePropertySource>,
}

pub(crate) fn read_entry(entry: Option<&[u8]>, mut offset: Option<u64>) -> NodeProperties {
    let mut properties = NodeProperties::default();
    let Some(entry) = entry else {
        return properties;
    };
    for line in entry.split(|&byte| byte == b'\n') {
        if let Some(parts) = parts(line) {
            properties.values.push((
                String::from_utf8_lossy(parts.key.trim_ascii()).into_owned(),
                String::from_utf8_lossy(parts.value.trim_ascii()).into_owned(),
            ));
            if let Some(start) = offset {
                properties.sites.push(ModelNodePropertySource {
                    line: start..start + line.len() as u64,
                    key: trimmed(parts.key, start),
                    value: trimmed(parts.value, start + parts.key.len() as u64 + 1),
                });
            }
        }
        offset = offset.map(|start| start + line.len() as u64 + 1);
    }
    properties
}

pub(crate) struct Parts<'a> {
    pub(crate) key: &'a [u8],
    pub(crate) value: &'a [u8],
}

pub(crate) fn parts(line: &[u8]) -> Option<Parts<'_>> {
    let separator = line.iter().position(|&byte| byte == b'=')?;
    let (key, tail) = line.split_at(separator);
    let value = tail.get(1..)?;
    (!key.trim_ascii().is_empty()).then_some(Parts { key, value })
}

pub(crate) const fn trimmed(bytes: &[u8], start: u64) -> Range<u64> {
    let text = bytes.trim_ascii_start();
    let start = start + (bytes.len() - text.len()) as u64;
    start..start + text.trim_ascii_end().len() as u64
}
