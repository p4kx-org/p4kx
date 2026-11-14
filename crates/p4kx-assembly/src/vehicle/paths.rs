// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(super) const DIRECTORY: &str = "Scripts/Entities/Vehicles/Implementations/Xml";

/// Returns `path` with `/` as separator and without a leading `Data`, or `None` when it has a
/// `.` or `..` segment or does not lie below the implementation folder.
pub(super) fn confine(path: &str) -> Option<String> {
    let path = path.trim().replace('\\', "/");
    let mut segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments
        .first()
        .is_some_and(|first| first.eq_ignore_ascii_case("data"))
    {
        segments.remove(0);
    }
    if segments
        .iter()
        .any(|part| matches!(*part, "." | "..") || part.contains(':'))
    {
        return None;
    }
    let root: Vec<&str> = DIRECTORY.split('/').collect();
    let inside = segments.len() > root.len()
        && segments
            .iter()
            .zip(&root)
            .all(|(part, wanted)| part.eq_ignore_ascii_case(wanted));
    inside.then(|| segments.join("/"))
}

/// Returns whether `name` can be a file name: it is not empty and holds no separator.
pub(super) fn usable_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':']) && !matches!(name, "." | "..")
}

/// Returns the path of a patch file: relative to the implementation folder unless it starts at
/// `Scripts` or `Data`, and with `.xml` when it has no extension.
pub(super) fn patch_path(file: &str) -> String {
    let file = file.trim().replace('\\', "/");
    let file = file.trim_start_matches('/');
    let rooted = ["scripts/", "data/"].iter().any(|root| {
        file.get(..root.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(root))
    });
    let mut path = if rooted {
        file.to_owned()
    } else {
        format!("{DIRECTORY}/{file}")
    };
    let has_extension = path
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'));
    if !has_extension {
        path.push_str(".xml");
    }
    path
}

/// Returns the file name of `path` without its folders and extension.
pub(super) fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}
