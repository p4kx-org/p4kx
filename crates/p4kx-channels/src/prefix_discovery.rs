// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use p4kx_settings::WineLocations;

use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::{DiscoveryLimit, DiscoveryLimits};
use crate::discovery_report::DiscoveryReport;
use crate::prefix_case;
use crate::prefix_fs::{DiscoveryPathKind, PrefixFileSystem};
use crate::prefix_seed_walk::{self, SeedKind};
use crate::steam_config;
use crate::wine::WineEnvironment;

pub(crate) fn discover<F: PrefixFileSystem>(
    environment: &WineEnvironment,
    fs: &F,
    limits: DiscoveryLimits,
) -> DiscoveryReport<Vec<PathBuf>> {
    let mut context = DiscoveryContext::new(fs, limits);
    let prefixes = discover_in(environment, None, &mut context);
    DiscoveryReport::new(prefixes, context.issues)
}

pub(crate) fn discover_in<F: PrefixFileSystem>(
    environment: &WineEnvironment,
    saved: Option<&WineLocations>,
    context: &mut DiscoveryContext<'_, F>,
) -> Vec<PathBuf> {
    let mut found = Prefixes::default();
    let mut libraries = Libraries::default();
    let mut seen = HashSet::new();
    prefix_seed_walk::walk(environment, saved, |kind, path| {
        if context.budget.exhausted() {
            return false;
        }
        if !context.admit(path, DiscoveryLimit::Roots, 1) {
            return false;
        }
        if !seen.insert((kind, path.to_path_buf())) {
            return true;
        }
        match kind {
            SeedKind::Direct => found.admit(context, path),
            SeedKind::Steam => libraries.steam_root(context, path, &mut found),
            SeedKind::Library => libraries.visit(context, path, &mut found),
            SeedKind::Bottles | SeedKind::CrossOver | SeedKind::Games => {
                children(context, path, kind, &mut found);
            }
        }
        !context.budget.exhausted()
    });
    found.paths
}

#[derive(Default)]
struct Prefixes {
    paths: Vec<PathBuf>,
    identities: HashSet<PathBuf>,
}

impl Prefixes {
    fn admit<F: PrefixFileSystem>(&mut self, context: &mut DiscoveryContext<'_, F>, path: &Path) {
        if context.budget.exhausted() {
            return;
        }
        let Some(path) = prefix_case::absolute(context, path) else {
            return;
        };
        let Some(identity) = context.canonical(&path) else {
            return;
        };
        if self.identities.contains(&identity) {
            return;
        }
        if !context.admit(&path, DiscoveryLimit::Prefixes, 1) {
            return;
        }
        self.identities.insert(identity);
        self.paths.push(path);
    }
}

fn children<F: PrefixFileSystem>(
    context: &mut DiscoveryContext<'_, F>,
    root: &Path,
    kind: SeedKind,
    found: &mut Prefixes,
) {
    let Some(root) = prefix_case::absolute(context, root) else {
        return;
    };
    let Some(names) = context.entries(&root) else {
        return;
    };
    for name in names {
        if context.budget.exhausted() {
            return;
        }
        let prefix = root.join(name);
        if kind == SeedKind::CrossOver
            && (context.kind(&prefix) != Some(DiscoveryPathKind::Directory)
                || prefix_case::descend(context, &prefix, &["drive_c"]).is_none())
        {
            continue;
        }
        found.admit(context, &prefix);
    }
}

#[derive(Default)]
struct Libraries {
    roots: HashSet<PathBuf>,
    configs: HashSet<PathBuf>,
}

impl Libraries {
    fn steam_root<F: PrefixFileSystem>(
        &mut self,
        context: &mut DiscoveryContext<'_, F>,
        root: &Path,
        found: &mut Prefixes,
    ) {
        let Some(root) = prefix_case::absolute(context, root) else {
            return;
        };
        self.visit(context, &root, found);
        for folder in ["steamapps", "config"] {
            if context.budget.exhausted() {
                return;
            }
            let Some(folder) = prefix_case::descend(context, &root, &[folder]) else {
                continue;
            };
            let config = folder.join("libraryfolders.vdf");
            let Some(identity) = context.canonical(&config) else {
                continue;
            };
            if !self.configs.insert(identity) {
                continue;
            }
            let Some(libraries) = steam_config::read(context, &config) else {
                continue;
            };
            for library in libraries {
                self.visit(context, &library, found);
            }
        }
    }

    fn visit<F: PrefixFileSystem>(
        &mut self,
        context: &mut DiscoveryContext<'_, F>,
        root: &Path,
        found: &mut Prefixes,
    ) {
        if context.budget.exhausted() {
            return;
        }
        let Some(root) = prefix_case::absolute(context, root) else {
            return;
        };
        let Some(identity) = context.canonical(&root) else {
            return;
        };
        if !self.roots.insert(identity) {
            return;
        }
        let Some(compat) = prefix_case::descend(context, &root, &["steamapps", "compatdata"])
        else {
            return;
        };
        let Some(apps) = context.entries(&compat) else {
            return;
        };
        for app in apps {
            if context.budget.exhausted() {
                return;
            }
            if !app.as_encoded_bytes().iter().all(u8::is_ascii_digit) {
                continue;
            }
            let Some(app) = prefix_case::component(context, &compat, &app) else {
                continue;
            };
            let Some(prefix) = prefix_case::descend(context, &app, &["pfx"]) else {
                continue;
            };
            found.admit(context, &prefix);
        }
    }
}
