// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, MaterialLibrarySourceInfo, ObservedEntryBinding};
use p4kx_gltf::DrawnMaterial;
use p4kx_model::{MaterialLibrary, MaterialSource};

use crate::manifest::{Issue, IssueOrigin};
use crate::manifest_materials::MaterialLibrarySourceDescription;
use crate::material_emission_origins::EmissionEvidence;
use crate::material_source_sidecar;
use crate::package_receipt::PackageFile;
use crate::{MaterialSourceReference, PackageResult, PackageWriteContext};

/// Source roots retained by their reader path, authored data and physical Entry.
#[derive(Debug, Default)]
pub(crate) struct MaterialLibrarySources {
    entries: Vec<RetainedLibrary>,
    conflicts: HashSet<(usize, usize)>,
}

#[derive(Debug)]
struct RetainedLibrary {
    owner: Arc<MaterialSource>,
    binding: Option<ObservedEntryBinding>,
    original: Option<EntryOrigin>,
    evidence: Option<EmissionEvidence>,
    description: MaterialLibrarySourceDescription,
}
impl MaterialLibrarySources {
    pub(crate) fn capture_fallback(
        &mut self,
        library: Option<&MaterialLibrary>,
        entry: Option<&EntryOrigin>,
        drawn: &[DrawnMaterial],
        issues: &mut Vec<Issue>,
    ) -> Option<usize> {
        let library = library.filter(|_| {
            drawn
                .iter()
                .any(|material| material.library_index.is_none())
        })?;
        Some(self.capture(library, entry, issues))
    }

    pub(crate) fn capture(
        &mut self,
        library: &MaterialLibrary,
        entry: Option<&EntryOrigin>,
        issues: &mut Vec<Issue>,
    ) -> usize {
        let binding = entry.map(EntryOrigin::binding);
        let entries = self.entries.iter().enumerate();
        for (index, entry) in entries {
            if entry.description.path == library.path
                && entry.binding == binding
                && (Arc::ptr_eq(&entry.owner, &library.authored)
                    || entry.owner.as_ref() == library.authored.as_ref())
            {
                return index;
            }
        }
        let index = self.entries.len();
        let authored = MaterialLibrarySourceInfo::of(library);
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
            binding,
            original: entry.cloned(),
            evidence: None,
            description: MaterialLibrarySourceDescription {
                path: library.path.clone(),
                authored_file: None,
                authored: None,
                authored_origin: None,
                emission_provenance: None,
            },
        });
        index
    }

    pub(crate) fn evidence(
        &mut self,
        index: usize,
        entry: Option<&EntryOrigin>,
    ) -> PackageResult<&EmissionEvidence> {
        let row = self
            .entries
            .get_mut(index)
            .ok_or_else(crate::material_emission_origins::missing_inventory)?;
        if row.evidence.is_none() {
            row.evidence = Some(EmissionEvidence::new(entry)?);
        }
        row.evidence
            .as_ref()
            .ok_or_else(crate::material_emission_origins::missing_inventory)
    }

    pub(crate) fn finish(&mut self, context: &PackageWriteContext<'_>) -> PackageResult<()> {
        for entry in &mut self.entries {
            context.check()?;

            if let Some(evidence) = entry.evidence.take() {
                entry.description.emission_provenance = Some(evidence.finish()?);
            }
        }
        Ok(())
    }

    pub(crate) fn write_authored(
        &mut self,
        staging: &Path,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<Vec<PackageFile>> {
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }
        let _permit =
            context.create_directory(&staging.join(material_source_sidecar::DIRECTORY))?;
        let mut files = Vec::with_capacity(self.entries.len());
        for (ordinal, entry) in self.entries.iter_mut().enumerate() {
            let file = material_source_sidecar::write(
                staging,
                ordinal,
                &entry.owner,
                entry.original.as_ref(),
                context,
            )?;
            entry.description.authored_file = Some(MaterialSourceReference::new(ordinal));
            files.push(file);
        }
        Ok(files)
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
            reason: "Distinct authored library roots share a normalized material name; each actual description is retained.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_extraction::{Archive, OpenOptions};
    use p4kx_model::MaterialLibrary;
    use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

    use super::MaterialLibrarySources;

    #[test]
    fn equal_authored_libraries_with_different_physical_inputs_stay_distinct()
    -> Result<(), Box<dyn Error>> {
        const PLAIN: &str = "Data/plain.mtl";
        const ENCRYPTED: &str = "Data/encrypted.mtl";
        const XML: &[u8] = br#"<Material Shader="Illum" Glow="2"/>"#;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("synthetic.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored(PLAIN, XML))
            .entry(TestEntry::stored(ENCRYPTED, XML).encrypted())
            .write_to(&path)?;
        let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
        let origin = |path| {
            archive
                .entries()
                .iter()
                .find(|row| row.path() == path)
                .and_then(|row| archive.entry_origin(row))
                .ok_or("physical test Entry absent")
        };
        let first = origin(PLAIN)?;
        let second = origin(ENCRYPTED)?;
        let library = MaterialLibrary::parse("same/authored", XML)?;
        let mut sources = MaterialLibrarySources::default();
        let mut issues = Vec::new();
        assert_eq!(sources.capture(&library, Some(&first), &mut issues), 0);
        assert_eq!(sources.capture(&library, Some(&first), &mut issues), 0);
        assert_eq!(sources.capture(&library, Some(&second), &mut issues), 1);
        assert_eq!(sources.capture(&library, None, &mut issues), 2);
        Ok(())
    }

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
            sources.capture(&first, None, &mut issues),
            0,
            "first observed root receives its source ordinal"
        );
        assert_eq!(
            sources.capture(&first, None, &mut issues),
            0,
            "several child materials do not clone the root"
        );
        assert_eq!(
            sources.capture(&second, None, &mut issues),
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
