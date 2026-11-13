// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;

use serde::Deserialize;

use p4kx_guard::is_redirected_metadata;

use crate::TextureFormat;
use crate::manifest::{FORMAT, MANIFEST_FILE, MESHES_FOLDER, TEXTURES_FOLDER};
use crate::package_receipt::RECEIPT_FILE;
use crate::{ManifestSocScene, PackageError, PackageResult, package_soc_scene};

const MAX_MARKER_LEN: u64 = 1 << 30;
const MIN_ID_DIGITS: usize = 4;

#[derive(Deserialize)]
struct Marker {
    format: String,
    authored_soc: Option<ManifestSocScene>,
    collision_helpers: Option<crate::CollisionHelperReference>,
    #[serde(default)]
    textures: Vec<TextureMarker>,
    #[serde(default)]
    meshes: Vec<MeshMarker>,
}

#[derive(Deserialize)]
struct MeshMarker {
    file: Option<String>,
    vertex_map: Option<String>,
}

#[derive(Deserialize)]
struct TextureMarker {
    file: String,
    format: Option<TextureFormat>,
}

pub(crate) fn check_shape(out: &Path) -> PackageResult<()> {
    if out.file_name().is_none() {
        return Err(PackageError::NoFolderName {
            path: out.to_path_buf(),
        });
    }
    check_replaceable(out)
}

fn check_replaceable(out: &Path) -> PackageResult<()> {
    let not_a_package = || PackageError::NotAPackage {
        path: out.to_path_buf(),
    };
    let io_error = |path: &Path, source| PackageError::write(path, source);
    let metadata = match fs::symlink_metadata(out) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(io_error(out, source)),
    };
    if !metadata.is_dir() {
        return Err(not_a_package());
    }
    let names = names_in(out).map_err(|source| io_error(out, source))?;
    if names.is_empty() {
        return Ok(());
    }
    let marker = read_marker(&out.join(MANIFEST_FILE)).ok_or_else(not_a_package)?;
    let known = [
        MANIFEST_FILE,
        RECEIPT_FILE,
        MESHES_FOLDER,
        TEXTURES_FOLDER,
        package_soc_scene::DIRECTORY,
        crate::collision_helper_document::FILE,
    ];
    if !names
        .iter()
        .all(|name| known.iter().any(|known| name == known))
    {
        return Err(not_a_package());
    }
    if names.iter().any(|name| name == RECEIPT_FILE) && !is_plain_file(&out.join(RECEIPT_FILE)) {
        return Err(not_a_package());
    }
    if !matches_asset_folders(out, &marker)? {
        return Err(not_a_package());
    }
    let collision = crate::collision_helper_document::FILE;
    let exists = names.iter().any(|name| name == collision);
    let valid_collision = marker
        .collision_helpers
        .as_ref()
        .map_or(!exists, |reference| {
            reference.is_valid() && exists && is_plain_file(&out.join(collision))
        });
    if !valid_collision {
        return Err(not_a_package());
    }
    let authored = out.join(package_soc_scene::DIRECTORY);
    if !matches_soc_companion(out, &authored, marker.authored_soc.as_ref())? {
        return Err(not_a_package());
    }
    Ok(())
}

fn matches_asset_folders(out: &Path, marker: &Marker) -> PackageResult<bool> {
    let mesh_prefix = format!("{MESHES_FOLDER}/");
    let Some(vertex_maps) = vertex_map_names(marker, &mesh_prefix) else {
        return Ok(false);
    };
    let texture_prefix = format!("{TEXTURES_FOLDER}/");
    let dds_names: BTreeSet<_> = marker
        .textures
        .iter()
        .filter(|texture| texture.format == Some(TextureFormat::Dds))
        .filter_map(|texture| texture.file.strip_prefix(&texture_prefix))
        .collect();
    for (folder, prefix, extension) in [
        (MESHES_FOLDER, 'm', "glb"),
        (TEXTURES_FOLDER, 't', TextureFormat::Png.extension()),
    ] {
        let path = out.join(folder);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if folder == MESHES_FOLDER && !vertex_maps.is_empty() {
                    return Ok(false);
                }
                continue;
            }
            Err(source) => return Err(PackageError::write(&path, source)),
        }
        let files = names_in(&path).map_err(|source| PackageError::write(&path, source))?;
        if !files.iter().all(|name| {
            let declared_dds = folder == TEXTURES_FOLDER
                && is_id_file(name, prefix, TextureFormat::Dds.extension())
                && name.to_str().is_some_and(|text| dds_names.contains(text));
            let vertex_map = folder == MESHES_FOLDER
                && name.to_str().is_some_and(|text| vertex_maps.contains(text));
            (is_id_file(name, prefix, extension) || declared_dds || vertex_map)
                && is_plain_file(&path.join(name))
        }) {
            return Ok(false);
        }
        if folder == MESHES_FOLDER
            && vertex_maps
                .iter()
                .any(|name| !is_plain_file(&path.join(name)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn vertex_map_names<'a>(marker: &'a Marker, prefix: &str) -> Option<BTreeSet<&'a str>> {
    let mut names = BTreeSet::new();
    for mesh in &marker.meshes {
        if let Some(reference) = &mesh.vertex_map {
            let file = mesh.file.as_deref()?.strip_prefix(prefix)?;
            let map = reference.strip_prefix(prefix)?;
            let stem = file.strip_suffix(".glb")?;
            if !is_id_file(OsStr::new(file), 'm', "glb")
                || map.strip_suffix(".vertex-map.jsonl") != Some(stem)
                || !names.insert(map)
            {
                return None;
            }
        }
    }
    Some(names)
}

fn names_in(folder: &Path) -> io::Result<Vec<OsString>> {
    fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect()
}

fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && !is_redirected_metadata(&metadata))
}

fn is_id_file(name: &OsStr, prefix: char, extension: &str) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(stem) = name
        .strip_suffix(extension)
        .and_then(|rest| rest.strip_suffix('.'))
        .and_then(|rest| rest.strip_prefix(prefix))
    else {
        return false;
    };
    stem.len() >= MIN_ID_DIGITS && stem.bytes().all(|byte| byte.is_ascii_digit())
}

fn matches_soc_companion(
    out: &Path,
    directory: &Path,
    reference: Option<&ManifestSocScene>,
) -> PackageResult<bool> {
    let expected = reference.is_some_and(package_soc_scene::has_expected_reference);
    match fs::symlink_metadata(directory) {
        Ok(metadata) if expected && metadata.is_dir() => {
            let names =
                names_in(directory).map_err(|source| PackageError::write(directory, source))?;
            Ok(names.len() == 1 && is_plain_file(&out.join(package_soc_scene::SCENE_FILE)))
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(reference.is_none()),
        Err(source) => Err(PackageError::write(directory, source)),
    }
}

fn read_marker(path: &Path) -> Option<Marker> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file.take(MAX_MARKER_LEN));
    serde_json::from_reader::<_, Marker>(reader)
        .ok()
        .filter(|marker| marker.format == FORMAT)
}
