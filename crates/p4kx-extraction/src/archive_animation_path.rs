// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Normalizes an authored Data- or Animations-rooted archive selector.
#[must_use]
pub fn archive_animation_path(path: &str) -> Option<String> {
    if path.split(['/', '\\']).any(|part| {
        matches!(part, "" | "." | "..")
            || part.contains([':', '\0', '!', '*', '?'])
            || part.ends_with([' ', '.'])
    }) {
        return None;
    }
    let mut parts = path.split(['/', '\\']);
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("data") && parts.next().is_some() {
        Some(path.replace('\\', "/"))
    } else if first.eq_ignore_ascii_case("animations") && parts.next().is_some() {
        Some(format!("Data/{}", path.replace('\\', "/")))
    } else {
        None
    }
}
