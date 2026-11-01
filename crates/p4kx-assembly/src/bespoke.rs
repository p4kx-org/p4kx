// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The own skins of a story character's head: the naming rule of ADR-0109 that the armour and the
//! entity path share.

use std::sync::Arc;

use crate::head_rig::rig_part;
use crate::{Geometry, ModelFailure, ModelProvider};

const HEAD_SUFFIX: &str = "_head";

/// Where the skins of a story character's own head are: the folder of the head's material
/// library, the prefix of the file names and that library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bespoke {
    pub(crate) folder: String,
    pub(crate) prefix: String,
    pub(crate) library: String,
}

impl Bespoke {
    /// Returns the own skins of the head Entity `class_name` whose material library is
    /// `library`; `None` when the library names no folder, so the head has none of its own.
    ///
    /// The prefix is the class name without `_head`, e.g. `eckhart_t2`.
    pub(crate) fn of_head(class_name: &str, library: &str) -> Option<Self> {
        let folder = folder_of(library)?;
        let prefix = class_name
            .len()
            .checked_sub(HEAD_SUFFIX.len())
            .and_then(|cut| Some((cut, class_name.get(cut..)?)))
            .filter(|(_, tail)| tail.eq_ignore_ascii_case(HEAD_SUFFIX))
            .and_then(|(cut, _)| class_name.get(..cut))
            .unwrap_or(class_name);
        Some(Self {
            folder,
            prefix: prefix.to_owned(),
            library: library.to_owned(),
        })
    }

    /// Returns the file of the head's own skin that stands for the generic skin at `path`:
    /// `<prefix>_<part>.skin` in the folder, where `<part>` is what the generic file's name adds
    /// to its rig's name, e.g. `head` or `eyes`. `None` for a file that is no generic head skin.
    /// Whether the Archive holds the file is the caller's to check.
    pub(crate) fn skin_for(&self, path: &str) -> Option<String> {
        let name = path.rsplit(['/', '\\']).next()?;
        let (stem, extension) = name.rsplit_once('.')?;
        if !extension.eq_ignore_ascii_case("skin") {
            return None;
        }
        let part = rig_part(stem)?;
        Some(format!("{}/{}_{part}.skin", self.folder, self.prefix))
    }
}

/// The own skin of a part of a story character's head, in the Loadout tree: the geometry that
/// names the skin, drawn when the Archive holds it, and the material library of the head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnSkin {
    pub(crate) geometry: Geometry,
    pub(crate) head_library: Arc<str>,
}

/// Returns whether the Archive holds the file at `path`, a path below `Data`; a file that cannot
/// be read still exists. A provider without file access, see [`ModelProvider::file`], holds none.
pub(crate) fn exists(path: &str, models: &dyn ModelProvider) -> bool {
    !matches!(models.file(path, 0), Err(ModelFailure::Missing))
}

/// A material library the Archive lacks, and what stands for it.
pub(crate) struct MissingLibrary {
    /// The library of the head, for a part that has an own skin.
    pub(crate) replacement: Option<String>,
    pub(crate) reason: String,
}

/// Returns what stands for the material library `library` of a part when `is_missing` says the
/// Archive lacks it, `None` when it is there. A bare name is looked for beside the model, so only
/// a name with a folder is checked. A part with an own skin takes `head_library`.
pub(crate) fn missing_library(
    library: &str,
    head_library: Option<&str>,
    is_missing: impl FnOnce(&str) -> bool,
) -> Option<MissingLibrary> {
    if !library.contains(['/', '\\']) || !is_missing(library) {
        return None;
    }
    let reason = head_library.map_or_else(
        || format!("the material library {library} does not exist"),
        |head| {
            format!(
                "the material library {library} does not exist; the library of the head, {head}, is used"
            )
        },
    );
    Some(MissingLibrary {
        replacement: head_library.map(str::to_owned),
        reason,
    })
}

/// Returns the folder part of `path`, with `/` as separator.
fn folder_of(path: &str) -> Option<String> {
    let unified = path.replace('\\', "/");
    unified
        .rsplit_once('/')
        .map(|(folder, _)| folder.to_owned())
        .filter(|folder| !folder.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{Bespoke, missing_library};

    const LIBRARY: &str =
        "Objects/Characters/Human/heads/male/pu/eckhart/eckhart_t2_head_material.mtl";

    #[test]
    fn the_prefix_is_the_class_name_without_head() {
        let own = Bespoke::of_head("eckhart_t2_head", LIBRARY);
        assert_eq!(
            own.as_ref().map(|own| own.prefix.as_str()),
            Some("eckhart_t2")
        );
        assert_eq!(
            own.map(|own| own.folder),
            Some("Objects/Characters/Human/heads/male/pu/eckhart".to_owned())
        );
    }

    #[test]
    fn a_library_without_a_folder_gives_no_own_skins() {
        assert_eq!(Bespoke::of_head("x_head", "head_material"), None);
    }

    #[test]
    fn a_generic_skin_maps_to_the_own_skin_of_its_part() {
        let own = Bespoke::of_head("eckhart_t2_head", LIBRARY);
        let teeth =
            "Objects/Characters/Human/heads/male/pu/protos_human_male_face_t1_pu_teeth.skin";
        assert_eq!(
            own.as_ref().and_then(|own| own.skin_for(teeth)),
            Some("Objects/Characters/Human/heads/male/pu/eckhart/eckhart_t2_teeth.skin".to_owned())
        );
        assert_eq!(
            own.and_then(|own| own.skin_for("Objects/x/eckhart_t2_eyebrow.skin")),
            None
        );
    }

    #[test]
    fn only_a_library_with_a_folder_that_is_missing_has_a_replacement() {
        assert!(missing_library("bare_name", Some("head"), |_| true).is_none());
        assert!(missing_library("a/b.mtl", Some("head"), |_| false).is_none());
        let missing = missing_library("a/b.mtl", Some("head"), |_| true);
        assert_eq!(
            missing.and_then(|missing| missing.replacement).as_deref(),
            Some("head")
        );
        let plain = missing_library("a/b.mtl", None, |_| true);
        assert!(plain.is_some_and(|plain| plain.replacement.is_none()));
    }
}
