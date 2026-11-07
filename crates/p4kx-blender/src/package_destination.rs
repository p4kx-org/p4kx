// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;

use serde::Deserialize;

use crate::manifest::{FORMAT, MANIFEST_FILE, MESHES_FOLDER, TEXTURES_FOLDER};
use crate::{PackageError, PackageResult};

const MAX_MARKER_LEN: u64 = 1 << 30;
const MIN_ID_DIGITS: usize = 4;

#[derive(Deserialize)]
struct Marker {
    format: String,
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
    let known = [MANIFEST_FILE, MESHES_FOLDER, TEXTURES_FOLDER];
    if !names
        .iter()
        .all(|name| known.iter().any(|known| name == known))
        || !is_manifest(&out.join(MANIFEST_FILE))
    {
        return Err(not_a_package());
    }
    for (folder, prefix, extension) in [(MESHES_FOLDER, 'm', "glb"), (TEXTURES_FOLDER, 't', "png")]
    {
        let path = out.join(folder);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(not_a_package()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => return Err(io_error(&path, source)),
        }
        let files = names_in(&path).map_err(|source| io_error(&path, source))?;
        if !files
            .iter()
            .all(|name| is_id_file(name, prefix, extension) && is_plain_file(&path.join(name)))
        {
            return Err(not_a_package());
        }
    }
    Ok(())
}

fn names_in(folder: &Path) -> io::Result<Vec<OsString>> {
    fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect()
}

fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
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

fn is_manifest(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let reader = BufReader::new(file.take(MAX_MARKER_LEN));
    serde_json::from_reader::<_, Marker>(reader).is_ok_and(|marker| marker.format == FORMAT)
}
