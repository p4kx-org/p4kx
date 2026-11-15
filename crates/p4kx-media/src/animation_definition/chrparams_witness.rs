// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write as _};

use p4kx_extraction::cryxml::{
    Format, SourceAttributePosition, SourceDeclarationKind, SourceDocument, SourceElement,
    SourceReadOptions,
};
use p4kx_extraction::{Archive, ArchiveWatch, Build, Entry};
use p4kx_testkit::{real_archive_key, real_archive_path};

#[path = "chrparams_witness_selectors.rs"]
mod selectors;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Counts {
    root: usize,
    nested: usize,
    containers: usize,
    text: usize,
    binary: usize,
    bytes: u64,
    elements: usize,
    attributes: usize,
    namespaces: usize,
    default_pose_sites: usize,
    selectors: selectors::Counts,
}

struct Census {
    stage: &'static str,
    counts: Counts,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_chrparams_source_census_reconciles_ordered_declarations_and_scope_targets()
-> Result<(), String> {
    let mut census = Census {
        stage: "opening Archive and Build",
        counts: Counts::default(),
    };
    census.verify().map_err(|_| {
        format!(
            "chrparams census stopped while {} after {} root and {} nested documents",
            census.stage, census.counts.root, census.counts.nested
        )
    })
}

impl Census {
    fn verify(&mut self) -> TestResult {
        let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
        let build = archive.build().ok_or("authoritative Build manifest")?;
        let directory = archive.path().parent().ok_or("Archive channel directory")?;
        let watch = ArchiveWatch::new(&archive);
        watch.check()?;
        let mut out = io::stdout().lock();
        writeln!(
            out,
            "chrparams source census: Build {} {} {} {}",
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id()
        )?;
        out.flush()?;
        drop(out);
        self.walk(&archive, &watch, false)?;
        self.stage = "confirming complete census and unchanged Build";
        watch.check()?;
        require(
            Build::read_from(directory)?.as_ref() == Some(build),
            "Build changed during source census",
        )?;
        self.counts.report(&mut io::stdout().lock())
    }

    fn walk(&mut self, archive: &Archive, watch: &ArchiveWatch, nested: bool) -> TestResult {
        self.stage = "checking physical archive directory";
        require(
            !archive
                .issues()
                .iter()
                .any(|(_, issue)| issue.is_archive_wide()),
            "complete physical archive directory required",
        )?;
        for entry in archive.entries() {
            watch.poll()?;
            let Some((_, extension)) = entry.path().rsplit_once('.') else {
                continue;
            };
            if extension.eq_ignore_ascii_case("chrparams") {
                self.entry(archive, entry, watch)?;
                if nested {
                    self.counts.nested += 1;
                } else {
                    self.counts.root += 1;
                }
            } else if extension.eq_ignore_ascii_case("pak")
                || extension.eq_ignore_ascii_case("socpak")
            {
                self.stage = "opening physical nested Archive";
                let container = archive.open_container(entry)?;
                self.counts.containers += 1;
                self.walk(&container, watch, true)?;
            }
        }
        Ok(())
    }

    fn entry(&mut self, archive: &Archive, entry: &Entry, watch: &ArchiveWatch) -> TestResult {
        self.stage = "reading CRC-verified parameter payload";
        let size = entry.uncompressed_size();
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(usize::try_from(size)?)?;
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        require(
            u64::try_from(bytes.len())? == size,
            "exact decoded payload size",
        )?;
        self.stage = "comparing source declarations and scoped selectors";
        let document = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        require(!document.is_lossy(), "complete original document strings")?;
        self.counts.syntax(document.root())?;
        selectors::compare(&bytes, document.root(), &mut self.counts.selectors, watch)?;
        self.counts.bytes = self
            .counts
            .bytes
            .checked_add(size)
            .ok_or("source byte total")?;
        match document.format() {
            Format::Text => self.counts.text += 1,
            Format::CryXml(_) => self.counts.binary += 1,
        }
        Ok(())
    }
}

impl Counts {
    fn syntax(&mut self, root: SourceElement<'_>) -> TestResult {
        let mut pending = vec![root];
        while let Some(element) = pending.pop() {
            self.elements += 1;
            self.default_pose_sites +=
                usize::from(selectors::default_pose_spelling(element.name()));
            self.declarations(element)?;
            for child in element.elements() {
                pending.try_reserve(1)?;
                pending.push(child);
            }
        }
        Ok(())
    }

    fn declarations(&mut self, element: SourceElement<'_>) -> TestResult {
        let mut attributes = element.attributes();
        for declaration in element.syntax()?.declarations() {
            let declaration = declaration?;
            let kind = declaration.kind();
            let namespace = match kind {
                SourceDeclarationKind::Attribute { namespace } => namespace,
                SourceDeclarationKind::Namespace { .. } => {
                    self.namespaces += 1;
                    if !matches!(
                        declaration.position(),
                        SourceAttributePosition::CryXml { .. }
                    ) {
                        continue;
                    }
                    None
                }
                _ => return Err("unhandled source declaration".into()),
            };
            let attribute = attributes.next().ok_or("extra lexical attribute")?;
            let qualified = declaration.qualified_name();
            let local = if namespace.is_some() {
                qualified
                    .rsplit_once(':')
                    .map_or(qualified, |(_, local)| local)
            } else {
                qualified
            };
            require(
                local == attribute.name()
                    && namespace == attribute.namespace()
                    && declaration.value() == attribute.value()
                    && declaration.position() == attribute.position(),
                "original attribute order, name, value and coordinate",
            )?;
            self.attributes += usize::from(matches!(kind, SourceDeclarationKind::Attribute { .. }));
            self.default_pose_sites += usize::from(selectors::default_pose_spelling(local));
        }
        require(
            attributes.next().is_none(),
            "every attribute has lexical syntax",
        )
    }

    fn report(&self, out: &mut impl io::Write) -> TestResult {
        require(
            self.root + self.nested == self.text + self.binary
                && self.root + self.nested > 0
                && self.attributes > 0
                && self.selectors.targets > 0,
            "positive complete parameter and selector census",
        )?;
        writeln!(
            out,
            "chrparams CRC verified: {} root, {} nested, {} containers, {} decoded bytes",
            self.root, self.nested, self.containers, self.bytes
        )?;
        writeln!(
            out,
            "chrparams ordered XML: {} text, {} CryXml, {} elements, {} attributes, {} namespace declarations",
            self.text, self.binary, self.elements, self.attributes, self.namespaces
        )?;
        writeln!(
            out,
            "chrparams selectors: {} declarations, {} wildcards, {} folder directives, {} malformed folders, {} reset folders, {} exact targets",
            self.selectors.declarations,
            self.selectors.wildcards,
            self.selectors.folders,
            self.selectors.malformed_folders,
            self.selectors.resets,
            self.selectors.targets
        )?;
        writeln!(
            out,
            "chrparams possible authored default-pose spellings: {}",
            self.default_pose_sites + self.selectors.default_pose_names
        )?;
        out.flush()?;
        Ok(())
    }
}

fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
