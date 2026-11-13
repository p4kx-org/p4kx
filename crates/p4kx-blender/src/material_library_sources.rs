// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::MaterialLibrarySourceInfo;
use p4kx_gltf::DrawnMaterial;
use p4kx_model::{MaterialLibrary, MaterialSource};

use crate::manifest::{Issue, IssueOrigin};
use crate::manifest_materials::MaterialLibrarySourceDescription;

/// Source root descriptions deduplicated only when both reader path and authored data agree.
#[derive(Debug, Default)]
pub(crate) struct MaterialLibrarySources {
    entries: Vec<RetainedLibrary>,
    conflicts: HashSet<(usize, usize)>,
}

#[derive(Debug)]
struct RetainedLibrary {
    owner: Arc<MaterialSource>,
    description: MaterialLibrarySourceDescription,
}
impl MaterialLibrarySources {
    pub(crate) fn capture_fallback(
        &mut self,
        library: Option<&MaterialLibrary>,
        drawn: &[DrawnMaterial],
        issues: &mut Vec<Issue>,
    ) -> Option<usize> {
        let library = library.filter(|_| {
            drawn
                .iter()
                .any(|material| material.library_index.is_none())
        })?;
        Some(self.capture(library, issues))
    }

    pub(crate) fn capture(&mut self, library: &MaterialLibrary, issues: &mut Vec<Issue>) -> usize {
        let entries = self.entries.iter().enumerate();
        for (index, entry) in entries {
            if entry.description.path == library.path
                && (Arc::ptr_eq(&entry.owner, &library.authored)
                    || entry.owner.as_ref() == library.authored.as_ref())
            {
                return index;
            }
        }
        let index = self.entries.len();
        let authored = MaterialLibrarySourceInfo::of(library).into_owned();
        if !authored.is_complete() {
            issues.push(Issue {
                origin: IssueOrigin::Material,
                kind: "material_source_projection_loss".into(),
                path: library.path.clone(),
                name: String::new(),
                skipped: false,
                reason: "Authored material root metadata has a diagnosed projection boundary; raw source and interpreted child surfaces remain separate.".into(),
            });
        }
        self.entries.push(RetainedLibrary {
            owner: Arc::clone(&library.authored),
            description: MaterialLibrarySourceDescription {
                path: library.path.clone(),
                authored,
            },
        });
        index
    }

    pub(crate) fn descriptions(&self) -> impl Iterator<Item = &MaterialLibrarySourceDescription> {
        self.entries.iter().map(|entry| &entry.description)
    }
    pub(crate) fn conflict(
        &mut self,
        known: Option<usize>,
        actual: usize,
        path: &str,
        name: &str,
    ) -> Option<Issue> {
        let known = known?;
        if known == actual || !self.conflicts.insert((known, actual)) {
            return None;
        }
        Some(Issue {
            origin: IssueOrigin::Material,
            kind: "material_source_ambiguity".into(),
            path: path.to_owned(),
            name: name.to_owned(),
            skipped: false,
            reason: "Distinct authored library roots share one renderer material identity; both source descriptions are retained.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_model::MaterialLibrary;

    use super::MaterialLibrarySources;

    #[test]
    fn the_same_authored_library_is_retained_once_and_same_path_different_roots_survive()
    -> Result<(), Box<dyn Error>> {
        let first = MaterialLibrary::parse("synthetic/library", b"<Material Root='one'><SubMaterials><Material Name='A'/><Material Name='B'/></SubMaterials></Material>")?;
        let second = MaterialLibrary::parse(
            "synthetic/library",
            b"<Material Root='two'><SubMaterials><Material Name='A'/></SubMaterials></Material>",
        )?;
        let mut sources = MaterialLibrarySources::default();
        let mut issues = Vec::new();
        assert_eq!(
            sources.capture(&first, &mut issues),
            0,
            "first observed root receives its source ordinal"
        );
        assert_eq!(
            sources.capture(&first, &mut issues),
            0,
            "several child materials do not clone the root"
        );
        assert_eq!(
            sources.capture(&second, &mut issues),
            1,
            "same path does not erase a distinct authored root"
        );
        assert_eq!(
            sources.entries.len(),
            2,
            "both actual source descriptions remain available"
        );
        Ok(())
    }
}
