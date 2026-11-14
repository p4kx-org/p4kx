// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    CompanionPath, CompanionResolver, LayerCache, MaterialCompanion, MaterialCompanionUse,
    MaterialLibrary, MaterialStatus,
};

/// How much of the material library a model names is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialReading {
    /// Not at all: the status is [`MaterialStatus::Skipped`].
    Skipped,
    /// The library only, as a listing needs it: the layers keep no
    /// [`LayerSurface`](crate::LayerSurface) and no diffuse map is checked.
    Plain,
    /// The library with the surfaces of its layers and its checked diffuse maps, as a caller that
    /// draws needs it.
    Drawn,
}

/// Fetches and reads the material library `name` through `companions` as `reading` says, taking
/// the layer files from `cache`; a library that is missing, unsafe to ask for or unreadable is a
/// status, not a failure of the model.
pub(crate) fn load(
    name: Option<&str>,
    companions: &mut dyn CompanionResolver,
    reading: MaterialReading,
    cache: &LayerCache,
) -> MaterialStatus {
    if reading == MaterialReading::Skipped {
        return MaterialStatus::Skipped;
    }
    let Some(name) = name else {
        return MaterialStatus::Unnamed;
    };
    let path = name.to_owned();
    let request = match companions.material_request(name) {
        Ok(request) => request,
        Err(error) => {
            return MaterialStatus::Unreadable {
                path,
                reason: error.to_string(),
                target: None,
            };
        }
    };
    let fetched = fetch(&request, companions);
    let target = request.into_target();
    companions.observe_material(MaterialCompanionUse::Library, fetched.origin.as_ref());
    match fetched.bytes {
        Ok(None) => MaterialStatus::Missing { path, target },
        Err(error) => MaterialStatus::Unreadable {
            path,
            reason: error.to_string(),
            target,
        },
        Ok(Some(bytes)) => match MaterialLibrary::parse_with_resolver(
            name,
            &bytes,
            target.clone(),
            companions,
            reading,
            cache,
        ) {
            Ok(library) => MaterialStatus::Loaded(library),
            Err(error) => MaterialStatus::Unreadable {
                path,
                reason: error.to_string(),
                target,
            },
        },
    }
}

/// Fetches an unrooted bare name beside the model first, then searches below `Data`.
fn fetch(
    request: &crate::MaterialRequest,
    companions: &mut dyn CompanionResolver,
) -> MaterialCompanion {
    let path = request.path();
    if request.searches_sibling() && !path.contains('/') {
        let sibling =
            companions.resolve_material_file(CompanionPath::Sibling(&format!("{path}.mtl")));
        if !matches!(&sibling.bytes, Ok(None)) {
            return sibling;
        }
    }
    companions.resolve_material_file(CompanionPath::Material(path))
}

/// Returns `name` with `/` separators and without a leading `Data/` or a `.mtl` extension, or
/// `None` when it is empty, absolute, names a drive or stream, or has an empty, `.` or `..`
/// component.
#[must_use]
pub fn request_path(name: &str) -> Option<String> {
    let path = name.trim().replace('\\', "/");
    let path = match path.len().checked_sub(4).and_then(|at| path.get(at..)) {
        Some(extension) if extension.eq_ignore_ascii_case(".mtl") => {
            path.get(..path.len() - 4)?.to_owned()
        }
        _ => path,
    };
    let path = match path.get(..5) {
        Some(prefix) if prefix.eq_ignore_ascii_case("data/") => path.get(5..)?.to_owned(),
        _ => path,
    };
    let plain = path.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.chars().any(|c| c.is_control() || c == ':')
    });
    plain.then_some(path)
}
