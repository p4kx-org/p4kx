// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Returns the lower-case file stem of an inventory file `value`, a path ending in `.json` or
/// `.xml`; `None` for anything else.
pub(crate) fn stem(value: &str) -> Option<String> {
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return None;
    }
    let file = value.rsplit(['/', '\\']).next()?;
    let (stem, extension) = file.rsplit_once('.')?;
    let known = extension.eq_ignore_ascii_case("json") || extension.eq_ignore_ascii_case("xml");
    (known && !stem.is_empty()).then(|| stem.to_ascii_lowercase())
}
