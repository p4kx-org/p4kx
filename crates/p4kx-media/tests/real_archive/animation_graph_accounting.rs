// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::io::{self, Read, Write as _};
use std::path::Path;

use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};
use p4kx_extraction::{Archive, ArchiveWatch, Build, Entry, Exclusion, Filter};
use p4kx_media::{
    ContainerIdentity, ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status,
};

use super::archive_replay::Lines;
use super::shader_accounting::require;
use super::{TestResult, manifests, privacy};

const FILTER_VAR: &str = "P4KX_ANIMATION_GRAPH_FILTER";

#[path = "animation_graph_accounting_clips.rs"]
pub(crate) mod clips;
#[path = "animation_graph_accounting_links.rs"]
mod links;
#[path = "animation_graph_accounting_source.rs"]
mod source;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Adb,
    Bspace,
    Comb,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn standalone_animation_graphs_reconcile_with_original_archive_sources()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    let filter = match env::var(FILTER_VAR) {
        Ok(value) if !value.trim().is_empty() => Some(Filter::parse(&value)?),
        Err(env::VarError::NotPresent) => None,
        _ => return Err("set a non-blank animation graph filter or leave it unset".into()),
    };
    let key = p4kx_testkit::real_archive_key()?;
    for path in std::iter::once(p4kx_testkit::real_archive_path()?)
        .chain(p4kx_testkit::real_archive_b_path())
    {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        if let Some(filter) = &filter {
            let build = archive.build().ok_or("authoritative installed Build")?;
            let directory = archive
                .path()
                .parent()
                .ok_or("installed channel directory")?;
            verify(&archive, Some(filter), true)?;
            require(
                Build::read_from(directory)?.as_ref() == Some(build),
                "unchanged installed Build",
            )?;
            writeln!(
                io::stdout(),
                "standalone graph witness Build {} {} {} {}",
                build.branch(),
                build.version(),
                build.change_number(),
                build.build_id(),
            )?;
        } else {
            verify(&archive, None, false)?;
        }
    }
    Ok(())
}

fn verify(archive: &Archive, root_filter: Option<&Filter>, validate_schema: bool) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let options = graph_export_options(archive, root_filter)?;
    let output = tempfile::tempdir()?;
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, output.path(), &options)?
    else {
        return Err("standalone definition selection did not publish a complete index".into());
    };
    let [stream] = index.kinds.as_slice() else {
        return Err("standalone definition export published another provider".into());
    };
    require(
        stream.provider.kind == MediaKind::AnimationDefinition,
        "definition kind is selected",
    )?;
    let base = output.path().join(&index.artifact_base);
    let validator = validate_schema
        .then(p4kx_media::builtin_record_schema)
        .transpose()?;
    let validator = validator
        .as_ref()
        .map(jsonschema::validator_for)
        .transpose()?;
    let mut census = Census {
        lines: Lines::open(&base.join(&stream.artifact.path))?,
        next: None,
        consumed: 0,
        counts: [0; 3],
        base: &base,
        watch: &watch,
        root_filter,
        validator: validator.as_ref(),
    };
    census.advance()?;
    census.directory(archive, &mut Vec::new())?;
    require(
        census.next.is_none(),
        "every standalone graph row has a physical original",
    )?;
    require(
        census.consumed == stream.counts.records.written,
        "every graph row is accounted for",
    )?;
    require(
        census.counts.iter().sum::<u64>() == stream.counts.source_entries.written,
        "every standalone source is counted once",
    )?;
    require_all_families(census.counts)?;
    watch.check()?;
    output.close()?;
    Ok(())
}

fn graph_export_options(
    archive: &Archive,
    root_filter: Option<&Filter>,
) -> TestResult<ExportOptions> {
    let exclusions = if let Some(filter) = root_filter {
        let exclusions = Exclusion::parse([
            "*.animevents",
            "*.chrparams",
            "*.pak",
            "*.socpak",
            "*.aim",
            "*.caf",
            "*.dba",
        ])?;
        check_root_selection(archive, filter, &exclusions)?;
        exclusions
    } else {
        Exclusion::parse(["*.animevents", "*.chrparams"])?
    };
    let mut options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        include: root_filter.cloned(),
        exclude: exclusions,
        ..ExportOptions::default()
    };
    if root_filter.is_some() {
        options.limits.output_bytes = manifests::MAX_ARTIFACT_BYTES;
    }
    Ok(options)
}

fn check_root_selection(archive: &Archive, filter: &Filter, exclude: &Exclusion) -> TestResult {
    let mut counts = [0; 3];
    let mut bytes = 0_u64;
    for entry in archive
        .entries()
        .iter()
        .filter(|entry| filter.matches(entry.path()) && !exclude.leaves_out(entry.path()))
    {
        bytes = bytes
            .checked_add(entry.uncompressed_size())
            .ok_or("selected raw byte count overflow")?;
        if let Some(family) = Family::of(entry.path()) {
            let count = counts
                .get_mut(family as usize)
                .ok_or("invalid definition family")?;
            *count += 1;
        }
    }
    require_all_families(counts)?;
    require(
        bytes <= manifests::MAX_ARTIFACT_BYTES,
        "all selected raw Entries fit the installed test write allowance",
    )?;
    writeln!(
        io::stdout(),
        "selected root graph sources: {} ADB, {} BSPACE, {} COMB, {bytes} total retained raw bytes",
        counts[0],
        counts[1],
        counts[2],
    )?;
    Ok(())
}

