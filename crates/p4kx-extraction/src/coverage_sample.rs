// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, JsonLayout, Record, RecordJson, RecordXml};

use crate::coverage::SampleExport;

use crate::coverage_origins::{SampleSection, SampleSite};

pub(super) fn reader<'forge>(
    forge: &'forge DataForge<'_>,
    mut sites: Option<&mut Vec<SampleSite<'forge>>>,
) -> impl FnMut(SampleSection, usize, u32) -> SampleExport {
    move |section, row, index| {
        let record = forge.record(index);
        if let Some(sites) = sites.as_deref_mut() {
            sites.push(SampleSite {
                section,
                row,
                record,
            });
        }
        read(record)
    }
}

fn read(record: Option<Record<'_>>) -> SampleExport {
    let Some(record) = record else {
        return failed(String::new(), "the Record is gone".to_owned());
    };
    let path = record.record_path().unwrap_or_default().to_owned();
    let tree = match record.tree() {
        Ok(tree) => tree,
        Err(error) => return failed(path, error.to_string()),
    };
    let mut xml = Vec::new();
    let mut json = Vec::new();
    let written = RecordXml::new()
        .write(&tree, &mut xml)
        .and_then(|()| RecordJson::new(JsonLayout::Pretty).write(&tree, &mut json));
    SampleExport {
        record_path: path,
        xml_bytes: xml.len(),
        json_bytes: json.len(),
        error: written.err().map(|error| error.to_string()),
    }
}

const fn failed(record_path: String, error: String) -> SampleExport {
    SampleExport {
        record_path,
        xml_bytes: 0,
        json_bytes: 0,
        error: Some(error),
    }
}
