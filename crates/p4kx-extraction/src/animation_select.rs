// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_archive::EntryOrigin;
use rayon::prelude::*;

use crate::animation_load::{Described, Kind, Loaded, kind};
use crate::animation_set::{ADDITIVE_REASON, Source, names, normalised, stem};
use crate::{AnimationChoice, ChunkSource, ConversionError, UnreadAnimation};

/// What a read of animation sources found, before it takes the shape of a front-end's type.
pub(crate) struct Gathered<T> {
    pub(crate) animations: Vec<T>,
    pub(crate) origins: Vec<String>,
    pub(crate) archive_sources: Vec<Option<EntryOrigin>>,
    pub(crate) unread: Vec<UnreadAnimation>,
    pub(crate) files: usize,
}

impl<T> Default for Gathered<T> {
    fn default() -> Self {
        Self {
            animations: Vec::new(),
            origins: Vec::new(),
            archive_sources: Vec::new(),
            unread: Vec::new(),
            files: 0,
        }
    }
}

/// What the choice asks for: every animation, or the ones the specs name.
pub(crate) struct Wanted<'a> {
    specs: Vec<&'a str>,
    by_name: bool,
}

impl<'a> Wanted<'a> {
    fn of(choice: &'a AnimationChoice) -> Self {
        match choice {
            AnimationChoice::Named(specs) => Self {
                specs: specs.iter().map(String::as_str).collect(),
                by_name: true,
            },
            AnimationChoice::All => Self {
                specs: Vec::new(),
                by_name: false,
            },
        }
    }

    fn hits(&self, name: &str, path: &str) -> Vec<&'a str> {
        self.specs
            .iter()
            .copied()
            .filter(|spec| names(spec, name, path))
            .collect()
    }

    /// Returns whether a source is worth reading: databases always, since their animations
    /// show their names only inside, and `.caf` files by their stem.
    fn reads(&self, source: &Source, kind: Kind) -> bool {
        !self.by_name
            || source.explicit
            || kind == Kind::Dba
            || !self.hits(stem(&source.label), &source.label).is_empty()
    }
}

/// Sorts and merges `sources`, reads those the choice wants and collects the animations.
pub(crate) fn read<T: Described + Send>(
    mut sources: Vec<Source>,
    choice: &AnimationChoice,
    open: impl Fn(&str) -> Result<ChunkSource, ConversionError> + Sync,
    load: impl Fn(&ChunkSource, &str, Kind, bool) -> Result<Loaded<T>, ConversionError> + Sync,
) -> Result<Gathered<T>, ConversionError> {
    merge_sources(&mut sources);
    let wanted = Wanted::of(choice);
    let results: Vec<_> = sources
        .par_iter()
        .filter_map(|source| {
            let kind = kind(&source.label)?;
            wanted.reads(source, kind).then(|| {
                let loaded = open(&source.label).and_then(|file| {
                    load(&file, &source.label, kind, source.explicit)
                        .map(|loaded| (file.archive_origin().cloned(), loaded))
                });
                (source, loaded)
            })
        })
        .collect();
    let mut set = Gathered::default();
    let mut matched: HashSet<&str> = HashSet::new();
    for (source, result) in results {
        let (origin, loaded) = match result {
            Ok(loaded) => loaded,
            Err(error) if is_fatal(source, &error) => return Err(error),
            Err(error) => {
                set.unread.push(UnreadAnimation {
                    source: source.label.clone(),
                    name: None,
                    reason: describe(&error),
                });
                continue;
            }
        };
        set.files += 1;
        set.unread.extend(loaded.unread);
        keep(
            &mut set,
            &mut matched,
            &wanted,
            source,
            origin.as_ref(),
            loaded.animations,
        );
    }
    let missing: Vec<String> = wanted
        .specs
        .iter()
        .filter(|spec| {
            !matched.contains(**spec)
                && !sources
                    .iter()
                    .any(|source| source.whole && names(spec, "", &source.label))
        })
        .map(|spec| (*spec).to_owned())
        .collect();
    if missing.is_empty() {
        Ok(set)
    } else {
        Err(ConversionError::UnknownAnimations { names: missing })
    }
}

fn merge_sources(sources: &mut Vec<Source>) {
    sources.sort_by_cached_key(|source| (normalised(&source.label), !source.whole));
    sources.dedup_by(|later, earlier| {
        let same = normalised(&later.label) == normalised(&earlier.label);
        earlier.explicit |= same && later.explicit;
        earlier.whole |= same && later.whole;
        same
    });
}

/// Adds the animations of `source` that the choice wants to `set`, and lists the additive ones
/// nothing names as unread.
fn keep<'a, T: Described>(
    set: &mut Gathered<T>,
    matched: &mut HashSet<&'a str>,
    wanted: &Wanted<'a>,
    source: &Source,
    origin: Option<&EntryOrigin>,
    animations: Vec<T>,
) {
    let caf_by_caller = source.explicit && kind(&source.label) == Some(Kind::Caf);
    for animation in animations {
        let hits = wanted.hits(animation.name(), animation.path());
        if animation.additive() && hits.is_empty() && !caf_by_caller {
            if wanted.by_name && !source.whole {
                continue;
            }
            set.unread.push(UnreadAnimation {
                source: source.label.clone(),
                name: Some(animation.name().to_owned()),
                reason: ADDITIVE_REASON.to_owned(),
            });
        } else if source.whole || !wanted.by_name || !hits.is_empty() {
            matched.extend(hits);
            set.origins.push(source.label.clone());
            set.archive_sources.push(origin.cloned());
            set.animations.push(animation);
        }
    }
}

/// Returns whether a failure to read `source` fails the whole read: a file the caller named, or
/// one that does not exist.
const fn is_fatal(source: &Source, error: &ConversionError) -> bool {
    source.explicit
        || matches!(
            error,
            ConversionError::UnknownEntry(_)
                | ConversionError::NoSuchFile { .. }
                | ConversionError::Cancelled
        )
}

fn describe(error: &ConversionError) -> String {
    let mut text = error.to_string();
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}
