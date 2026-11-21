// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Write as _};
use std::path::{Path, PathBuf};

use p4kx_extraction::{ArchiveWatch, Build, DataForge, RecordTree};
use serde::{Deserialize, Serialize};

use super::TestResult;
use super::discovery::{self, Discovery};

const SELECTOR_OUTPUT_BYTES: usize = 4 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Selector {
    pub(super) record_index: u32,
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    discovery: Option<Discovery>,
}

impl Selector {
    fn read(path: &Path) -> TestResult<Self> {
        let file = File::open(path).map_err(|_| "open the private selector file")?;
        serde_json::from_reader(BufReader::new(file)).map_err(|_| {
            "the private selector needs recordIndex, branch, version, changeNumber and buildId"
                .into()
        })
    }

    fn from_build(record_index: u32, build: &Build, discovery: Discovery) -> Self {
        Self {
            record_index,
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            change_number: build.change_number().to_owned(),
            build_id: build.build_id().to_owned(),
            discovery: Some(discovery),
        }
    }

    fn write_new(&self, path: &Path) -> TestResult<Self> {
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|_| "serialize the private source selector")?;
        if bytes.len() > SELECTOR_OUTPUT_BYTES {
            return Err("private source selector exceeds its output contract".into());
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|_| "create a new private selector without replacing an existing file")?;
        file.write_all(&bytes)
            .map_err(|_| "write the private source selector")?;
        file.flush()
            .map_err(|_| "flush the private source selector")?;
        Self::read(path)
    }

    pub(super) fn check(&self, build: &Build) -> TestResult {
        let expected = [
            &self.branch,
            &self.version,
            &self.change_number,
            &self.build_id,
        ];
        let actual = [
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id(),
        ];
        if expected
            .iter()
            .zip(actual)
            .any(|(expected, actual)| expected.is_empty() || expected.as_str() != actual)
            || self.change_number.len() > 20
            || !self.change_number.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "the private selector does not match the complete installed Build identity".into(),
            );
        }
        Ok(())
    }

    pub(super) fn change(&self) -> &str {
        &self.change_number
    }
}

pub(super) enum Request {
    Existing(Selector),
    Discover(PathBuf),
}

impl Request {
    pub(super) fn read() -> TestResult<Self> {
        match (
            std::env::var_os("P4KX_CONTRACT_SDK_SELECTOR_FILE"),
            std::env::var_os("P4KX_CONTRACT_SDK_DISCOVER_SELECTOR_FILE"),
        ) {
            (Some(path), None) => Selector::read(Path::new(&path)).map(Self::Existing),
            (None, Some(path)) => {
                let path = PathBuf::from(path);
                if !path.is_absolute()
                    || !path.parent().is_some_and(Path::is_dir)
                    || path
                        .try_exists()
                        .map_err(|_| "inspect the private discovery destination")?
                {
                    return Err("discovery requires a new absolute private selector path with an existing parent directory".into());
                }
                Ok(Self::Discover(path))
            }
            _ => Err(
                "set exactly one existing-selector or explicit source-discovery selector variable"
                    .into(),
            ),
        }
    }

    pub(super) fn check(&self, build: &Build) -> TestResult {
        match self {
            Self::Existing(selector) => selector.check(build),
            Self::Discover(_) => Ok(()),
        }
    }

    pub(super) fn resolve<'source>(
        self,
        build: &Build,
        forge: &'source DataForge<'source>,
        watch: &ArchiveWatch,
    ) -> TestResult<(Selector, RecordTree<'source>)> {
        match self {
            Self::Existing(selector) => {
                let tree = super::original(forge, selector.record_index)?;
                Ok((selector, tree))
            }
            Self::Discover(path) => {
                let (tree, counts) = discovery::first(forge, watch)?;
                let selector = Selector::from_build(tree.record_index(), build, counts);
                selector.check(build)?;
                super::check(watch)?;
                let reread = selector.write_new(&path)?;
                reread.check(build)?;
                if reread.record_index != tree.record_index() {
                    return Err(
                        "strict selector roundtrip changed the selected original owner".into(),
                    );
                }
                Ok((reread, tree))
            }
        }
    }
}
