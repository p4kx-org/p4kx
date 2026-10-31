// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Returns whether `path` starts with `prefix`, ignoring ASCII case and treating `\` as `/`.
pub(crate) fn starts_with_path(path: &str, prefix: &str) -> bool {
    let mut own = path.bytes();
    prefix.bytes().all(|byte| {
        own.next().is_some_and(|found| {
            let unify = |value: u8| {
                if value == b'\\' {
                    b'/'
                } else {
                    value.to_ascii_lowercase()
                }
            };
            unify(found) == unify(byte)
        })
    })
}
