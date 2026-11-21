// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};
use p4kx_media::{
    ContainerIdentity, ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status,
};

use super::shader_accounting::require;
use super::{TestResult, manifests, privacy};

#[path = "animation_event_source_fields.rs"]
mod source_fields;

use self::source_fields::ExpectedEvents;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn animation_event_timelines_reconcile_against_verified_archive_sources()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    verify_archive(&p4kx_testkit::real_archive_path()?)?;
    if let Some(second) = p4kx_testkit::real_archive_b_path() {
        verify_archive(&second)?;
    }
    Ok(())
}

fn verify_archive(path: &Path) -> TestResult {
    let key = p4kx_testkit::real_archive_key()?;
    verify_selected(&Archive::open_with_key(path, Some(key))?)
}

fn verify_selected(archive: &Archive) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let output = tempfile::tempdir()?;
    let options = ExportOptions {
        include: Some(Filter::parse("*.animevents")?),
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, output.path(), &options)?
    else {
        return Err("installed animation events did not produce a complete generation".into());
    };
    let stream = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::AnimationDefinition)
        .ok_or("animation definition stream missing")?;
    let base = output.path().join(&index.artifact_base);
    let mut rows = BufReader::new(File::open(base.join(&stream.artifact.path))?)
        .lines()
        .map(|line| Ok(serde_json::from_str::<MediaRecord>(&line?)?))
        .peekable();
    let mut census = Census {
        rows: &mut rows,
        entries: 0,
        observed_event: false,
        watch: &watch,
        base: &base,
    };
    census.walk(archive, &mut Vec::new())?;
    require(
        census.entries == index.source_entries.written,
        "complete source tally",
    )?;
    require(
        census.observed_event && rows.next().is_none(),
        "complete positive event stream",
    )?;
    watch.check()?;
    output.close()?;
    Ok(())
}

struct Census<'a, R: Iterator<Item = TestResult<MediaRecord>>> {
    rows: &'a mut std::iter::Peekable<R>,
    entries: u64,
    observed_event: bool,
    watch: &'a ArchiveWatch,
    base: &'a Path,
}

impl<R: Iterator<Item = TestResult<MediaRecord>>> Census<'_, R> {
    fn walk(&mut self, archive: &Archive, containers: &mut Vec<ContainerIdentity>) -> TestResult {
        for entry in archive.entries() {
            self.watch.poll()?;
            let Some((_, ext)) = entry.path().rsplit_once('.') else {
                continue;
            };
            if ext.eq_ignore_ascii_case("animevents") {
                self.entry(archive, entry, containers)?;
            } else if ext.eq_ignore_ascii_case("pak") || ext.eq_ignore_ascii_case("socpak") {
                let identity = ContainerIdentity::try_new_with_entry(archive, entry)?;
                let nested = archive.open_container(entry)?;
                containers.push(identity);
                self.walk(&nested, containers)?;
                let _parent = containers.pop();
            }
        }
        Ok(())
    }

    fn entry(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        containers: &[ContainerIdentity],
    ) -> TestResult {
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let document = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        let expected = original(document.root());
        let mut fields = ExpectedEvents::new(document.root())?;
        let mut observed = Tallies::default();
        let entry = ContainerIdentity::try_new_with_entry(archive, entry)?;
        while self.rows.peek().is_some_and(|next| {
            next.as_ref().map_or(true, |row| matches!(&row.source.identity,
                Identity::Entry { entry: actual, containers: chain } if actual == &entry && chain == containers))
        }) {
            let row = self.rows.next().ok_or("peeked row disappeared")??;
            require(row.status == Status::Complete, "installed event source projected completely")?;
            match row.subresource.namespace.as_str() {
                "animation-definition-document" => {
                    verify_raw(self.base, &row, &bytes)?;
                    observed.documents += 1;
                }
                "animation-definition-event-timeline" => {
                    fields.timeline(&row)?;
                    observed.timelines += 1;
                }
                "animation-definition-event" => {
                    fields.event(&row)?;
                    observed.events += 1;
                }
                "animation-definition-event-attribute" => {
                    fields.attribute(&row)?;
                    observed.attributes += 1;
                }
                "animation-definition-declaration"
                    if row.data.pointer("/payload/kind/type").and_then(serde_json::Value::as_str)
                        == Some("attribute") =>
                {
                    observed.declarations += 1;
                }
                _ => {}
            }
        }
        require(
            expected == observed && fields.is_empty(),
            "event and declaration tallies match each physical Entry",
        )?;
        self.observed_event |= observed.events > 0;
        self.entries += 1;
        Ok(())
    }
}

#[derive(Default, PartialEq, Eq)]
struct Tallies {
    documents: usize,
    timelines: usize,
    events: usize,
    attributes: usize,
    declarations: usize,
}

fn original(root: SourceElement<'_>) -> Tallies {
    let mut counts = Tallies {
        documents: 1,
        ..Tallies::default()
    };
    if named(root, "anim_event_list") {
        for timeline in root
            .elements()
            .filter(|element| named(*element, "animation"))
        {
            counts.timelines += 1;
            for event in timeline
                .elements()
                .filter(|element| named(*element, "event"))
            {
                counts.events += 1;
                counts.attributes += event.attributes().count();
            }
        }
    }
    let mut elements = vec![root];
    while let Some(element) = elements.pop() {
        counts.declarations += element.attributes().count();
        elements.extend(element.elements());
    }
    counts
}

pub(super) fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}
fn verify_raw(base: &Path, row: &MediaRecord, original: &[u8]) -> TestResult {
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("original animation event artifact")?;
    let retained = manifests::check_payload(
        base,
        &raw.path,
        raw.size,
        &raw.digest,
        original.len() as u64,
    )?;
    require(
        retained == original,
        "retained animation event bytes equal the verified source",
    )
}

#[test]
fn nested_event_replay_checks_original_values_lexemes_positions_and_receipts() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

    let folder = tempfile::tempdir()?;
    let input = folder.path().join("animations.p4k");
    let text = br#"<anim_event_list><animation name="Animations/A.caf">
        <event xmlns:x="urn:test" time="" parameter="a&amp;b" x:unknown="nine"/>
        <event time="3.5" name="repeat"/>
    </animation></anim_event_list>"#;
    let binary = TestElement::new("anim_event_list")
        .child(
            TestElement::new("animation")
                .attribute("name", "Animations/B.caf")
                .child(
                    TestElement::new("event")
                        .attribute("time", "0.25")
                        .attribute("time", "0.75"),
                ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let nested = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/fixture.animevents", binary))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/fixture.animevents", text))
        .entry(TestEntry::stored("Data/fixture.pak", nested))
        .write_to(&input)?;
    verify_selected(&Archive::open(input)?)
}
