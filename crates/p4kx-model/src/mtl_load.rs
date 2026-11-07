// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    CompanionError, CompanionPath, CompanionResolver, LayerCache, MaterialLibrary, MaterialStatus,
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
    let Some(request) = request_path(name) else {
        return MaterialStatus::Unreadable {
            path,
            reason: "the path leaves the Data folder or is no plain path".to_owned(),
        };
    };
    match fetch(&request, companions) {
        Ok(None) => MaterialStatus::Missing { path },
        Err(error) => MaterialStatus::Unreadable {
            path,
            reason: error.to_string(),
        },
        Ok(Some(bytes)) => match MaterialLibrary::parse(name, &bytes) {
            Ok(mut library) => {
                if reading == MaterialReading::Drawn {
                    crate::mtl_layers::resolve(&mut library, companions, cache);
                    crate::mtl_relief::check_diffuse_maps(&mut library, companions);
                }
                MaterialStatus::Loaded(library)
            }
            Err(error) => MaterialStatus::Unreadable {
                path,
                reason: error.to_string(),
            },
        },
    }
}

/// Fetches the library: a bare name is first looked for beside the model, as the engine does,
/// then, like any other path, below `Data`.
fn fetch(
    request: &str,
    companions: &mut dyn CompanionResolver,
) -> Result<Option<Vec<u8>>, CompanionError> {
    if !request.contains('/')
        && let Some(bytes) =
            companions.resolve(CompanionPath::Sibling(&format!("{request}.mtl")))?
    {
        return Ok(Some(bytes));
    }
    companions.resolve(CompanionPath::Material(request))
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
