// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;

use p4kx_extraction::model::{MaterialLibrary, MaterialReadError, MaterialSourceLimits};
use p4kx_extraction::{
    Archive, ArchiveWatch, Entry, MaterialAuthoredInfo, MaterialLibrarySourceInfo,
};

use super::material_source_census_compare as compare;
use super::material_source_census_fault::{Fault, Stage};

#[derive(Default)]
struct Counts {
    sources: usize,
    roots: usize,
    partial: usize,
    materials: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "the real-Archive census reports only aggregate technical counts"
)]
fn every_material_source_compares_retained_reader_fields_with_the_shared_export_wire_in_memory()
-> Result<(), Box<dyn Error>> {
    let first =
        p4kx_testkit::real_archive_path().map_err(|error| Fault::with(Stage::Setup, error))?;
    let mut paths = vec![first];
    if let Some(second) = p4kx_testkit::real_archive_b_path() {
        paths.push(second);
    }
    let key = p4kx_testkit::real_archive_key().map_err(|error| Fault::with(Stage::Setup, error))?;
    let archives = paths.into_iter().enumerate();
    for (ordinal, path) in archives {
        let archive = Archive::open_with_key(path, Some(key.clone()))
            .map_err(|error| Fault::with(Stage::Archive, error))?;
        let counts = census(&archive)?;
        println!(
            "material-source-census archive={} sources={} retained_roots={} partial={} materials={}",
            ordinal + 1,
            counts.sources,
            counts.roots,
            counts.partial,
            counts.materials
        );
    }
    Ok(())
}

fn census(archive: &Archive) -> Result<Counts, Fault> {
    let watch = ArchiveWatch::new(archive);
    watch
        .check()
        .map_err(|error| Fault::with(Stage::Changed, error))?;
    let mut counts = Counts::default();
    let entries = archive.entries().iter().filter(|entry| {
        entry
            .path()
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("mtl"))
    });
    for entry in entries {
        watch
            .poll()
            .map_err(|error| Fault::with(Stage::Changed, error))?;
        let bytes = input(archive, entry, &watch)?;
        let library = MaterialLibrary::parse_with(
            entry.path(),
            &bytes,
            MaterialSourceLimits::default(),
            || {
                watch
                    .poll()
                    .map_err(|error| Fault::with(Stage::Changed, error))
            },
        )
        .map_err(|error| match error {
            MaterialReadError::Interrupted(error) => error,
            other => Fault::with(Stage::Material, other),
        })?;
        counts.sources += 1;
        verify(&library)?;
        counts.roots += usize::from(library.authored.root.is_some());
        counts.partial += usize::from(
            !library.authored.losses.is_empty()
                || library
                    .materials
                    .iter()
                    .any(|material| !material.truncated.is_empty()),
        );
        counts.materials += library.materials.len();
    }
    watch
        .check()
        .map_err(|error| Fault::with(Stage::Changed, error))?;
    if counts.sources == 0 || counts.roots == 0 {
        return Err(Fault::new(Stage::Coverage));
    }
    Ok(counts)
}

fn verify(library: &MaterialLibrary) -> Result<(), Fault> {
    let value = serde_json::to_value(MaterialLibrarySourceInfo::of(library))
        .map_err(|error| Fault::with(Stage::Serialize, error))?;
    let wire: MaterialLibrarySourceInfo<'static> =
        serde_json::from_value(value).map_err(|error| Fault::with(Stage::Serialize, error))?;
    if !compare::root(&library.authored, &wire) {
        return Err(Fault::new(Stage::Compare));
    }
    for material in &library.materials {
        let value = serde_json::to_value(MaterialAuthoredInfo::of(material))
            .map_err(|error| Fault::with(Stage::Serialize, error))?;
        let wire: MaterialAuthoredInfo<'static> =
            serde_json::from_value(value).map_err(|error| Fault::with(Stage::Serialize, error))?;
        if !compare::material(material, &wire) {
            return Err(Fault::new(Stage::Compare));
        }
    }
    Ok(())
}

fn input(archive: &Archive, entry: &Entry, watch: &ArchiveWatch) -> Result<Vec<u8>, Fault> {
    let limit = MaterialSourceLimits::default().source_bytes;
    if entry.uncompressed_size()
        > u64::try_from(limit).map_err(|error| Fault::with(Stage::Limit, error))?
    {
        return Err(Fault::new(Stage::Limit));
    }
    let mut reader = archive
        .open_entry(entry)
        .map_err(|error| Fault::with(Stage::Read, error))?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 16 << 10];
    loop {
        watch
            .poll()
            .map_err(|error| Fault::with(Stage::Changed, error))?;
        let count = reader
            .read(&mut buffer)
            .map_err(|error| Fault::with(Stage::Read, error))?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes
            .len()
            .checked_add(count)
            .is_none_or(|size| size > limit)
        {
            return Err(Fault::new(Stage::Limit));
        }
        bytes.extend_from_slice(buffer.get(..count).ok_or_else(|| Fault::new(Stage::Read))?);
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::error::Error;
    use std::io;

    use p4kx_extraction::MaterialLibrarySourceInfo;
    use p4kx_extraction::model::MaterialLibrary;

    use super::{Fault, Stage, compare};

    #[test]
    fn a_whole_lost_root_or_altered_attribute_fails_the_independent_field_check()
    -> Result<(), Box<dyn Error>> {
        let source = MaterialLibrary::parse(
            "synthetic/coverage",
            b"<Material Keep='source'><Unknown/></Material>",
        )?;
        let mut wire = MaterialLibrarySourceInfo::of(&source).into_owned();
        assert!(
            compare::root(&source.authored, &wire),
            "baseline retained fields agree"
        );
        wire.root
            .as_mut()
            .ok_or("root")?
            .syntax
            .attributes
            .first_mut()
            .ok_or("attribute")?
            .value = Cow::Borrowed("mutated");
        assert!(
            !compare::root(&source.authored, &wire),
            "a changed original value is not accepted merely because a row exists"
        );
        wire.root = None;
        assert!(
            !compare::root(&source.authored, &wire),
            "whole-source omission is not a survivor-only success"
        );
        Ok(())
    }

    #[test]
    fn private_original_errors_are_retained_but_never_displayed_or_chained()
    -> Result<(), Box<dyn Error>> {
        let marker = "private-sentinel-source-path-and-values";
        let fault = Fault::with(Stage::Read, io::Error::other(marker));
        assert!(
            !format!("{fault:?} {fault}").contains(marker),
            "console output remains purely technical"
        );
        assert!(
            fault.source().is_none(),
            "automatic error formatting cannot traverse private source details"
        );
        let original = fault
            .original()
            .and_then(|error| error.downcast_ref::<io::Error>())
            .ok_or("original typed error")?;
        assert_eq!(
            original.to_string(),
            marker,
            "original error payload remains internally inspectable"
        );
        Ok(())
    }
}
