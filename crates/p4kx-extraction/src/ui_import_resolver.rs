// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use p4kx_archive::{Archive, Entry};
use p4kx_gfx::{Assets, Export, Font, Import, MAX_MOVIE_LEN, Movie};
use p4kx_localization::Language;

use super::{
    ImportIssue, ImportReport, MAX_IMPORT_BYTES, MAX_IMPORT_DEPTH, MAX_IMPORTED_FONTS,
    MAX_IMPORTED_MOVIES, UnresolvedImport,
};
use crate::sound::read_capped;
use crate::ui_import_match::{candidates, folder_of};
use crate::ui_movie::MAX_UI_MOVIE_LEN;
use crate::ui_service::ui_font_sources::{FontHop, FontRoute, ImportedFontSource, LoadedSources};

/// The fonts taken from other movies, each named by the name the importing movie imports it under.
#[derive(Default)]
pub(crate) struct ImportedFonts<'archive> {
    pub(crate) fonts: Vec<Font>,
    pub(crate) report: ImportReport,
    pub(crate) sources: Vec<Option<ImportedFontSource<'archive>>>,
}

struct ResolvedFont<'archive> {
    font: Font,
    route: Option<FontRoute<'archive>>,
}

struct Loaded<'archive> {
    path: String,
    fonts: Vec<Font>,
    exports: Vec<Export>,
    imports: Vec<Import>,
    sources: Option<Rc<LoadedSources<'archive>>>,
}

enum Slot<'archive> {
    Loaded(Rc<Loaded<'archive>>),
    Failed(ImportIssue),
}

struct Resolver<'a> {
    archive: &'a Archive,
    language: Language,
    slots: HashMap<usize, Slot<'a>>,
    found: HashMap<(String, String), Vec<usize>>,
    bytes: u64,
    out: ImportedFonts<'a>,
    observe: bool,
}

/// Resolves the fonts that texts of `assets`, read from the Entry `importer`, use by an imported
/// ID: the exporting movies are looked up in `archive`, with the localisation folder of
/// `language` as the fallback, and read once each. Every (URL, name) pair is resolved once, so
/// two imports of one name from different movies stay two fonts.
pub(crate) fn resolve_fonts<'archive>(
    archive: &'archive Archive,
    importer: &str,
    assets: &Assets<'_>,
    language: Language,
    observe: bool,
) -> ImportedFonts<'archive> {
    let mut used: Vec<u16> = assets
        .static_texts
        .iter()
        .flat_map(|text| text.records.iter().filter_map(|record| record.font_id))
        .chain(assets.edit_texts.iter().filter_map(|text| text.font_id))
        .collect();
    used.sort_unstable();
    used.dedup();
    let mut resolver = Resolver {
        archive,
        language,
        slots: HashMap::new(),
        found: HashMap::new(),
        bytes: 0,
        out: ImportedFonts::default(),
        observe,
    };
    let folder = folder_of(importer);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (import_index, import) in assets.imports.iter().enumerate() {
        for (item_index, asset) in import
            .assets
            .iter()
            .enumerate()
            .filter(|(_, asset)| used.binary_search(&asset.id).is_ok())
        {
            let key = (
                import.url.to_ascii_lowercase(),
                asset.name.to_ascii_lowercase(),
            );
            if !seen.insert(key) {
                continue;
            }
            match resolver.font(&import.url, &asset.name, folder, 0, &mut Vec::new()) {
                Ok(mut found) => {
                    found.font.export_name = Some(asset.name.clone());
                    found.font.import_url = Some(import.url.clone());
                    resolver.out.fonts.push(found.font);
                    if observe {
                        resolver
                            .out
                            .sources
                            .push(found.route.map(|route| ImportedFontSource {
                                import_index,
                                item_index,
                                route,
                            }));
                    }
                }
                Err(issue) => resolver.out.report.unresolved.push(UnresolvedImport {
                    url: import.url.clone(),
                    name: asset.name.clone(),
                    issue,
                }),
            }
        }
    }
    resolver.out
}

