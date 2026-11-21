// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::DiscoveryLimit;
use crate::discovery_report::DiscoveryIssueKind;
use crate::prefix_fs::{DiscoveryPathKind, PrefixFileSystem};
use crate::steam_libraries::{self, ConfigError};

const CONFIG_BUFFER_BYTES: usize = 4096;

pub(crate) fn read<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    path: &Path,
) -> Option<Vec<PathBuf>> {
    if context.kind(path) != Some(DiscoveryPathKind::File) {
        return None;
    }
    if !context.admit(path, DiscoveryLimit::Operations, 1)
        || !context.admit(path, DiscoveryLimit::ConfigFiles, 1)
    {
        return None;
    }
    let mut file = match context.fs.open_config(path) {
        Ok(file) => file,
        Err(error) => {
            context.io(path, error);
            return None;
        }
    };
    let bytes = contents(context, path, &mut file)?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        context.issue(path, DiscoveryIssueKind::MalformedConfig);
        return None;
    };
    match steam_libraries::parse(text, &mut context.budget) {
        Ok(roots) => Some(roots),
        Err(ConfigError::Malformed) => {
            context.issue(path, DiscoveryIssueKind::MalformedConfig);
            None
        }
        Err(ConfigError::Limit(limit)) => {
            context.issue(path, DiscoveryIssueKind::Limit(limit));
            None
        }
    }
}

fn contents<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    path: &Path,
    file: &mut impl Read,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; CONFIG_BUFFER_BYTES];
    loop {
        let count = match file.read(&mut buffer) {
            Ok(0) => return Some(bytes),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                context.io(path, error);
                return None;
            }
        };
        if count
            > context
                .budget
                .limits
                .config_file_bytes
                .saturating_sub(bytes.len())
        {
            context.issue(
                path,
                DiscoveryIssueKind::Limit(DiscoveryLimit::ConfigFileBytes),
            );
            return None;
        }
        if !context.admit(path, DiscoveryLimit::ConfigBytes, count) {
            return None;
        }
        let Some(chunk) = buffer.get(..count) else {
            context.io(
                path,
                io::Error::new(io::ErrorKind::InvalidData, "invalid reader byte count"),
            );
            return None;
        };
        bytes.extend_from_slice(chunk);
    }
}
