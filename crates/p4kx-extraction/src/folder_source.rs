// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A folder of files on disk as the source of a Conversion.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use thiserror::Error;

use crate::source::{EntrySource, SourceEntry, SourceReader};
use crate::{CancelToken, EntryFailure, Exclusion, ExtractionError, Filter};

/// The most files, folders and rejected names [`FolderSource::scan`] visits; it bounds the memory
/// of the listing, of the rejected names and of the folders waiting to be read.
pub const MAX_FOLDER_ENTRIES: usize = 1 << 22;
/// The deepest nesting of folders [`FolderSource::scan`] descends into.
pub const MAX_FOLDER_DEPTH: usize = 128;

/// Why a file of an input folder is not read as an Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FolderFileError {
    /// A symbolic link or junction; links are never followed.
    #[error("a link is not followed out of the input folder")]
    Link,
    /// A file name that is not valid UTF-8.
    #[error("the name is not valid UTF-8")]
    NotUtf8,
    /// A file name that holds a path separator.
    #[error("the name contains a path separator")]
    Separator,
    /// Neither a file, a folder nor a link.
    #[error("it is neither a file nor a folder")]
    Special,
    /// The file or folder cannot be read.
    #[error("cannot be read: {0}")]
    Unreadable(io::ErrorKind),
    /// The size of the file differs from the one when the folder was read.
    #[error("the file changed after the folder was read")]
    Changed,
    /// The file resolves to a place outside the input folder when it is opened.
    #[error("the file lies outside the input folder")]
    Outside,
}

/// The files of a folder that a Filter matches and an exclusion leaves in.
///
/// It lists files of the [`FolderSource`] that made it.
#[derive(Debug, Clone)]
pub struct FolderSelection<'a> {
    pub(crate) entries: Vec<SourceEntry<'a>>,
    matched: usize,
    excluded_parts: usize,
}

impl FolderSelection<'_> {
    /// Returns how many files the Filter matched.
    #[must_use]
    pub const fn matched(&self) -> usize {
        self.matched
    }

    /// Returns how many files remain after the exclusion.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no file remains.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns how many part files of textures whose header remains the exclusion left out; a
    /// Conversion still reads them.
    #[must_use]
    pub const fn excluded_parts(&self) -> usize {
        self.excluded_parts
    }
}

/// Why an input folder cannot be converted.
#[derive(Debug, Error)]
pub enum FolderError {
    /// The input folder cannot be read.
    #[error("cannot read {}", path.display())]
    Read {
        /// The folder.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The input path is no folder.
    #[error("{} is not a folder", .0.display())]
    NotAFolder(PathBuf),
    /// The folder holds more than [`MAX_FOLDER_ENTRIES`] files, folders and rejected names.
    #[error("the folder holds more than {limit} files and folders")]
    TooManyEntries {
        /// The limit.
        limit: usize,
    },
    /// The folder nests deeper than [`MAX_FOLDER_DEPTH`] levels.
    #[error("the folder nests deeper than {limit} levels")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// One of the input and output folders lies inside the other, or they are the same.
    #[error("the output folder {} and the input folder {} overlap", output.display(), input.display())]
    Overlap {
        /// The input folder.
        input: PathBuf,
        /// The output folder.
        output: PathBuf,
    },
    /// The output folder lies inside a Star Citizen installation.
    #[error(transparent)]
    Installation(#[from] p4kx_guard::WriteProtectionError),
    /// The run was cancelled while the folder was read.
    #[error("cancelled")]
    Cancelled,
}

/// One file of a folder: its path below the folder with `\`, its size when listed and where it
/// is on disk.
#[derive(Debug)]
struct FolderFile {
    path: String,
    size: u64,
    disk: PathBuf,
}

/// A folder still to be read: where it is, the path of its files up to it, ending in `\`, and how
/// deep it lies.
type Pending = (PathBuf, String, usize);

/// The files below an input folder, listed once, with the paths below it using `\`.
///
/// Links are never followed and so never listed; each is reported as a rejected file.
#[derive(Debug)]
pub struct FolderSource {
    root: PathBuf,
    files: Vec<FolderFile>,
    rejected: Vec<(String, FolderFileError)>,
    visited: usize,
}

impl FolderSource {
    /// Lists the files below the folder `root`, sorted by path, stopping once `cancel` is
    /// cancelled.
    ///
    /// # Errors
    ///
    /// Returns [`FolderError::NotAFolder`] when `root` is no folder, [`FolderError::Read`] when
    /// it cannot be read, [`FolderError::TooManyEntries`] and [`FolderError::TooDeep`] beyond the
    /// limits, and [`FolderError::Cancelled`].
    pub fn scan(root: &Path, cancel: Option<&CancelToken>) -> Result<Self, FolderError> {
        let read = |source| FolderError::Read {
            path: root.to_path_buf(),
            source,
        };
        if !fs::metadata(root).map_err(read)?.is_dir() {
            return Err(FolderError::NotAFolder(root.to_path_buf()));
        }
        let root = fs::canonicalize(root).map_err(read)?;
        let mut source = Self {
            root,
            files: Vec::new(),
            rejected: Vec::new(),
            visited: 0,
        };
        let mut pending = vec![(source.root.clone(), String::new(), 0)];
        while let Some((folder, prefix, depth)) = pending.pop() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(FolderError::Cancelled);
            }
            source.list(&folder, (&prefix, depth), &mut pending)?;
        }
        source.files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        Ok(source)
    }

