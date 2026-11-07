// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_assembly::{ArchiveModels, Assembly};
use p4kx_blender::{PackageError, PackageInfo, PackageWriteContext, write_package_with_context};
use p4kx_extraction::Archive;
use p4kx_gltf::{LoadedTexture, TextureLoadError, TextureLoader};
use p4kx_guard::ArchiveWriteProtection;
use p4kx_testkit::livery_entries;
use p4kx_texture::RgbaImage;

use super::support::{self, Fixture, TestResult};

fn fixture() -> Result<(Fixture, Archive, Assembly), Box<dyn std::error::Error>> {
    let fixture = support::fixture(livery_entries())?;
    fs::create_dir(fixture.path.parent().ok_or("source parent")?.join("Bin64"))?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = support::assemble(&archive, "TEST_Livery")?;
    Ok((fixture, archive, assembly))
}

#[test]
fn package_context_refuses_the_actual_source_installation_and_allows_outside_output() -> TestResult
{
    let (_fixture, archive, assembly) = fixture()?;
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(archive.path(), &cancel)?;
    let context = PackageWriteContext::new(&protection, &cancel);
    let models = ArchiveModels::new(&archive);
    let inside = archive
        .path()
        .parent()
        .ok_or("source parent")?
        .join("package");
    let error = write_package_with_context(
        &assembly,
        &models,
        &models.textures(),
        &support::glb_options(0),
        &PackageInfo::default(),
        &inside,
        &context,
    )
    .err()
    .ok_or("source installation accepted as package output")?;
    assert!(matches!(error, PackageError::Refused(cause) if cause.installation().is_some()));
    assert!(!inside.exists());
    let output = tempfile::tempdir()?;
    let outside = output.path().join("package");
    let summary = write_package_with_context(
        &assembly,
        &models,
        &models.textures(),
        &support::glb_options(0),
        &PackageInfo::default(),
        &outside,
        &context,
    )?;
    assert!(summary.files > 1);
    assert!(outside.join("scene.json").is_file());
    Ok(())
}

struct CancelTexture<'a>(&'a AtomicBool);

impl TextureLoader for CancelTexture<'_> {
    fn load(&self, _path: &str, _mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        self.0.store(true, Ordering::Relaxed);
        let image = RgbaImage::new(1, 1, vec![1, 2, 3, 255])
            .map_err(|error| TextureLoadError::Unreadable(error.to_string()))?;
        Ok(LoadedTexture::new(image, true))
    }
}

#[test]
fn cancellation_at_a_texture_leaf_cleans_owned_staging_and_preserves_the_previous_package()
-> TestResult {
    let (_fixture, archive, assembly) = fixture()?;
    let output = tempfile::tempdir()?;
    let out = output.path().join("package");
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(archive.path(), &cancel)?;
    let context = PackageWriteContext::new(&protection, &cancel);
    let models = ArchiveModels::new(&archive);
    write_package_with_context(
        &assembly,
        &models,
        &models.textures(),
        &support::glb_options(0),
        &PackageInfo::default(),
        &out,
        &context,
    )?;
    let original = fs::read(out.join("scene.json"))?;
    let error = write_package_with_context(
        &assembly,
        &models,
        &CancelTexture(&cancel),
        &support::glb_options(0),
        &PackageInfo::default(),
        &out,
        &context,
    )
    .err()
    .ok_or("cancelled texture write succeeded")?;
    assert!(
        cancel.load(Ordering::Relaxed),
        "real loader work triggered cancellation"
    );
    assert!(matches!(error, PackageError::Cancelled));
    assert_eq!(fs::read(out.join("scene.json"))?, original);
    assert_eq!(support::names_in(output.path())?, ["package"]);
    Ok(())
}
