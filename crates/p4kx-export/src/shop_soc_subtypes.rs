// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::{self, Write};
use std::time::Instant;

use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry, real_archive_key, real_archive_path,
};

use crate::catalog_read::original::with_entry_capture;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::shop_containers::Containers;
use crate::shop_soc::CapturedScan;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "shop_soc_structure.rs"]
mod structure;
pub(super) use structure::Probe;
use structure::{Evidence, InventoryValue, Metric, RawKind, Relation};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Subtypes {
    unattached_valid_path: bool,
    attached_invalid_value: bool,
    unattached_invalid_value: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NestedEntry {
    outer: usize,
    inner: usize,
}

#[derive(Debug)]
pub(crate) struct EntryFindings {
    entry: NestedEntry,
    subtypes: Subtypes,
    structural: Evidence,
}

impl EntryFindings {
    pub(super) fn inspect(
        &mut self,
        probe: &Probe<'_, '_>,
        original: &crate::catalog_read::original::EntryRead,
        observed: &CapturedScan,
    ) {
        self.structural = probe.inspect(original, &observed.scan.raw_findings);
    }
}

pub(super) fn findings(
    source: &EntrySource,
    observed: &CapturedScan,
) -> Result<Option<EntryFindings>, CatalogError> {
    if observed.scan.rejected == 0 && !observed.scan.malformed {
        return Ok(None);
    }
    let receipt = source.receipt();
    let (Some(outer), Some(inner)) = (receipt.entry_index(), receipt.container_entry_index())
    else {
        return Err(ExportAccountingError::UnboundHealthSubject.into());
    };
    Ok(Some(EntryFindings {
        entry: NestedEntry { outer, inner },
        structural: Evidence::default(),
        subtypes: Subtypes {
            unattached_valid_path: observed.scan.rejected > 0,
            attached_invalid_value: !observed.rejected.is_empty(),
            unattached_invalid_value: observed.unattributed_declarations,
        },
    }))
}

fn subtype_groups(containers: &Containers) -> TestResult<BTreeMap<Subtypes, usize>> {
    let reportable: BTreeSet<_> = containers
        .ledger
        .entries
        .iter()
        .filter(|entry| {
            entry.kind == Kind::Shops
                && entry.role == ExportSourceRole::Dependency
                && entry.operation == EntryOperation::ShopSoc
                && entry.disposition == SourceDisposition::ProjectionFailed
                && entry.diagnosis.code == Some(ExportDiagnosticCode::MalformedSource)
        })
        .map(|entry| {
            let outer = entry.entry_index.ok_or("shop SOC outer receipt")?;
            let inner = entry
                .container_entry_index
                .ok_or("shop SOC nested receipt")?;
            Ok::<_, Box<dyn Error>>(NestedEntry { outer, inner })
        })
        .collect::<Result<_, _>>()?;
    let findings = &containers.soc_findings;
    let identities: BTreeSet<_> = findings.iter().map(|item| item.entry).collect();
    assert_eq!(
        identities.len(),
        findings.len(),
        "one subtype row per physical Entry"
    );
    assert!(
        identities == reportable,
        "subtypes reconcile with the normal health ledger"
    );
    let mut groups = BTreeMap::new();
    for item in findings {
        *groups.entry(item.subtypes).or_insert(0) += 1;
    }
    Ok(groups)
}

fn structural_groups(
    containers: &Containers,
) -> BTreeMap<Subtypes, BTreeMap<Metric, (usize, usize)>> {
    let mut groups = BTreeMap::<_, BTreeMap<_, (usize, usize)>>::new();
    for item in &containers.soc_findings {
        let group = groups.entry(item.subtypes).or_default();
        for (metric, count) in item.structural.counts() {
            let (entries, occurrences) = group.entry(metric).or_default();
            *entries += 1;
            *occurrences += count;
        }
    }
    groups
}

#[test]
fn shop_soc_subtypes_distinguish_rejection_and_malformed_values() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/ObjectContainers/PU/Shops/fixture.socpak",
            source_container(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let observed = read(&archive)?;
    let groups = subtype_groups(&observed)?;
    for (unattached_valid_path, attached_invalid_value, unattached_invalid_value) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, false),
    ] {
        assert_eq!(
            groups.get(&Subtypes {
                unattached_valid_path,
                attached_invalid_value,
                unattached_invalid_value,
            }),
            Some(&1),
            "each synthetic subtype combination appears once"
        );
    }
    assert_eq!(groups.len(), 4);
    assert_eq!(
        observed.soc_findings.len(),
        4,
        "valid and empty values are not failures"
    );
    assert!(
        observed.unattributed_declarations,
        "the production resolution remains uncertain"
    );
    Ok(())
}

