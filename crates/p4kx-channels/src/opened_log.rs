// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use same_file::Handle;

use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::DiscoveryLimit;
use crate::discovery_report::DiscoveryIssueKind;
use crate::launcher_stamp::LogFileStamp;
use crate::prefix_fs::PrefixFileSystem;

#[derive(Debug)]
pub(crate) struct OpenedLog {
    pub(crate) source: usize,
    pub(crate) name: &'static str,
    pub(crate) identity: Arc<Handle>,
    pub(crate) length: u64,
    pub(crate) modified: SystemTime,
    reader: Mutex<()>,
}

impl OpenedLog {
    pub(crate) fn with_file<T>(&self, read: impl FnOnce(&File) -> io::Result<T>) -> io::Result<T> {
        let _held = self
            .reader
            .lock()
            .map_err(|error: PoisonError<_>| io::Error::other(error.to_string()))?;
        read(self.identity.as_file())
    }
}

pub(crate) fn open<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    path: &Path,
    logs: &mut Vec<OpenedLog>,
    identities: &mut HashMap<Arc<Handle>, usize>,
    source: usize,
    name: &'static str,
) -> (LogFileStamp, Option<usize>) {
    if !context.admit(path, DiscoveryLimit::Operations, 1) {
        return (LogFileStamp::Unavailable, None);
    }
    let file = match regular_file(path) {
        Ok(RegularFile::File(file)) => file,
        Ok(RegularFile::Missing) => return (LogFileStamp::Missing, None),
        Ok(RegularFile::NonRegular) => return (LogFileStamp::NonRegular, None),
        Err(error) => {
            context.io(path, error);
            return (LogFileStamp::Unavailable, None);
        }
    };
    let opened = match described(file, source, name) {
        Ok(opened) => opened,
        Err(error) => {
            context.io(path, error);
            return (LogFileStamp::Unavailable, None);
        }
    };
    let stamp = LogFileStamp::Regular {
        identity: Arc::clone(&opened.identity),
        length: opened.length,
        modified: opened.modified,
    };
    if let Some(&index) = identities.get(&opened.identity) {
        if let Some(known) = logs.get(index)
            && (known.length != opened.length || known.modified != opened.modified)
        {
            context.issue(path, DiscoveryIssueKind::Changed);
        }
        return (stamp, Some(index));
    }
    let index = logs.len();
    identities.insert(Arc::clone(&opened.identity), index);
    logs.push(opened);
    (stamp, Some(index))
}

enum RegularFile {
    Missing,
    NonRegular,
    File(File),
}

fn regular_file(path: &Path) -> io::Result<RegularFile> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(RegularFile::Missing),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Ok(RegularFile::NonRegular);
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Ok(RegularFile::NonRegular);
    }
    Ok(RegularFile::File(file))
}

fn described(file: File, source: usize, name: &'static str) -> io::Result<OpenedLog> {
    let metadata = file.metadata()?;
    let modified = metadata.modified()?;
    Ok(OpenedLog {
        source,
        name,
        identity: Arc::new(Handle::from_file(file)?),
        length: metadata.len(),
        modified,
        reader: Mutex::new(()),
    })
}