    /// Returns the folder, canonical.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the number of files listed.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.files.len()
    }

    /// Returns whether no file is listed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Returns the sizes of the listed files added up.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }

    /// Returns the files and folders that were not listed, with the path below the folder and
    /// the reason.
    #[must_use]
    pub fn rejected(&self) -> &[(String, FolderFileError)] {
        &self.rejected
    }

    /// Returns the files whose path matches `filter`, or every file, without those `exclusion`
    /// leaves out, in path order.
    #[must_use]
    pub fn select(&self, filter: Option<&Filter>, exclusion: &Exclusion) -> FolderSelection<'_> {
        let matching: Vec<SourceEntry<'_>> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| filter.is_none_or(|filter| filter.matches(&file.path)))
            .map(|(index, file)| self.entry(index, file))
            .collect();
        let left = exclusion.leave_out(matching, |entry: &SourceEntry<'_>| entry.path());
        FolderSelection {
            matched: left.kept.len().saturating_add(left.removed),
            excluded_parts: left.parts_of_kept_textures.len(),
            entries: left.kept,
        }
    }

    /// Returns the failures of the files and folders that were not listed.
    pub(crate) fn rejected_failures(&self) -> impl Iterator<Item = EntryFailure> {
        self.rejected.iter().map(|(path, reason)| EntryFailure {
            path: path.clone(),
            error: ExtractionError::Folder(*reason),
        })
    }

    /// Returns the file `file` at `index` as an entry of this source.
    fn entry<'s>(&'s self, index: usize, file: &'s FolderFile) -> SourceEntry<'s> {
        SourceEntry::new(self, (&file.path, file.size), index)
    }

    /// Lists the folder `folder`, whose files have the path `prefix` in front and which lies
    /// `depth` levels down: files go to the listing, folders to `pending`, everything else to the
    /// rejected names.
    fn list(
        &mut self,
        folder: &Path,
        (prefix, depth): (&str, usize),
        pending: &mut Vec<Pending>,
    ) -> Result<(), FolderError> {
        let shown = prefix.trim_end_matches('\\');
        let entries = match fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(error) if depth > 0 => {
                self.reject(shown, FolderFileError::Unreadable(error.kind()));
                return Ok(());
            }
            Err(source) => {
                return Err(FolderError::Read {
                    path: folder.to_path_buf(),
                    source,
                });
            }
        };
        for item in entries {
            self.visit()?;
            let item = match item {
                Ok(item) => item,
                Err(error) => {
                    self.reject(shown, FolderFileError::Unreadable(error.kind()));
                    continue;
                }
            };
            let Ok(name) = item.file_name().into_string() else {
                let lossy = item.file_name().to_string_lossy().into_owned();
                self.reject(&format!("{prefix}{lossy}"), FolderFileError::NotUtf8);
                continue;
            };
            let path = format!("{prefix}{name}");
            if name.contains(['\\', '/']) {
                self.reject(&path, FolderFileError::Separator);
                continue;
            }
            let kind = match item.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    self.reject(&path, FolderFileError::Unreadable(error.kind()));
                    continue;
                }
            };
            if kind.is_symlink() {
                self.reject(&path, FolderFileError::Link);
            } else if kind.is_dir() {
                if depth + 1 > MAX_FOLDER_DEPTH {
                    return Err(FolderError::TooDeep {
                        limit: MAX_FOLDER_DEPTH,
                    });
                }
                pending.push((item.path(), format!("{path}\\"), depth + 1));
            } else if kind.is_file() {
                self.add(&item, path);
            } else {
                self.reject(&path, FolderFileError::Special);
            }
        }
        Ok(())
    }

    /// Counts one more visited file, folder or rejected name against [`MAX_FOLDER_ENTRIES`].
    const fn visit(&mut self) -> Result<(), FolderError> {
        self.visited += 1;
        if self.visited > MAX_FOLDER_ENTRIES {
            return Err(FolderError::TooManyEntries {
                limit: MAX_FOLDER_ENTRIES,
            });
        }
        Ok(())
    }

    /// Lists the file `item` as `path`, or rejects it when its size cannot be read.
    fn add(&mut self, item: &fs::DirEntry, path: String) {
        match item.metadata() {
            Ok(metadata) => self.files.push(FolderFile {
                path,
                size: metadata.len(),
                disk: item.path(),
            }),
            Err(error) => self.reject(&path, FolderFileError::Unreadable(error.kind())),
        }
    }

    /// Records that `path` is not listed, for `reason`; the visit that found it is already
    /// counted.
    fn reject(&mut self, path: &str, reason: FolderFileError) {
        self.rejected.push((path.to_owned(), reason));
    }
}

