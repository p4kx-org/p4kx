// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::process::is_game_process;

/// Bytes of comm read per process; the kernel keeps 15 plus a newline.
const COMM_LIMIT: u64 = 64;

/// Bytes of cmdline read per process; only the first argument is needed.
const CMDLINE_LIMIT: u64 = 4096;

struct ProcessFile {
    name: &'static str,
    limit: u64,
    names_game: fn(&[u8]) -> bool,
}

const PROCESS_FILES: [ProcessFile; 2] = [
    ProcessFile {
        name: "comm",
        limit: COMM_LIMIT,
        names_game: comm_names_game,
    },
    ProcessFile {
        name: "cmdline",
        limit: CMDLINE_LIMIT,
        names_game: cmdline_names_game,
    },
];

/// Returns whether a process below a proc-style root identifies the game.
///
/// Only bounded comm/cmdline text in real numeric directories is read; links and non-files are
/// skipped, and a process disappearing during the scan does not make the scan incomplete.
///
/// # Errors
///
/// Returns an enumeration or read error when no game is found and part of the scan is unreadable.
pub fn game_in_proc_tree(root: &Path) -> io::Result<bool> {
    let folders = fs::read_dir(root)?.filter_map(process_folder);
    scan(folders, read_prefix)
}

fn process_folder(entry: io::Result<DirEntry>) -> Option<io::Result<PathBuf>> {
    let entry = match entry {
        Ok(entry) => entry,
        Err(error) => return Some(Err(error)),
    };
    if !is_process_id(&entry.file_name()) {
        return None;
    }
    match entry.file_type() {
        Ok(kind) if kind.is_dir() => Some(Ok(entry.path())),
        Ok(_) => None,
        Err(error) => Some(Err(error)),
    }
}

fn scan(
    folders: impl IntoIterator<Item = io::Result<PathBuf>>,
    mut read: impl FnMut(&Path, u64, &mut Vec<u8>) -> io::Result<bool>,
) -> io::Result<bool> {
    let mut buffer = Vec::new();
    let mut failure = None;
    for folder in folders {
        let mut path = match folder {
            Ok(path) => path,
            Err(error) => {
                remember_error(&mut failure, error);
                continue;
            }
        };
        for source in &PROCESS_FILES {
            path.push(source.name);
            match read(&path, source.limit, &mut buffer) {
                Ok(true) if (source.names_game)(&buffer) => return Ok(true),
                Err(error) => remember_error(&mut failure, error),
                Ok(_) => {}
            }
            path.pop();
        }
    }
    failure.map_or(Ok(false), Err)
}

fn remember_error(failure: &mut Option<io::Error>, error: io::Error) {
    if error.kind() != io::ErrorKind::NotFound && failure.is_none() {
        *failure = Some(error);
    }
}

fn is_process_id(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
}

fn read_prefix(path: &Path, limit: u64, buffer: &mut Vec<u8>) -> io::Result<bool> {
    buffer.clear();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    match File::open(path) {
        Ok(file) => {
            file.take(limit).read_to_end(buffer)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn comm_names_game(bytes: &[u8]) -> bool {
    is_game_process(&String::from_utf8_lossy(bytes.trim_ascii_end()))
}

fn cmdline_names_game(bytes: &[u8]) -> bool {
    image_name(bytes).is_some_and(|name| is_game_process(&name))
}

fn image_name(cmdline: &[u8]) -> Option<Cow<'_, str>> {
    let first = cmdline.split(|&byte| byte == 0).next()?;
    let name = first.rsplit(|&byte| matches!(byte, b'/' | b'\\')).next()?;
    Some(String::from_utf8_lossy(name))
}

#[cfg(test)]
mod tests;
