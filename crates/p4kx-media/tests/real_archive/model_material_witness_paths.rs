// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(super) struct MaterialPath {
    pub(super) filename: String,
    pub(super) beside_model: bool,
}

pub(super) fn authored(value: &str, fold_leading_backslash: bool) -> Option<MaterialPath> {
    if !fold_leading_backslash && value.trim_start().starts_with('\\') {
        return None;
    }
    let value = value.trim().replace('\\', "/");
    let candidate = virtual_path(&value)?;
    let token = without_mtl(candidate);
    let token = strip_data(token);
    if !plain(token) {
        return None;
    }
    Some(MaterialPath {
        filename: format!("{token}.mtl"),
        beside_model: !value.starts_with('/') && !token.contains('/'),
    })
}

fn virtual_path(value: &str) -> Option<&str> {
    let Some(authoring) = value.strip_prefix("//") else {
        return Some(value.strip_prefix('/').unwrap_or(value));
    };
    if !plain(authoring) {
        return None;
    }
    let mut roots = authoring.match_indices('/').filter_map(|(position, _)| {
        let tail = authoring.get(position + 1..)?;
        let (first, _) = tail.split_once('/').unwrap_or((tail, ""));
        first
            .eq_ignore_ascii_case("Data")
            .then_some((position, tail))
    });
    let (position, tail) = roots.next()?;
    if !authoring.get(..position)?.contains('/')
        || roots.next().is_some()
        || authoring.split('/').next()?.eq_ignore_ascii_case("Data")
    {
        return None;
    }
    tail.split_once('/').map(|(_, relative)| relative)
}

fn without_mtl(value: &str) -> &str {
    match value.rsplit_once('.') {
        Some((stem, extension)) if extension.eq_ignore_ascii_case("mtl") => stem,
        _ => value,
    }
}

fn strip_data(value: &str) -> &str {
    match value.split_once('/') {
        Some((prefix, rest)) if prefix.eq_ignore_ascii_case("Data") => rest,
        _ => value,
    }
}

fn plain(value: &str) -> bool {
    value.split('/').all(|part| {
        !matches!(part, "" | "." | "..") && !part.chars().any(|c| c.is_control() || c == ':')
    })
}

#[test]
fn routes_only_authored_virtual_material_paths() -> crate::TestResult {
    for (name, fold_leading, target, sibling) in [
        ("surface.MTL", false, "surface.mtl", true),
        ("Data/surface.mtl", false, "surface.mtl", true),
        ("/Data/surface.mtl", true, "surface.mtl", false),
        (r"\Shared\Paint.MTL", true, "Shared/Paint.mtl", false),
        (r"Data\Shared\Paint", false, "Shared/Paint.mtl", false),
        (
            "//server/share/Data/Shared/Paint",
            false,
            "Shared/Paint.mtl",
            false,
        ),
    ] {
        let path = authored(name, fold_leading).ok_or("admitted authored material path")?;
        assert_eq!(path.filename, target, "virtual material name");
        assert_eq!(
            path.beside_model, sibling,
            "physical sibling search condition"
        );
    }
    for (name, fold_leading) in [
        (r"\Shared\Paint.MTL", false),
        ("../escape", true),
        ("//server/Data/Shared/Paint", true),
        ("//server/share/Data/other/Data/name", true),
        ("C:/paint.mtl", true),
    ] {
        assert!(
            authored(name, fold_leading).is_none(),
            "invalid virtual path"
        );
    }
    Ok(())
}