impl EntrySource for FolderSource {
    fn open(&self, index: usize, _verify: bool) -> Result<SourceReader<'_>, ExtractionError> {
        let file = self.files.get(index).ok_or_else(|| {
            ExtractionError::Read(io::Error::new(
                io::ErrorKind::NotFound,
                "the folder has no file at that position",
            ))
        })?;
        let unreadable =
            |error: io::Error| ExtractionError::Folder(FolderFileError::Unreadable(error.kind()));
        if fs::symlink_metadata(&file.disk)
            .map_err(unreadable)?
            .is_symlink()
        {
            return Err(FolderFileError::Link.into());
        }
        let opened = File::open(&file.disk).map_err(unreadable)?;
        let metadata = opened.metadata().map_err(unreadable)?;
        if !metadata.is_file() || metadata.len() != file.size {
            return Err(FolderFileError::Changed.into());
        }
        let resolved = fs::canonicalize(&file.disk).map_err(unreadable)?;
        if !resolved.starts_with(&self.root) {
            return Err(FolderFileError::Outside.into());
        }
        Ok(Box::new(opened.take(file.size)))
    }

    fn checksum(&self, _index: usize) -> Option<p4kx_archive::Checksum> {
        None
    }

    fn raw_path(&self, _index: usize) -> Option<&[u8]> {
        None
    }

    fn path_of(&self, index: usize) -> Option<&str> {
        self.files.get(index).map(|file| file.path.as_str())
    }

    fn members(&self, wanted: &(dyn Fn(&str) -> bool + Sync)) -> Vec<SourceEntry<'_>> {
        self.files
            .par_iter()
            .enumerate()
            .filter(|(_, file)| wanted(&file.path))
            .map(|(index, file)| self.entry(index, file))
            .collect()
    }
}
