// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use super::DiskTextures;
use crate::texture_parts::Parts;
use crate::{Texture, UiMovie};

#[test]
fn shared_disk_parts_preserve_existing_thread_safety_and_clone_contracts() {
    fn send_sync<T: Send + Sync>() {}
    fn cloneable<T: Clone>() {}
    send_sync::<Texture<'static>>();
    send_sync::<UiMovie<'static>>();
    send_sync::<Parts<'static>>();
    send_sync::<DiskTextures>();
    cloneable::<Parts<'static>>();
    cloneable::<DiskTextures>();
}

#[cfg(target_os = "linux")]
#[test]
fn metadata_open_and_cloned_handles_keep_the_same_actual_path() -> io::Result<()> {
    use std::io::Read as _;

    let dir = tempfile::tempdir()?;
    let actual = dir.path().join("PART.dds.1");
    let requested = dir.path().join("part.DDS.1");
    fs::write(&actual, b"chosen original bytes")?;
    let files = DiskTextures::new();
    assert_eq!(files.resolve_path(&requested)?, Some(actual.clone()));
    fs::write(&requested, b"new")?;

    let copied = files.clone();
    let size = copied.part_len(&requested)?;
    let mut bytes = Vec::new();
    copied.open_file(&requested)?.read_to_end(&mut bytes)?;

    assert_eq!(size, Some(fs::metadata(&actual)?.len()));
    assert_eq!(bytes, fs::read(&actual)?);
    assert_eq!(copied.resolve_path(&requested)?, Some(actual.clone()));
    assert_eq!(files.resolve_path(&requested)?, Some(actual));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn a_bound_missing_path_does_not_switch_to_a_later_case_variant() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let requested = dir.path().join("missing.dds.1");
    let files = DiskTextures::new();
    assert_eq!(files.resolve_path(&requested)?, None);
    fs::write(dir.path().join("MISSING.DDS.1"), b"late")?;

    assert_eq!(files.resolve_path(&requested)?, None);
    assert_eq!(files.part_len(&requested)?, None);
    assert_eq!(
        files
            .open_file(&requested)
            .expect_err("retained absence")
            .kind(),
        io::ErrorKind::NotFound
    );
    Ok(())
}

#[test]
fn exact_paths_do_not_depend_on_a_stale_fallback_listing() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let files = DiskTextures::new();
    assert_eq!(files.resolve_path(&dir.path().join("absent"))?, None);
    let exact = dir.path().join("new.dds.1");
    fs::write(&exact, b"exact")?;

    assert_eq!(files.part_len(&exact)?, Some(5));
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_failed_binding_keeps_the_original_os_error_after_a_folder_is_replaced() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("blocked");
    fs::write(&folder, b"not a directory")?;
    let requested = folder.join("part.dds.1");
    let expected = fs::metadata(&requested).expect_err("invalid directory component");
    let files = DiskTextures::new();
    let first = files
        .resolve_path(&requested)
        .expect_err("metadata failure");
    fs::remove_file(&folder)?;
    fs::create_dir(&folder)?;
    fs::write(&requested, b"new exact input")?;

    let repeated = files.open_file(&requested).expect_err("retained failure");

    assert_eq!(first.kind(), io::ErrorKind::NotADirectory);
    assert_eq!(first.raw_os_error(), expected.raw_os_error());
    assert_eq!(repeated.kind(), first.kind());
    assert_eq!(repeated.raw_os_error(), first.raw_os_error());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a non-root Linux process with directory read permission enforcement"]
fn native_unreadable_fallback_is_retained_after_permissions_are_restored() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("restricted");
    fs::create_dir(&folder)?;
    fs::write(folder.join("PART.dds.1"), b"part")?;
    let original = fs::metadata(&folder)?.permissions();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o111))?;
    let files = DiskTextures::new();
    let requested = folder.join("part.dds.1");
    let denied = files.resolve_path(&requested);
    fs::set_permissions(&folder, original)?;

    let error = denied.expect_err("native directory permission denial");
    let repeated = files.open_file(&requested).expect_err("retained denial");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(repeated.raw_os_error(), error.raw_os_error());
    assert_eq!(repeated.kind(), error.kind());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a non-root Linux process with file read permission enforcement"]
