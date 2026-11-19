// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_extraction::{
    ArchiveWatch, Build, DataForge, DataForgeOrigin, InputScope, PointerProbe, Record, RecordTree,
    SourceEncryption, SourceInput, SourceScope, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};

use crate::real_archive_item_durability::{self as proof, Trace};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) struct ComponentEvidence<'source> {
    forge: &'source DataForge<'source>,
    build: &'source Build,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'source> ComponentEvidence<'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        origin: &DataForgeOrigin,
        build: &'source Build,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(source) = origin else {
            return Err("the installed DataForge needs an original Archive Entry".into());
        };
        let scope = SourceUses::with_compact_coordinates()
            .observe(SourceInput::Archive(source), InputScope::Payload)
            .map_err(|_| "admit the original DataForge Entry")?;
        let encrypted = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        Ok(Self {
            forge,
            build,
            scope,
            encrypted,
        })
    }

    pub(super) fn verify<Row>(
        &self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        index: u32,
        kinds: &[&str],
        owner: impl Fn(&Row) -> &ComponentOwner,
        mut compare: impl FnMut(&Row, &StructValue<'source>, &Trace<'_, 'source>) -> TestResult,
    ) -> TestResult {
        let tree = self
            .forge
            .record(index)
            .ok_or("one original physical Record")?
            .tree()
            .map_err(|_| "read the selected original DataForge Record")?;
        let coverage = snapshot.coverage();
        assert!(
            snapshot.build() == Some(self.build)
                && matches!(snapshot.availability(), DatasetAvailability::Available)
                && coverage.unreadable_headers.is_empty()
                && coverage.unreadable_bodies.is_empty(),
            "the selected equipment family is readable on this exact Build"
        );
        assert!(
            snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
            "component lookup does not admit any locale payload"
        );
        let inventory = serde_json::to_value(snapshot.source_inventory())?;
        let uses = proof::indexed(&inventory, "/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &self.scope, self.encrypted);
        let Some(Original::Array(components)) = tree.field("Components") else {
            return Err("the original owner needs a Components array".into());
        };
        let mut rows = snapshot.rows().iter();
        let mut matched = false;
        for (ordinal, raw) in components.iter().enumerate() {
            let Original::Struct(original) = raw else {
                continue;
            };
            if !kinds.contains(&original.type_name()) {
                continue;
            }
            let row = rows
                .next()
                .ok_or("SDK must retain every matching physical slot")?;
            verify_owner(owner(row), &tree, u32::try_from(ordinal)?, raw, &trace)?;
            compare(row, original, &trace)?;
            matched = true;
        }
        assert!(
            matched && rows.next().is_none(),
            "exactly the original component occurrences"
        );
        Ok(())
    }
}

fn verify_owner<'source>(
    owner: &ComponentOwner,
    tree: &RecordTree<'source>,
    ordinal: u32,
    raw: &Original<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    assert!(
        owner.record_index().value() == Some(&tree.record_index())
            && owner
                .guid()
                .value()
                .is_some_and(|guid| guid.get() == tree.guid())
            && owner.component_ordinal().value() == Some(&ordinal),
        "the SDK retains this original owner and Components occurrence"
    );
    trace.header(
        &serde_json::to_value(owner.record_index())?,
        "recordIndex",
        "selector",
    )?;
    trace.header(&serde_json::to_value(owner.guid())?, "guid", "payload")?;
    trace.check(
        &serde_json::to_value(owner.component_ordinal())?,
        raw,
        "conversion",
    )
}

pub(super) struct ComponentCandidate<'source> {
    kind: &'static str,
    probe: PointerProbe<'source>,
    qualifies: fn(&StructValue<'_>) -> bool,
    require_every_occurrence: bool,
    owner: Option<u32>,
}

impl<'source> ComponentCandidate<'source> {
    pub(super) fn new(
        forge: &'source DataForge<'source>,
        kind: &'static str,
        qualifies: fn(&StructValue<'_>) -> bool,
    ) -> TestResult<Self> {
        let probe = forge
            .strict_pointer_probe("Components", kind)
            .ok_or("the installed DataForge must declare the required component family")?;
        Ok(Self {
            kind,
            probe,
            qualifies,
            require_every_occurrence: false,
            owner: None,
        })
    }

    pub(super) const fn requiring_every_occurrence(mut self) -> Self {
        self.require_every_occurrence = true;
        self
    }

    pub(super) fn owner(&self, missing: &'static str) -> TestResult<u32> {
        self.owner.ok_or_else(|| missing.into())
    }

    fn matches(&self, components: &[Original<'source>]) -> bool {
        let mut matching = components.iter().filter_map(|slot| match slot {
            Original::Struct(value) if value.type_name() == self.kind => Some(value),
            _ => None,
        });
        if self.require_every_occurrence {
            matching.next().is_some_and(self.qualifies) && matching.all(self.qualifies)
        } else {
            matching.any(self.qualifies)
        }
    }
}

pub(super) fn select_component_owners<'source, const N: usize>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    candidates: &mut [ComponentCandidate<'source>; N],
    admitted: impl Fn(&Record<'source>) -> TestResult<bool>,
) -> TestResult {
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during equipment discovery")?;
        if record.type_name().ok() != Some("EntityClassDefinition") {
            continue;
        }
        let matches = candidates.each_ref().map(|candidate| {
            candidate.owner.is_none() && trees.holds(&candidate.probe, &record).ok() == Some(true)
        });
        if !matches.contains(&true) || !admitted(&record)? {
            continue;
        }
        let Ok(tree) = trees.build(&record) else {
            continue;
        };
        let Some(Original::Array(components)) = tree.field("Components") else {
            continue;
        };
        if !components
            .iter()
            .all(|value| matches!(value, Original::Struct(_) | Original::Null))
        {
            continue;
        }
        for (candidate, held) in candidates.iter_mut().zip(matches) {
            if held && candidate.matches(components) {
                candidate.owner = Some(record.index());
            }
        }
        if candidates.iter().all(|candidate| candidate.owner.is_some()) {
            break;
        }
    }
    Ok(())
}
