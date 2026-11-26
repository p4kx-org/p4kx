// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{AudioIndex, BreakReason, BrokenLink, HircPathSource, TriggerSounds};
use p4kx_extraction::{Archive, FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUses};

#[path = "wwise_extract_report_origins.rs"]
mod extract_report_origins;
#[path = "wwise_resolve_source_fields.rs"]
mod source_fields;
#[path = "wwise_resolve_totals.rs"]
mod totals;
#[path = "wwise_resolve_variant_origins.rs"]
mod variant_origins;

pub(crate) use extract_report_origins::capture_extract;
use source_fields::{Definition, LanguageOrigins};
use totals::{SummaryFields, Totals};

pub(crate) fn capture(
    archive: &Archive,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut capture = Capture::open(archive, index, &uses, sounds)?;
    let root = FieldPath::root();
    capture.header(sounds)?;
    capture
        .resolved(sounds, index, &root)?
        .write(&mut capture, &root, SummaryFields::Counts)?;
    capture.issues(index)?;
    Ok((capture.fields, capture.uses.freeze()?))
}

pub(crate) fn capture_nested(
    archive: &Archive,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
    uses: &SourceUses,
    root: &FieldPath,
    trigger_selector: &FieldOrigin,
) -> Result<(FieldOrigins, FieldOrigin)> {
    let mut capture = Capture::open(archive, index, uses, sounds)?;
    capture.definition.selection =
        uses.origin_with_origins([&capture.definition.selection, trigger_selector], true)?;
    let origin =
        capture
            .resolved(sounds, index, root)?
            .write(&mut capture, root, SummaryFields::Nested)?;
    Ok((capture.fields, origin))
}

struct Capture<'a> {
    archive: &'a Archive,
    uses: SourceUses,
    fields: FieldOrigins,
    definition: Definition<'a>,
    language: LanguageOrigins,
}

struct Step {
    kind: FieldOrigin,
    id: FieldOrigin,
}

struct Route {
    path: FieldOrigin,
    end: Option<Step>,
}

struct BreakOrigins {
    row: FieldOrigin,
    reason: FieldOrigin,
}

impl<'a> Capture<'a> {
    fn open(
        archive: &'a Archive,
        index: &AudioIndex,
        uses: &'a SourceUses,
        sounds: &TriggerSounds<'a>,
    ) -> Result<Self> {
        Ok(Self {
            archive,
            uses: uses.clone(),
            fields: FieldOrigins::new(),
            definition: Definition::open(archive, index, uses, sounds.trigger())?,
            language: LanguageOrigins::of(archive, index, uses, sounds.language_choice())?,
        })
    }

    fn resolved(
        &mut self,
        sounds: &TriggerSounds<'_>,
        index: &AudioIndex,
        root: &FieldPath,
    ) -> Result<Totals> {
        let variants = field(root, "variants")?;
        let breaks = field(root, "breaks")?;
        let mut totals = Totals::default();
        for (ordinal, variant) in sounds.variants().iter().enumerate() {
            let proof = variant_origins::write(self, &index_path(&variants, ordinal)?, variant)?;
            totals.variant(proof);
        }
        let searched = self.searched_banks(sounds, index)?;
        for (ordinal, link) in sounds.breaks().iter().enumerate() {
            let proof =
                self.broken_link(&index_path(&breaks, ordinal)?, link, searched.as_ref())?;
            totals.broken_link(proof, link.reason());
        }
        Ok(totals)
    }

    fn searched_banks(
        &self,
        sounds: &TriggerSounds<'_>,
        index: &AudioIndex,
    ) -> Result<Option<FieldOrigin>> {
        sounds
            .breaks()
            .iter()
            .any(|link| matches!(link.reason(), BreakReason::NoSoundbank))
            .then(|| source_fields::bank_survey(self.archive, index, &self.uses))
            .transpose()
    }

