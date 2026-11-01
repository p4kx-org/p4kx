// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The own skins of a story character's head: the naming rule of ADR-0109 that the armour and the
//! entity path share.

use crate::armour::rig_part;
use crate::{ModelFailure, ModelProvider};

const HEAD_SUFFIX: &str = "_head";

/// Where the skins of a story character's own head are: the folder of the head's material
/// library, the prefix of the file names and that library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bespoke {
    pub(crate) folder: String,
    pub(crate) prefix: String,
    pub(crate) library: Option<String>,
}

impl Bespoke {
    /// Returns the own skins of the head Entity `class_name` whose material library is
    /// `library`; `None` when the library names no folder, so the head has none of its own.
    ///
    /// The prefix is the class name without `_head`, e.g. `eckhart_t2`.
    pub(crate) fn of_head(class_name: &str, library: Option<&str>) -> Option<Self> {
        let library = library?;
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
            library: Some(library.to_owned()),
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

/// Returns whether the Archive holds the file at `path`, a path below `Data`; a file that cannot
/// be read still exists.
pub(crate) fn exists(path: &str, models: &dyn ModelProvider) -> bool {
    !matches!(models.file(path, 0), Err(ModelFailure::Missing))
}

/// Returns the note for a material `library` the Archive lacks and the library of the head,
/// `head_library`, that stands for it.
pub(crate) fn replaced_library_note(library: &str, head_library: &str) -> String {
    format!(
        "the material library {library} does not exist; the library of the head, {head_library}, is used"
    )
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
    use super::Bespoke;

    const LIBRARY: &str =
        "Objects/Characters/Human/heads/male/pu/eckhart/eckhart_t2_head_material.mtl";

    #[test]
    fn the_prefix_is_the_class_name_without_head() {
        let own = Bespoke::of_head("eckhart_t2_head", Some(LIBRARY));
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
        assert_eq!(Bespoke::of_head("x_head", Some("head_material")), None);
        assert_eq!(Bespoke::of_head("x_head", None), None);
    }

    #[test]
    fn a_generic_skin_maps_to_the_own_skin_of_its_part() {
        let own = Bespoke::of_head("eckhart_t2_head", Some(LIBRARY));
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
}
