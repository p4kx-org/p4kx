// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files that the material libraries of a scene could not resolve, counted once each.

use std::collections::HashMap;

use crate::MaterialStatus;
use crate::mtl_load::request_path;

/// The layer files missing, unreadable or over the load budget across the material libraries of
/// one scene, see the field
/// [`MaterialLibrary::unresolved_layers`](crate::MaterialLibrary::unresolved_layers).
///
/// A library lists one entry per layer file and `Submtl`, so a single library counts as the Export
/// as GLB summary does. Across libraries, spellings of one path that differ in case, in `\` and
/// `/`, in a leading `Data/` or in the `.mtl` extension are one file, as for the layer cache; it
/// counts as often as its most frequent library lists it. The count is a
/// lower bound, because each library lists at most [`MAX_LAYER_FILES`](crate::MAX_LAYER_FILES)
/// entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnresolvedLayers {
    files: HashMap<String, usize>,
    first: Option<String>,
}

impl UnresolvedLayers {
    /// Collects the unresolved layer files of `statuses`, in their order.
    #[must_use]
    pub fn of<'a>(statuses: impl IntoIterator<Item = &'a MaterialStatus>) -> Self {
        let mut all = Self::default();
        all.extend(statuses);
        all
    }

    /// Returns how many layer files are unresolved, see the type.
    #[must_use]
    pub fn count(&self) -> usize {
        self.files.values().sum()
    }

    /// Returns the first unresolved layer file, as the layer names it.
    #[must_use]
    pub fn first(&self) -> Option<&str> {
        self.first.as_deref()
    }
}

impl<'a> Extend<&'a MaterialStatus> for UnresolvedLayers {
    /// Adds the unresolved layer files of every library among `statuses`.
    fn extend<T: IntoIterator<Item = &'a MaterialStatus>>(&mut self, statuses: T) {
        for library in statuses.into_iter().filter_map(MaterialStatus::library) {
            let mut listed: HashMap<String, usize> = HashMap::new();
            for path in &library.unresolved_layers {
                if self.first.is_none() {
                    self.first = Some(path.clone());
                }
                let key = request_path(path)
                    .unwrap_or_else(|| path.replace('\\', "/"))
                    .to_ascii_lowercase();
                *listed.entry(key).or_default() += 1;
            }
            for (key, times) in listed {
                let known = self.files.entry(key).or_default();
                *known = (*known).max(times);
            }
        }
    }
}