    fn header(&mut self, sounds: &TriggerSounds<'_>) -> Result<()> {
        let root = FieldPath::root();
        self.put(&root, "archive", self.uses.origin(&[], true))?;
        let language = self.group(
            &field(&root, "language")?,
            [
                ("requested", self.language.requested.clone()),
                ("fallback", self.language.fallback.clone()),
            ],
        )?;
        self.put(&root, "language", language)?;
        let row = field(&root, "trigger")?;
        let mut events = vec![
            self.definition.selection.clone(),
            self.definition.topology.clone(),
        ];
        for (ordinal, event) in sounds.trigger().events().iter().enumerate() {
            let values = self.definition.event(&self.uses, event)?;
            let address = index_path(&field(&row, "events")?, ordinal)?;
            let origin = self.group(
                &address,
                [
                    ("name", values.name),
                    ("id", values.id),
                    ("localised", values.localised),
                ],
            )?;
            self.fields.insert(address, origin.clone())?;
            events.push(origin);
        }
        let events = self.union(&events, true)?;
        let trigger = self.group(
            &row,
            [
                ("name", self.definition.name.clone()),
                ("definition", self.definition.path.clone()),
                ("events", events),
            ],
        )?;
        self.put(&root, "trigger", trigger)
    }

    fn path(
        &mut self,
        row: Option<&FieldPath>,
        length: usize,
        sources: &[HircPathSource],
        event: &FieldOrigin,
    ) -> Result<Route> {
        let path = row.map(|row| field(row, "path")).transpose()?;
        let mut end = None;
        let mut steps = vec![event.clone()];
        for position in 0..length {
            let origin = self.step(sources.get(position), (position == 0).then_some(event))?;
            let proof = match &path {
                Some(path) => self.put_step(&index_path(path, position)?, &origin)?,
                None => self.union([&origin.kind, &origin.id], true)?,
            };
            steps.push(proof);
            end = Some(origin);
        }
        let proof = self.union(&steps, length == sources.len())?;
        if let Some(path) = path {
            self.fields.insert(path, proof.clone())?;
        }
        Ok(Route { path: proof, end })
    }

    fn step(&self, source: Option<&HircPathSource>, event: Option<&FieldOrigin>) -> Result<Step> {
        let Some(source) = source else {
            let unknown = self.uses.origin(&[], false);
            return Ok(Step {
                kind: unknown.clone(),
                id: unknown,
            });
        };
        let site = source.origins(self.archive, &self.uses)?;
        let selected = self.union(std::iter::once(&site.inbound).chain(event), true)?;
        Ok(Step {
            kind: self.union([&selected, &site.kind], true)?,
            id: self.union([&selected, &site.id], true)?,
        })
    }

    fn put_step(&mut self, path: &FieldPath, origin: &Step) -> Result<FieldOrigin> {
        let joined = self.group(
            path,
            [("kind", origin.kind.clone()), ("id", origin.id.clone())],
        )?;
        self.fields.insert(path.clone(), joined.clone())?;
        Ok(joined)
    }

    fn broken_link(
        &mut self,
        row: &FieldPath,
        link: &BrokenLink<'_>,
        searched: Option<&FieldOrigin>,
    ) -> Result<BreakOrigins> {
        let event = self.definition.event(&self.uses, link.event())?;
        let route = self.path(Some(row), link.path().len(), link.path_sources(), &event.id)?;
        let reason = self.observed_break(link, &route, &event.id, searched)?;
        let object = if let Some(end) = route.end {
            self.put_step(&field(row, "object")?, &end)?
        } else {
            self.put(row, "object", reason.clone())?;
            reason.clone()
        };
        let bank = if link.soundbank().is_some() {
            self.bank(
                &field(row, "soundbank")?,
                link.entry(self.archive),
                &route.path,
            )?
        } else {
            self.put(row, "soundbank", reason.clone())?;
            reason.clone()
        };

        let trailing = self.group(
            row,
            [
                ("event", event.name),
                ("target", reason.clone()),
                ("kind", reason.clone()),
                ("message", reason.clone()),
            ],
        )?;
        let origin = self.union([&trailing, &object, &bank, &route.path], true)?;
        self.fields.insert(row.clone(), origin.clone())?;
        Ok(BreakOrigins {
            row: origin,
            reason,
        })
    }

