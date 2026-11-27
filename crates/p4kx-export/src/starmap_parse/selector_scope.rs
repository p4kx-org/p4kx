// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, ArchiveWatch, Guid, SourceValue};
use serde::Deserialize;

use crate::catalog_read::original::{EntryCapture, EntryXml, with_entry_capture};
use crate::export_ledger::ExportLedger;
use crate::starmap_index::{self, Index, SOCPAK};
use crate::starmap_read;
use crate::starmap_xml::{self, Node, Root};
use crate::{CatalogError, ExportAccountingError};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type Requests<'receipt> = BTreeMap<usize, OwnerRequest<'receipt>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    build: String,
    source_rows: Vec<SourceRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRow {
    kind: String,
    role: String,
    selector: String,
    authoring_entry_routes: Vec<Vec<usize>>,
}

struct OwnerRequest<'receipt> {
    descriptor: u32,
    selectors: BTreeSet<&'receipt str>,
}

impl Receipt {
    fn read(path: &Path) -> TestResult<Self> {
        let file = std::fs::File::open(path).map_err(|_| "open the private Starmap receipt")?;
        serde_json::from_reader(file).map_err(|_| "decode the private Starmap receipt".into())
    }

    fn requests(&self) -> TestResult<Requests<'_>> {
        let mut requests = Requests::new();
        for row in self
            .source_rows
            .iter()
            .filter(|row| row.kind == "starmapObjects" && row.role == "dependency")
        {
            let [route] = row.authoring_entry_routes.as_slice() else {
                return Err("ambiguous Starmap source route in private receipt".into());
            };
            let [owner, descriptor] = route.as_slice() else {
                return Err("unexpected Starmap source depth in private receipt".into());
            };
            let descriptor = u32::try_from(*descriptor)?;
            let request = requests.entry(*owner).or_insert_with(|| OwnerRequest {
                descriptor,
                selectors: BTreeSet::new(),
            });
            if request.descriptor != descriptor || !request.selectors.insert(&row.selector) {
                return Err("duplicate or conflicting Starmap receipt selection".into());
            }
        }
        if requests.is_empty() {
            return Err("private receipt has no scoped Starmap dependencies".into());
        }
        Ok(requests)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Outcome {
    OwnerAbsent,
    DescriptorUnavailable,
    DescriptorMismatch,
    DescriptorUnparsed,
    NoOriginalChild,
    OriginalChild,
    ChildInCryXml,
    ChildInNamedDescriptor,
    ChildWithUsableLocalPose,
    ChildWithStarmapRecord,
    ChildWithInlineChildList,
    Resolution(Resolution),
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct Resolution {
    outer: usize,
    containing: usize,
    containing_relativized: usize,
    containing_basename: usize,
    containing_same_stem_soc: usize,
}

#[test]
#[ignore = "installed Archive and private P4KX_STARMAP_SELECTOR_WITNESS receipt"]
fn installed_starmap_selector_source_scopes() -> TestResult {
    use std::io::Write as _;

    let path = std::env::var_os("P4KX_STARMAP_SELECTOR_WITNESS")
        .map(PathBuf::from)
        .ok_or("configure the private Starmap receipt")?;
    let receipt = Receipt::read(&path)?;
    let requests = receipt.requests()?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive
        .build()
        .ok_or("the installed Archive has no Build")?;
    if build.change_number() != receipt.build
        || [build.branch(), build.version(), build.build_id()]
            .into_iter()
            .any(str::is_empty)
    {
        return Err("private receipt Build change does not match the installed Archive".into());
    }
    watch
        .check()
        .map_err(|_| "Archive changed before the source comparison")?;
    let counts = with_entry_capture(&archive, |capture| inspect(&archive, capture, &requests))
        .map_err(|_| "cannot verify the original Starmap child declarations")?;
    watch
        .check()
        .map_err(|_| "Archive changed during the source comparison")?;
    let mut output = io::stdout().lock();
    let selections: usize = requests
        .values()
        .map(|request| request.selectors.len())
        .sum();
    writeln!(
        output,
        "starmap_selector_scope change={} owners={} selections={selections}",
        build.change_number(),
        requests.len()
    )?;
    for (outcome, count) in counts {
        writeln!(
            output,
            "starmap_selector_scope observation={outcome:?} count={count}"
        )?;
    }
    Ok(())
}

fn inspect(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    requests: &Requests<'_>,
) -> Result<BTreeMap<Outcome, usize>, CatalogError> {
    let index = Index::new(archive);
    let mut counts = BTreeMap::new();
    for (&owner, request) in requests {
        if let Some(blocker) = observe_owner(&index, archive, owner, capture, request, &mut counts)?
        {
            *counts.entry(blocker).or_default() += request.selectors.len();
        }
    }
    Ok(counts)
}

fn observe_owner(
    index: &Index<'_>,
    archive: &Archive,
    owner: usize,
    capture: &EntryCapture<'_>,
    request: &OwnerRequest<'_>,
    counts: &mut BTreeMap<Outcome, usize>,
) -> Result<Option<Outcome>, CatalogError> {
    let Some(entry) = archive
        .entries()
        .get(owner)
        .filter(|entry| entry.index() == owner)
    else {
        return Ok(Some(Outcome::OwnerAbsent));
    };
    let Ok(main) = starmap_read::main_xml(archive, entry, capture, &mut ExportLedger::default())?
    else {
        return Ok(Some(Outcome::DescriptorUnavailable));
    };
    let reference = main.source.receipt().payload_reference(capture)?;
    if !matches!(reference.value(), SourceValue::Present(source) if source.entry_index == request.descriptor)
    {
        return Ok(Some(Outcome::DescriptorMismatch));
    }
    let Ok(xml) = EntryXml::parse_starmap_main(&main.bytes, capture)? else {
        return Ok(Some(Outcome::DescriptorUnparsed));
    };
    if xml.is_lossy() {
        return Ok(Some(Outcome::DescriptorUnparsed));
    }
    let mut observed = BTreeSet::new();
    super::visit_original_children(&xml, |node, selected| {
        let Some(selected) = selected else {
            return Ok(());
        };
        let Some(selector) = request.selectors.get(selected.as_str()) else {
            return Ok(());
        };
        let (uses, complete) = selected.selector_inputs()?;
        if uses.is_empty() || !complete {
            return Err(ExportAccountingError::UnboundHealthSubject.into());
        }
        observed.insert(*selector);
        observe_child(node, main.fallback.is_none(), counts);
        Ok(())
    })?;
    for &selector in &observed {
        let found = Resolution::inspect(index, &main.container, selector);
        *counts.entry(Outcome::Resolution(found)).or_default() += 1;
    }
    let missing = request.selectors.len() - observed.len();
    if missing > 0 {
        *counts.entry(Outcome::NoOriginalChild).or_default() += missing;
    }
    Ok(None)
}

fn observe_child(node: Root<'_>, named_descriptor: bool, counts: &mut BTreeMap<Outcome, usize>) {
    let starmap_record = starmap_xml::attribute(node, "starMapRecord")
        .and_then(|value| value.trim().parse::<Guid>().ok())
        .is_some_and(|guid| guid.stored() != [0; 16]);
    let features = [
        (true, Outcome::OriginalChild),
        (matches!(node, Root::Cry(_)), Outcome::ChildInCryXml),
        (named_descriptor, Outcome::ChildInNamedDescriptor),
        (
            starmap_xml::pose(node).is_some(),
            Outcome::ChildWithUsableLocalPose,
        ),
        (starmap_record, Outcome::ChildWithStarmapRecord),
        (
            node.children().any(super::is_child_list),
            Outcome::ChildWithInlineChildList,
        ),
    ];
    for (present, category) in features {
        if present {
            *counts.entry(category).or_default() += 1;
        }
    }
}

impl Resolution {
    fn inspect(index: &Index<'_>, container: &Archive, selector: &str) -> Self {
        let mut result = Self {
            outer: index.candidates(selector).len(),
            ..Self::default()
        };
        let qualified = starmap_index::key(selector);
        let Some(stem) = qualified.strip_suffix(SOCPAK) else {
            return result;
        };
        let basename = qualified.rsplit('\\').next().unwrap_or_default();
        let local_soc = format!("{stem}.soc");
        for entry in container.entries() {
            let actual = starmap_index::key(entry.path());
            result.containing += usize::from(actual == qualified);
            result.containing_relativized += usize::from(
                ["data\\", "data\\objectcontainers\\"]
                    .into_iter()
                    .filter_map(|prefix| qualified.strip_prefix(prefix))
                    .any(|candidate| candidate == actual),
            );
            result.containing_basename += usize::from(actual.rsplit('\\').next() == Some(basename));
            result.containing_same_stem_soc += usize::from(actual == local_soc);
        }
        result
    }
}