#[test]
fn the_structural_probe_matches_original_slots_and_distinguishes_pooled_adjacency() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/ObjectContainers/PU/Shops/fixture.socpak",
            structural_container(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let observed = read(&archive)?;
    assert_eq!(subtype_groups(&observed)?.values().sum::<usize>(), 4);
    let groups = structural_groups(&observed);
    let subtype = |valid_path| Subtypes {
        unattached_valid_path: valid_path,
        attached_invalid_value: !valid_path,
        unattached_invalid_value: false,
    };
    let valid = &groups[&subtype(true)];
    let invalid = &groups[&subtype(false)];
    assert_eq!(
        valid[&Metric::Raw(RawKind::UnattachedPath, Relation::Total)],
        (2, 2)
    );
    assert_eq!(
        valid[&Metric::Raw(
            RawKind::UnattachedPath,
            Relation::ExactInventoryValueWithUniqueMetadataGuid(InventoryValue::File),
        )],
        (1, 1),
        "only the original inventory value slot matches"
    );
    assert_eq!(
        valid[&Metric::Raw(RawKind::UnattachedPath, Relation::StartsInUninspectedChunk,)],
        (1, 1)
    );
    assert_eq!(
        invalid[&Metric::Raw(
            RawKind::AttachedInvalid,
            Relation::ExactInventoryPropertyWithSameAttachedGuid(InventoryValue::Empty),
        )],
        (1, 1),
        "the author used an empty value despite the nearby invalid token"
    );
    assert_eq!(
        invalid[&Metric::Raw(RawKind::AttachedInvalid, Relation::AttributeName)],
        (1, 1)
    );
    assert_eq!(
        invalid[&Metric::Raw(
            RawKind::AttachedInvalid,
            Relation::ExactInventoryPropertyAndValue,
        )],
        (1, 1),
        "a genuine invalid attribute references both original slots"
    );
    Ok(())
}