    fn observed_break(
        &self,
        link: &BrokenLink<'_>,
        route: &Route,
        event: &FieldOrigin,
        searched: Option<&FieldOrigin>,
    ) -> Result<FieldOrigin> {
        let edge = link.failed_edge_origin(self.archive, &self.uses)?;
        self.break_reason(link.reason(), &route.path, event, searched, &edge)
    }

    fn break_reason(
        &self,
        reason: &BreakReason,
        route: &FieldOrigin,
        event: &FieldOrigin,
        searched: Option<&FieldOrigin>,
        edge: &FieldOrigin,
    ) -> Result<FieldOrigin> {
        match reason {
            BreakReason::NoSoundbank => {
                self.union(std::iter::once(event).chain(searched), searched.is_some())
            }
            BreakReason::Missing { .. }
            | BreakReason::OtherSoundbank { .. }
            | BreakReason::Cycle { .. } => self.union([route, edge], true),
            BreakReason::NoLocalisation { .. } => self.union(
                [route, &self.language.requested, &self.language.fallback],
                false,
            ),
            BreakReason::Unreadable(_) => self.union([route, edge], false),
            BreakReason::Malformed(_)
            | BreakReason::UnknownLayout { .. }
            | BreakReason::TooManyObjects { .. } => self.union([route], false),
            _ => Ok(route.clone()),
        }
    }

    fn bank(
        &mut self,
        row: &FieldPath,
        entry: Option<p4kx_extraction::ObservedEntry<'_>>,
        selected: &FieldOrigin,
    ) -> Result<FieldOrigin> {
        use p4kx_extraction::SourceUseRole;

        let path = source_fields::directory(&self.uses, entry, SourceUseRole::Payload)?;
        let language = source_fields::directory(&self.uses, entry, SourceUseRole::Conversion)?;
        let bank = self.group(
            row,
            [
                ("path", self.union([selected, &path], true)?),
                ("language", self.union([selected, &language], true)?),
            ],
        )?;
        self.fields.insert(row.clone(), bank.clone())?;
        Ok(bank)
    }

    fn issues(&mut self, index: &AudioIndex) -> Result<()> {
        let root = FieldPath::root();
        let base = field(&root, "issues")?;
        let mut origins = Vec::new();
        for (ordinal, issue) in index.issues().iter().enumerate() {
            let (path, diagnostic) = source_fields::issue(&self.uses, issue.source(self.archive))?;
            let address = index_path(&base, ordinal)?;
            let origin = self.group(
                &address,
                [
                    ("path", path),
                    ("kind", diagnostic.clone()),
                    ("message", diagnostic),
                ],
            )?;
            self.fields.insert(address, origin.clone())?;
            origins.push(origin);
        }
        self.fields.insert(base, self.union(&origins, true)?)?;
        Ok(())
    }

    fn union<'origin>(
        &self,
        origins: impl IntoIterator<Item = &'origin FieldOrigin>,
        complete: bool,
    ) -> Result<FieldOrigin> {
        Ok(self.uses.origin_with_origins(origins, complete)?)
    }

    fn put(&mut self, row: &FieldPath, name: &str, origin: FieldOrigin) -> Result<()> {
        Ok(self.fields.insert(field(row, name)?, origin)?)
    }

    fn group<const N: usize>(
        &mut self,
        row: &FieldPath,
        fields: [(&str, FieldOrigin); N],
    ) -> Result<FieldOrigin> {
        let result = self.union(fields.iter().map(|(_, origin)| origin), true)?;
        for (name, origin) in fields {
            self.put(row, name, origin)?;
        }
        Ok(result)
    }
}

fn index_path(base: &FieldPath, index: usize) -> Result<FieldPath> {
    let mut path = base.clone();
    path.push_index(index)?;
    Ok(path)
}

fn field(base: &FieldPath, name: &str) -> Result<FieldPath> {
    let mut path = base.clone();
    path.push_field(name)?;
    Ok(path)
}
