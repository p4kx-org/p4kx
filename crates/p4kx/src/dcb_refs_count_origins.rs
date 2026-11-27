// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use anyhow::{Context as _, Result, ensure};
use p4kx_extraction::{
    FieldOrigins, ObservedFileGraph, ReferenceScanGap, SourceUse, SourceUseRole,
};

use super::{HeaderField, Origins};

#[derive(Clone)]
pub(in crate::dcb_refs_json) struct CountEvidence {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl CountEvidence {
    pub(in crate::dcb_refs_json) const fn empty() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }

    pub(in crate::dcb_refs_json) fn add(&mut self, used: SourceUse) -> Result<()> {
        self.uses.try_reserve(1)?;
        self.uses.push(used);
        Ok(())
    }

    pub(in crate::dcb_refs_json) fn include(&mut self, other: &Self) -> Result<()> {
        self.uses.try_reserve(other.uses.len())?;
        self.uses.extend(other.uses.iter().cloned());
        self.complete &= other.complete;
        Ok(())
    }

    pub(in crate::dcb_refs_json) const fn incomplete(&mut self) {
        self.complete = false;
    }

    pub(in crate::dcb_refs_json) fn field_with(
        &mut self,
        origins: &Origins<'_, '_>,
        fields: &mut FieldOrigins,
        path: &str,
        extra: &Self,
    ) -> Result<()> {
        let length = self.uses.len();
        let complete = self.complete;
        self.include(extra)?;
        let result = self.field(origins, fields, path);
        self.uses.truncate(length);
        self.complete = complete;
        result
    }

    pub(in crate::dcb_refs_json) fn field(
        &self,
        origins: &Origins<'_, '_>,
        fields: &mut FieldOrigins,
        path: &str,
    ) -> Result<()> {
        origins.insert(fields, path, &self.uses, self.complete)
    }
}

impl Origins<'_, '_> {
    pub(in crate::dcb_refs_json) fn whole_scan(&self) -> Result<CountEvidence> {
        let total = u64::try_from(self.index.forge().record_count())?;
        let mut proof = self.scan(0..total, self.index.gaps())?;
        proof.add(self.record_bound()?)?;
        Ok(proof)
    }

    pub(in crate::dcb_refs_json) fn record_scan(&self, record: u32) -> Result<CountEvidence> {
        let gaps = self.index.gaps();
        let start = gaps.partition_point(|gap| gap.record_index() < record);
        let end = gaps.partition_point(|gap| gap.record_index() <= record);
        let gaps = gaps
            .get(start..end)
            .context("original Reference scan range")?;
        let record = u64::from(record);
        self.scan(record..record + 1, gaps)
    }

    fn scan(&self, records: Range<u64>, gaps: &[ReferenceScanGap]) -> Result<CountEvidence> {
        let mut proof = CountEvidence::empty();
        let mut next = records.start;
        for &gap in gaps {
            let record = u64::from(gap.record_index());
            ensure!(records.contains(&record), "foreign Reference scan gap");
            self.read_range(&mut proof, next..record)?;
            self.attempt(gap)?;
            proof.complete = false;
            next = record + 1;
        }
        self.read_range(&mut proof, next..records.end)?;
        Ok(proof)
    }

    fn read_range(&self, proof: &mut CountEvidence, range: Range<u64>) -> Result<()> {
        if !range.is_empty() {
            proof.add(self.observe(
                "dataforgeRecordBodyRange",
                vec![range.start, range.end],
                "referenceMembership",
                SourceUseRole::Selector,
            )?)?;
        }
        Ok(())
    }

    fn attempt(&self, gap: ReferenceScanGap) -> Result<()> {
        match gap {
            ReferenceScanGap::Record { record_index } => {
                self.observe(
                    "dataforgeRecordBody",
                    vec![u64::from(record_index)],
                    "referenceMembership",
                    SourceUseRole::Attempted,
                )?;
            }
            ReferenceScanGap::Value {
                record_index,
                value_ordinal,
            } => {
                self.observe(
                    "dataforgeValue",
                    vec![u64::from(record_index), value_ordinal],
                    "$value",
                    SourceUseRole::Attempted,
                )?;
            }
            _ => anyhow::bail!("unknown Reference scan gap"),
        }
        Ok(())
    }

    pub(in crate::dcb_refs_json) fn file_scan(
        &self,
        graph: &ObservedFileGraph<'_, '_>,
    ) -> Result<CountEvidence> {
        let mut proof = CountEvidence::empty();
        proof.add(self.record_bound()?)?;
        let mut start = 0_u64;
        for record in self.index.forge().records() {
            let index = record.index();
            if graph.position_of_record(index).is_none() {
                let at = u64::from(index);
                self.path_range(&mut proof, start..at)?;
                self.header(index, HeaderField::FileName, SourceUseRole::Attempted)?;
                start = at + 1;
                proof.complete = false;
            }
        }
        self.path_range(
            &mut proof,
            start..u64::try_from(self.index.forge().record_count())?,
        )?;
        Ok(proof)
    }

    fn path_range(&self, proof: &mut CountEvidence, range: Range<u64>) -> Result<()> {
        if !range.is_empty() {
            proof.add(self.observe(
                "dataforgeRecordHeaderRange",
                vec![range.start, range.end],
                "fileName",
                SourceUseRole::Selector,
            )?)?;
        }
        Ok(())
    }

    fn record_bound(&self) -> Result<SourceUse> {
        self.observe(
            "dataforgeHeader",
            Vec::new(),
            "recordCount",
            SourceUseRole::Selector,
        )
    }
}