fn structural_container() -> Vec<u8> {
    const ENTITY: &str = "a0000000-0000-4000-8000-000000000001";
    const INVENTORY: &str = "Scripts/ShopInventories/good.json";
    let scenario = |component: TestElement| {
        let xml = TestElement::new("Entities")
            .child(
                TestElement::new("Entity")
                    .attribute("EntityCryGUID", ENTITY)
                    .child(TestElement::new("PropertiesDataCore").child(component)),
            )
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
        TestChunkFile::crch().chunk(0x0004, 3, xml).bytes()
    };
    let outside = TestChunkFile::crch()
        .chunk(0x0199, 1, format!("\0EntityComponentShop\0barrier\0{INVENTORY}\0").into_bytes())
        .chunk(0x0004, 3, format!(
            r#"<Entities><Entity EntityCryGUID="{ENTITY}"><PropertiesDataCore><EntityComponentShop offlineInventoryJSON="{INVENTORY}"/></PropertiesDataCore></Entity></Entities>"#
        ).into_bytes())
        .bytes();
    TestContainer::new()
        .entry(TestEntry::stored(
            "metadata/entitycomponentshop.xml",
            format!(r#"<Entities><Entity CryGUID="{ENTITY}"/></Entities>"#),
        ))
        .entry(TestEntry::stored(
            "unattached-valid.soc",
            scenario(
                TestElement::new("EntityComponentShop")
                    .attribute("unrelated", "barrier")
                    .attribute("offlineInventoryJSON", INVENTORY),
            ),
        ))
        .entry(TestEntry::stored(
            "pooled-adjacency.soc",
            scenario(
                TestElement::new("EntityComponentShop")
                    .attribute("offlineInventoryXMLName", "")
                    .attribute("another.attribute", "fixture-value"),
            ),
        ))
        .entry(TestEntry::stored(
            "authored-invalid.soc",
            scenario(
                TestElement::new("EntityComponentShop")
                    .attribute("offlineInventoryXMLName", "genuinely.invalid"),
            ),
        ))
        .entry(TestEntry::stored("uninspected.soc", outside))
        .build()
}

fn source_container() -> Vec<u8> {
    const ENTITY: &str = "a0000000-0000-4000-8000-000000000001";
    let metadata = format!(r#"<Entities><Entity CryGUID="{ENTITY}"/></Entities>"#);
    let unattached_valid = format!(
        "{ENTITY}\0EntityComponentShop\0DifferentOwner\0Scripts/ShopInventories/good.json\0"
    );
    let attached_invalid =
        format!("{ENTITY}\0EntityComponentShop\0offlineInventoryJSON\0good.invalid\0");
    let unattached_invalid =
        "DifferentOwner\0EntityComponentShop\0offlineInventoryJSON\0good.invalid\0";
    let mixed = format!("{unattached_valid}{attached_invalid}");
    let valid = format!("{ENTITY}\0EntityComponentShop\0offlineInventoryJSON\0good.json\0");
    let empty = format!("{ENTITY}\0EntityComponentShop\0offlineInventoryJSON\0\0");
    TestContainer::new()
        .entry(TestEntry::stored(
            "metadata/entitycomponentshop.xml",
            metadata,
        ))
        .entry(TestEntry::stored("unattached-valid.soc", unattached_valid))
        .entry(TestEntry::stored("attached-invalid.soc", attached_invalid))
        .entry(TestEntry::stored(
            "unattached-invalid.soc",
            unattached_invalid,
        ))
        .entry(TestEntry::stored("mixed.soc", mixed))
        .entry(TestEntry::stored("valid.soc", valid))
        .entry(TestEntry::stored("empty.soc", empty))
        .build()
}

fn read(archive: &Archive) -> Result<Containers, CatalogError> {
    with_entry_capture(archive, |capture| Containers::read(archive, None, capture))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_shop_soc_subtypes_report_has_no_record_or_path_values() -> TestResult {
    let path = real_archive_path()?;
    let channel = p4kx_channels::channel_name(&path).ok_or("identified Archive channel")?;
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))?;
    let build = archive.build().ok_or("identified Archive Build")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let start = Instant::now();
    let observed = read(&archive).map_err(|_| "canonical Shop SOC walk failed");
    watch.check()?;
    let observed = observed?;
    let groups = subtype_groups(&observed)?;
    let change = build.change_number();
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "shopSocSummary channel={channel} change={change} failedEntries={} elapsedMs={}",
        observed.soc_findings.len(),
        start.elapsed().as_millis()
    )?;
    for (subtypes, count) in groups {
        writeln!(
            out,
            "shopSocSubtype channel={channel} change={change} unattachedValidPath={} \
             attachedInvalidValue={} unattachedInvalidValue={} entries={count}",
            subtypes.unattached_valid_path,
            subtypes.attached_invalid_value,
            subtypes.unattached_invalid_value
        )?;
    }
    for (subtypes, metrics) in structural_groups(&observed) {
        for (metric, (entries, occurrences)) in metrics {
            writeln!(
                out,
                "shopSocStructure channel={channel} change={change} unattachedValidPath={} \
                 attachedInvalidValue={} unattachedInvalidValue={} metric={metric:?} \
                 entries={entries} occurrences={occurrences}",
                subtypes.unattached_valid_path,
                subtypes.attached_invalid_value,
                subtypes.unattached_invalid_value
            )?;
        }
    }
    Ok(())
}