fn native_unreadable_exact_input_never_chooses_a_readable_other_case() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir()?;
    let exact = dir.path().join("part.dds.1");
    fs::write(&exact, b"exact")?;
    fs::write(dir.path().join("PART.DDS.1"), b"other case")?;
    let original = fs::metadata(&exact)?.permissions();
    fs::set_permissions(&exact, fs::Permissions::from_mode(0o000))?;
    let files = DiskTextures::new();
    let denied = files.open_file(&exact);
    fs::set_permissions(&exact, original)?;

    assert_eq!(
        denied
            .expect_err("native exact-file permission denial")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a non-root Linux process with directory read permission enforcement"]
fn native_alpha_lookup_failure_is_deferred_and_cannot_be_silently_dropped() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use p4kx_testkit::{SplitDds, TestTextureFormat};

    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("restricted");
    fs::create_dir(&folder)?;
    let header = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .alpha(TestTextureFormat::R8)
        .write_files(&folder, "glass.dds")?;
    fs::rename(folder.join("glass.dds.a"), folder.join("GLASS.DDS.A"))?;
    let original = fs::metadata(&folder)?.permissions();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o111))?;
    let opened = Texture::from_file(&header);
    fs::set_permissions(&folder, original)?;
    let texture = opened.map_err(io::Error::other)?;

    assert!(
        !texture.has_alpha_tail(),
        "presence could not be established"
    );
    assert!(
        matches!(texture.alpha_tail(), Some(Err(crate::ConversionError::Read { source, .. }))
        if source.kind() == io::ErrorKind::PermissionDenied)
    );
    let mut colour = Vec::new();
    texture.merge_into(&mut colour).map_err(io::Error::other)?;
    let output = dir.path().join("colour.dds");
    assert!(
        matches!(texture.merge_to_file(&output), Err(crate::ConversionError::Read { source, .. })
        if source.kind() == io::ErrorKind::PermissionDenied)
    );
    assert!(
        !output.exists(),
        "unresolved inputs prevent file publication"
    );
    assert!(
        matches!(texture.decode_mip(0), Err(crate::ConversionError::Read { source, .. })
        if source.kind() == io::ErrorKind::PermissionDenied)
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a non-root Linux process with directory read permission enforcement"]
fn native_unresolved_alpha_prevents_overwriting_its_other_case_input() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use p4kx_testkit::{SplitDds, TestTextureFormat};

    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("restricted");
    fs::create_dir(&folder)?;
    let header = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .alpha(TestTextureFormat::R8)
        .write_files(&folder, "glass.dds")?;
    let alpha = folder.join("GLASS.DDS.A");
    fs::rename(folder.join("glass.dds.a"), &alpha)?;
    let original_alpha = fs::read(&alpha)?;
    let original_colour = fs::read(&header)?;
    let original_permissions = fs::metadata(&folder)?.permissions();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o311))?;
    let attempted = (|| {
        let probe = folder.join("write-probe");
        fs::write(&probe, b"writable")?;
        fs::remove_file(probe)?;
        let texture = Texture::from_file(&header);
        let opened = texture.is_ok();
        Ok::<_, io::Error>((
            opened,
            texture.and_then(|texture| texture.merge_to_file(&alpha)),
        ))
    })();
    fs::set_permissions(&folder, original_permissions)?;

    let (opened, attempted) = attempted?;
    assert!(opened, "the explicit colour header stays available");
    let failure = attempted.expect_err("unresolved alpha input must prevent the file writer");
    assert!(
        matches!(failure, crate::ConversionError::Read { source, .. }
        if source.kind() == io::ErrorKind::PermissionDenied),
        "the original lookup failure remains typed",
    );
    assert_eq!(fs::read(&alpha)?, original_alpha);
    assert_eq!(fs::read(&header)?, original_colour);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a non-root Linux process with file read permission enforcement"]
fn native_unreadable_present_alpha_prevents_overwriting_its_part() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use p4kx_testkit::{SplitDds, TestTextureFormat};

    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("inputs");
    fs::create_dir(&folder)?;
    let header = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
        .split(1)
        .alpha(TestTextureFormat::R8)
        .write_files(&folder, "glass.dds")?;
    let alpha = folder.join("glass.dds.a");
    let alpha_part = folder.join("GLASS.DDS.1A");
    fs::rename(folder.join("glass.dds.1a"), &alpha_part)?;
    let inputs = [
        header.clone(),
        folder.join("glass.dds.1"),
        alpha.clone(),
        alpha_part.clone(),
    ];
    let mut original_bytes = Vec::new();
    for input in &inputs {
        original_bytes.push(fs::read(input)?);
    }
    Texture::from_file(&header)
        .and_then(|texture| texture.decode_mip(0))
        .map_err(io::Error::other)?;
    let original_permissions = fs::metadata(&alpha)?.permissions();
    fs::set_permissions(&alpha, fs::Permissions::from_mode(0o000))?;
    let attempted = (|| {
        let probe = folder.join("write-probe");
        fs::write(&probe, b"writable")?;
        fs::remove_file(probe)?;
        let original_failure = fs::File::open(&alpha).err();
        let texture = Texture::from_file(&header).map_err(io::Error::other)?;
        let present = texture.has_alpha_tail();
        let mut colour = Vec::new();
        let streamed = texture.merge_into(&mut colour);
        let publication = texture.merge_to_file(&alpha_part);
        Ok::<_, io::Error>((original_failure, present, colour, streamed, publication))
    })();
    fs::set_permissions(&alpha, original_permissions)?;

    let (original_failure, present, colour, streamed, publication) = attempted?;
    let expected = original_failure.expect("native alpha-header permission denial");
    assert_eq!(expected.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        present,
        "metadata establishes alpha presence without opening it"
    );
    streamed.map_err(io::Error::other)?;
    assert!(!colour.is_empty(), "colour streaming remains usable");
    let failure = publication.expect_err("unreadable alpha header must prevent the file writer");
    let crate::ConversionError::Read { source, .. } = failure else {
        panic!("the original alpha-header open failure remains typed: {failure:?}");
    };
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(source.raw_os_error(), expected.raw_os_error());
    for (input, original) in inputs.iter().zip(&original_bytes) {
        assert_eq!(&fs::read(input)?, original, "{}", input.display());
    }
    Ok(())
}
