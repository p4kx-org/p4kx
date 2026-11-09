// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use p4kx_channels::{DiscoveryPathKind, PrefixFileSystem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Probe {
    Kind(PathBuf),
    Directory(PathBuf),
    Config(PathBuf),
    Canonical(PathBuf),
}

#[derive(Clone)]
pub(super) enum Entry {
    Name(OsString),
    Failure(io::ErrorKind),
}

#[derive(Clone, Default)]
pub(super) struct FakeFs {
    kinds: BTreeMap<PathBuf, DiscoveryPathKind>,
    directories: BTreeMap<PathBuf, Vec<Entry>>,
    configs: BTreeMap<PathBuf, Vec<u8>>,
    identities: BTreeMap<PathBuf, PathBuf>,
    failures: BTreeMap<PathBuf, io::ErrorKind>,
    config_failures: BTreeMap<PathBuf, io::ErrorKind>,
    pub(super) probes: RefCell<Vec<Probe>>,
}

impl FakeFs {
    pub(super) fn directory(&mut self, path: &Path) {
        if self.kinds.contains_key(path) {
            return;
        }
        if let Some(parent) = path.parent() {
            self.directory(parent);
            if let Some(name) = path.file_name() {
                self.directories
                    .entry(parent.to_path_buf())
                    .or_default()
                    .push(Entry::Name(name.to_owned()));
            }
        }
        self.kinds
            .insert(path.to_path_buf(), DiscoveryPathKind::Directory);
        self.directories.entry(path.to_path_buf()).or_default();
    }

    pub(super) fn config(&mut self, path: &Path, bytes: Vec<u8>) {
        if let Some(parent) = path.parent() {
            self.directory(parent);
        }
        self.kinds
            .insert(path.to_path_buf(), DiscoveryPathKind::File);
        self.configs.insert(path.to_path_buf(), bytes);
    }

    pub(super) fn alias(&mut self, path: &Path, actual: &Path) {
        self.identities
            .insert(path.to_path_buf(), actual.to_path_buf());
    }

    pub(super) fn fail(&mut self, path: &Path, kind: io::ErrorKind) {
        self.failures.insert(path.to_path_buf(), kind);
    }

    pub(super) fn fail_config(&mut self, path: &Path, kind: io::ErrorKind) {
        self.config_failures.insert(path.to_path_buf(), kind);
    }

    pub(super) fn entries(&mut self, path: &Path, entries: Vec<Entry>) {
        self.directory(path);
        self.directories.insert(path.to_path_buf(), entries);
    }

    fn failure(&self, path: &Path) -> io::Result<()> {
        self.failures.get(path).map_or(Ok(()), |kind| {
            Err(io::Error::new(*kind, "synthetic discovery I/O failure"))
        })
    }
}

impl PrefixFileSystem for FakeFs {
    type Entries = std::vec::IntoIter<io::Result<OsString>>;
    type Config = Cursor<Vec<u8>>;

    fn kind(&self, path: &Path) -> io::Result<DiscoveryPathKind> {
        self.probes
            .borrow_mut()
            .push(Probe::Kind(path.to_path_buf()));
        self.failure(path)?;
        self.kinds
            .get(path)
            .copied()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        self.probes
            .borrow_mut()
            .push(Probe::Directory(path.to_path_buf()));
        self.failure(path)?;
        let entries = self
            .directories
            .get(path)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok(entries
            .iter()
            .map(|entry| match entry {
                Entry::Name(name) => Ok(name.clone()),
                Entry::Failure(kind) => {
                    Err(io::Error::new(*kind, "synthetic directory entry failure"))
                }
            })
            .collect::<Vec<_>>()
            .into_iter())
    }

    fn open_config(&self, path: &Path) -> io::Result<Self::Config> {
        self.probes
            .borrow_mut()
            .push(Probe::Config(path.to_path_buf()));
        self.failure(path)?;
        if let Some(kind) = self.config_failures.get(path) {
            return Err(io::Error::new(
                *kind,
                "synthetic confirmed configuration open failure",
            ));
        }
        self.configs
            .get(path)
            .cloned()
            .map(Cursor::new)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.probes
            .borrow_mut()
            .push(Probe::Canonical(path.to_path_buf()));
        self.failure(path)?;
        if !self.kinds.contains_key(path) {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(self
            .identities
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_path_buf()))
    }
}

pub(super) fn root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"Z:\synthetic-prefix-discovery")
    } else {
        PathBuf::from("/synthetic-prefix-discovery")
    }
}
