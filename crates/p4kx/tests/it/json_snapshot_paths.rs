// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use serde_json::Value;

pub(super) fn redact(value: &mut Value, directory: &Path) -> io::Result<()> {
    let canonical = directory.canonicalize()?;
    let canonical = canonical.to_string_lossy();
    let original = directory.to_string_lossy();
    redact_value(value, [canonical.as_ref(), original.as_ref()]);
    Ok(())
}

fn redact_value(value: &mut Value, roots: [&str; 2]) {
    match value {
        Value::String(text) => {
            if let Some(rest) = roots.into_iter().find_map(|root| text.strip_prefix(root)) {
                *text = format!("[DIR]{}", rest.replace('\\', "/"));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact_value(item, roots)),
        Value::Object(map) => map.values_mut().for_each(|item| redact_value(item, roots)),
        _ => {}
    }
}