impl<'archive> Resolver<'archive> {
    fn font(
        &mut self,
        url: &str,
        name: &str,
        folder: &str,
        depth: usize,
        visiting: &mut Vec<usize>,
    ) -> Result<ResolvedFont<'archive>, ImportIssue> {
        if depth > MAX_IMPORT_DEPTH {
            return Err(ImportIssue::TooDeep);
        }
        let mut issue = ImportIssue::NotFound;
        for at in self.find(url, folder) {
            match self.font_in(at, name, depth, visiting) {
                Ok(font) => return Ok(font),
                Err(next) => {
                    if issue == ImportIssue::NotFound || next != ImportIssue::NoSuchExport {
                        issue = next;
                    }
                }
            }
        }
        Err(issue)
    }

    fn font_in(
        &mut self,
        at: usize,
        name: &str,
        depth: usize,
        visiting: &mut Vec<usize>,
    ) -> Result<ResolvedFont<'archive>, ImportIssue> {
        if visiting.contains(&at) {
            return Err(ImportIssue::TooDeep);
        }
        let loaded = self.load(at)?;
        let (export_index, export) = loaded
            .exports
            .iter()
            .enumerate()
            .find(|(_, export)| export.name.eq_ignore_ascii_case(name))
            .ok_or(ImportIssue::NoSuchExport)?;
        if let Some((font_index, font)) = loaded
            .fonts
            .iter()
            .enumerate()
            .find(|(_, font)| font.id == export.id)
        {
            if self.out.fonts.len() >= MAX_IMPORTED_FONTS {
                return Err(ImportIssue::OverBudget);
            }
            let route = loaded.sources.as_ref().map(|sources| FontRoute {
                font_index,
                hops: vec![FontHop {
                    movie: Rc::clone(sources),
                    export_index,
                    forwarding: None,
                }],
            });
            return Ok(ResolvedFont {
                font: font.clone(),
                route,
            });
        }
        visiting.push(at);
        let result = self.forward_font(&loaded, (export_index, export), depth, visiting);
        visiting.pop();
        result
    }

    fn forward_font(
        &mut self,
        loaded: &Loaded<'archive>,
        (export_index, export): (usize, &Export),
        depth: usize,
        visiting: &mut Vec<usize>,
    ) -> Result<ResolvedFont<'archive>, ImportIssue> {
        let (import_index, item_index, next_url, next) = loaded
            .imports
            .iter()
            .enumerate()
            .find_map(|(import_index, import)| {
                import
                    .assets
                    .iter()
                    .enumerate()
                    .find(|(_, asset)| asset.id == export.id)
                    .map(|(item_index, asset)| {
                        (
                            import_index,
                            item_index,
                            import.url.as_str(),
                            asset.name.as_str(),
                        )
                    })
            })
            .ok_or(ImportIssue::NoSuchExport)?;
        let result = self.font(next_url, next, folder_of(&loaded.path), depth + 1, visiting);
        let mut resolved = result?;
        if let Some(route) = &mut resolved.route {
            if let Some(movie) = &loaded.sources {
                route.hops.push(FontHop {
                    movie: Rc::clone(movie),
                    export_index,
                    forwarding: Some((import_index, item_index)),
                });
            } else {
                resolved.route = None;
            }
        }
        Ok(resolved)
    }

    fn find(&mut self, url: &str, folder: &str) -> Vec<usize> {
        let key = (url.to_owned(), folder.to_owned());
        if let Some(found) = self.found.get(&key) {
            return found.clone();
        }
        let found: Vec<_> = candidates(self.archive, url, folder, self.language).collect();
        self.found.insert(key, found.clone());
        found
    }

    fn load(&mut self, at: usize) -> Result<Rc<Loaded<'archive>>, ImportIssue> {
        if let Some(slot) = self.slots.get(&at) {
            return match slot {
                Slot::Loaded(loaded) => Ok(Rc::clone(loaded)),
                Slot::Failed(issue) => Err(issue.clone()),
            };
        }
        let result = self.read(at);
        self.slots.insert(
            at,
            match &result {
                Ok(loaded) => Slot::Loaded(Rc::clone(loaded)),
                Err(issue) => Slot::Failed(issue.clone()),
            },
        );
        result
    }

    fn charge(&mut self, amount: u64) -> Option<()> {
        let total = self
            .bytes
            .checked_add(amount)
            .filter(|&total| total <= MAX_IMPORT_BYTES)?;
        self.bytes = total;
        Some(())
    }

    fn read(&mut self, at: usize) -> Result<Rc<Loaded<'archive>>, ImportIssue> {
        let entry: &Entry = self
            .archive
            .entries()
            .get(at)
            .ok_or(ImportIssue::NotFound)?;
        let size = entry.uncompressed_size();
        if self.out.report.movies.len() >= MAX_IMPORTED_MOVIES || self.charge(size).is_none() {
            return Err(ImportIssue::OverBudget);
        }
        let reader = self
            .archive
            .open_entry(entry)
            .map_err(|error| ImportIssue::Unreadable(error.to_string()))?;
        let bytes = read_capped(reader, size, MAX_UI_MOVIE_LEN)
            .map_err(|error| ImportIssue::Unreadable(error.to_string()))?
            .ok_or_else(|| ImportIssue::Unreadable("the movie is too large".to_owned()))?;
        let declared = Movie::declared_len(&bytes)
            .map_err(|error| ImportIssue::Unreadable(error.to_string()))?;
        if u64::from(declared) > MAX_MOVIE_LEN {
            return Err(ImportIssue::Unreadable(
                "the movie declares more than it may decompress to".to_owned(),
            ));
        }
        if self
            .charge(u64::from(declared).saturating_sub(size))
            .is_none()
        {
            return Err(ImportIssue::OverBudget);
        }
        let movie =
            Movie::parse(bytes).map_err(|error| ImportIssue::Unreadable(error.to_string()))?;
        let (mut assets, sources) = if self.observe {
            let observed = movie.assets_observed();
            let source = LoadedSources::new(
                self.archive.observed_entry(entry),
                *movie.read_source(),
                observed.origins,
            );
            (observed.assets, Some(Rc::new(source)))
        } else {
            (movie.assets(), None)
        };
        let path = entry.path().to_owned();
        self.out.report.movies.push(path.clone());
        Ok(Rc::new(Loaded {
            path,
            fonts: std::mem::take(&mut assets.fonts),
            exports: std::mem::take(&mut assets.exports),
            imports: std::mem::take(&mut assets.imports),
            sources,
        }))
    }
}