fn require_all_families(counts: [u64; 3]) -> TestResult {
    require(
        counts.into_iter().all(|count| count > 0),
        "all three graph definition families have a witness",
    )
}

struct Census<'a> {
    lines: Lines,
    next: Option<MediaRecord>,
    consumed: u64,
    counts: [u64; 3],
    base: &'a Path,
    watch: &'a ArchiveWatch,
    root_filter: Option<&'a Filter>,
    validator: Option<&'a jsonschema::Validator>,
}

impl Census<'_> {
    fn directory(&mut self, archive: &Archive, chain: &mut Vec<ContainerIdentity>) -> TestResult {
        let paths = links::Paths::new(archive, self.root_filter);
        let mut edges = links::Edges::default();
        for entry in archive.entries() {
            self.watch.poll()?;
            if let Some(family) = Family::of(entry.path()).filter(|_| paths.includes(entry.path()))
            {
                self.entry(archive, entry, chain, family, &paths, &mut edges)?;
                let Some(count) = self.counts.get_mut(family as usize) else {
                    return Err("invalid definition family".into());
                };
                *count += 1;
            } else if self.root_filter.is_none()
                && extension(entry.path()).is_some_and(|ext| {
                    ["pak", "socpak"]
                        .iter()
                        .any(|value| ext.eq_ignore_ascii_case(value))
                })
            {
                let identity = ContainerIdentity::try_new_with_entry(archive, entry)?;
                let nested = archive.open_container(entry)?;
                chain.push(identity);
                self.directory(&nested, chain)?;
                let _ = chain.pop();
            }
        }
        while self
            .next
            .as_ref()
            .is_some_and(|row| links::Edges::owns(row, chain))
        {
            edges.verify(&self.take()?)?;
        }
        edges.finish()
    }

    fn entry(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        chain: &[ContainerIdentity],
        family: Family,
        paths: &links::Paths<'_>,
        edges: &mut links::Edges,
    ) -> TestResult {
        let identity = ContainerIdentity::try_new_with_entry(archive, entry)?;
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let document = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        let mut source = source::ExpectedSource::new(&document, family)?;
        edges.document(entry.index(), source.known_root());
        let mut documents = 0;
        while self.next.as_ref().is_some_and(|row| {
            matches!(&row.source.identity,
                Identity::Entry { entry, containers } if entry == &identity && containers == chain
            ) && !links::Edges::owns(row, chain)
        }) {
            let row = self.take()?;
            if row.subresource.namespace == "animation-definition-document" {
                documents += 1;
                self.document(&row, &bytes, &source, family)?;
            } else {
                source.verify(&row, paths, chain, entry.index(), edges)?;
            }
        }
        require(
            documents == 1,
            "exactly one document row belongs to each physical graph input",
        )?;
        source.finish()
    }

    fn document(
        &self,
        row: &MediaRecord,
        original: &[u8],
        source: &source::ExpectedSource<'_>,
        family: Family,
    ) -> TestResult {
        let raw = row
            .source
            .raw
            .as_ref()
            .ok_or("standalone graph raw artifact is missing")?;
        let retained = manifests::check_payload(
            self.base,
            &raw.path,
            raw.size,
            &raw.digest,
            original.len() as u64,
        )?;
        require(
            retained == original,
            "retained graph bytes match the verified physical input",
        )?;
        require(
            row.status == Status::Complete,
            "all accepted original graph documents are complete",
        )?;
        require(
            row.data
                .pointer("/payload/family")
                .and_then(serde_json::Value::as_str)
                == Some(family.name()),
            "document family comes from the original Entry",
        )?;
        require(
            row.data
                .pointer("/payload/projection")
                .and_then(serde_json::Value::as_str)
                == Some("readable"),
            "document projection is explicitly readable",
        )?;
        require(
            links::decimal(&row.data, "/payload/nodes")? == source.node_count(),
            "document census counts every ordered source content node",
        )
    }

    fn take(&mut self) -> TestResult<MediaRecord> {
        let row = self
            .next
            .take()
            .ok_or("missing expected standalone graph row")?;
        require(
            row.kind == MediaKind::AnimationDefinition,
            "graph rows have the only requested provider",
        )?;
        if let Some(validator) = self.validator {
            require(
                validator.is_valid(&serde_json::to_value(&row)?),
                "every synthetic row satisfies the built-in schema",
            )?;
        }
        self.consumed += 1;
        self.advance()?;
        Ok(row)
    }

    fn advance(&mut self) -> TestResult {
        self.next = self.lines.next()?;
        Ok(())
    }
}

impl Family {
    fn of(path: &str) -> Option<Self> {
        let extension = extension(path)?;
        [Self::Adb, Self::Bspace, Self::Comb]
            .into_iter()
            .find(|family| extension.eq_ignore_ascii_case(family.name()))
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Adb => "adb",
            Self::Bspace => "bspace",
            Self::Comb => "comb",
        }
    }

    const fn root(self) -> &'static str {
        match self {
            Self::Adb => "AnimDB",
            Self::Bspace => "ParaGroup",
            Self::Comb => "CombinedBlendSpace",
        }
    }

    const fn root_role(self) -> &'static str {
        match self {
            Self::Adb => "database",
            Self::Bspace => "blend-space",
            Self::Comb => "combined-space",
        }
    }
}

fn extension(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(_, extension)| extension)
}

#[cfg(test)]
#[path = "animation_graph_accounting_fixture.rs"]
mod fixture;
