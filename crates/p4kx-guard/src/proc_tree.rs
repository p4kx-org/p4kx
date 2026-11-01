// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::process::is_game_process;

/// Bytes of `comm` read per process; the kernel keeps 15 plus a newline.
const COMM_LIMIT: u64 = 64;

/// Bytes of `cmdline` read per process; only the first argument is needed.
const CMDLINE_LIMIT: u64 = 4096;

/// Returns whether a process below the `/proc`-style folder `root` is the Star Citizen game.
///
/// Wine and Proton name the game process `StarCitizen.exe` in `comm`, which the 15-byte limit of
/// the kernel keeps whole, and rewrite the first argument of `cmdline` to the Windows image path.
/// A process counts when either names the game: `comm` as a whole, or the last component of the
/// first `cmdline` argument split at `/` and `\`. Only the two text files of each numeric
/// process folder are read, up to a fixed size; a process that ends during the scan is skipped.
///
/// `root` must be a `/proc`-style tree: only numeric entries that are real folders (not symbolic
/// links) are entered, and only regular files are opened, so a link or a FIFO in a fake tree
/// cannot redirect or block the scan.
///
/// # Errors
///
/// Returns the error of listing `root`: the process list is unknown then.
pub fn game_in_proc_tree(root: &Path) -> io::Result<bool> {
    let mut buffer = Vec::new();
    let mut path = PathBuf::new();
    for entry in fs::read_dir(root)? {
        let Ok(entry) = entry else { continue };
        if !is_process_id(&entry.file_name()) || !entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            continue;
        }
        path.clear();
        path.push(entry.path());
        path.push("comm");
        if read_prefix(&path, COMM_LIMIT, &mut buffer)
            && is_game_process(&String::from_utf8_lossy(buffer.trim_ascii_end()))
        {
            return Ok(true);
        }
        path.pop();
        path.push("cmdline");
        if read_prefix(&path, CMDLINE_LIMIT, &mut buffer)
            && image_name(&buffer).is_some_and(|image| is_game_process(&image))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_process_id(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Replaces the contents of `buffer` with at most `limit` bytes of the regular file at `path`.
///
/// Returns false when the file is not a regular file or cannot be read, as when the process is
/// gone or not visible; the caller skips that process.
fn read_prefix(path: &Path, limit: u64, buffer: &mut Vec<u8>) -> bool {
    buffer.clear();
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
        && File::open(path).is_ok_and(|file| file.take(limit).read_to_end(buffer).is_ok())
}

/// The file name in the first argument of a `cmdline`, split at `/` and `\`.
fn image_name(cmdline: &[u8]) -> Option<Cow<'_, str>> {
    let first = cmdline.split(|&byte| byte == 0).next()?;
    let name = first.rsplit(|&byte| matches!(byte, b'/' | b'\\')).next()?;
    Some(String::from_utf8_lossy(name))
}
