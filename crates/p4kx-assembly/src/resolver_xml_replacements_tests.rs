// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use p4kx_dataforge::{DataForge, Record, RecordTree, StructValue, Value};

use crate::{
    Entities, LoadoutError, LoadoutFileError, LoadoutFileObservation, LoadoutFileOperationKey,
    LoadoutFiles, LoadoutPath, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver, LoadoutSelection, XmlLoadoutPathSite, XmlPortReplacement, XmlPortSelection,
};

#[path = "resolver_xml_replacements_fixture.rs"]
mod fixture;
use fixture::database;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

const ROOT: &str = "synthetic/membership-root.xml";
const BASE: &str = "synthetic/membership-base.xml";
const OVERRIDE: &str = "synthetic/membership-override.xml";
const UNUSED: &str = "synthetic/prefetched-only.xml";
const ROOT_XML: &str = r#"<Loadout><Loadouts><File loadout="synthetic/membership-base.xml"/><File loadout="synthetic/membership-override.xml"/></Loadouts><Items><Item portName="Other" itemName="Inert"/></Items></Loadout>"#;
const BASE_XML: &str = r#"<Loadout><Items><Item portName="PAD" itemName="Cargo"/><Item portName="other" itemName="Cargo"/></Items></Loadout>"#;
const OVERRIDE_XML: &str =
    r#"<Loadout><Items><Item portName="Pad" itemName="Inert"/></Items></Loadout>"#;
const UNUSED_XML: &str = r#"<Loadout><Loadouts><File loadout="synthetic/membership-base.xml"/></Loadouts><Items><Item portName="Pad" itemName="Inert"/></Items></Loadout>"#;

#[derive(Default)]
struct Files(Mutex<Vec<String>>);

impl LoadoutFiles for Files {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        lock(&self.0).push(path.to_owned());
        let source = match path {
            ROOT => ROOT_XML,
            BASE => BASE_XML,
            OVERRIDE => OVERRIDE_XML,
            UNUSED => UNUSED_XML,
            _ => return Err(LoadoutFileError::NotFound),
        };
        Ok(source.as_bytes().to_vec())
    }
}

#[derive(Default)]
struct Probe {
    records: Mutex<Vec<u32>>,
    attributes: Mutex<Vec<Attribute>>,
    used: Mutex<Vec<Result<Used, String>>>,
    enabled: bool,
    stop_on_use: bool,
}

struct Attribute {
    selection: LoadoutSelection,
    operation: LoadoutFileOperationKey,
    name: String,
}

struct Used {
    root: LoadoutFileOperationKey,
    request: LoadoutSelection,
    replacements: Vec<(usize, String, String)>,
}

impl LoadoutReadObserver for Probe {
    fn observes_xml_port_replacements(&self) -> bool {
        self.enabled
    }

    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if let LoadoutReadEvent::CanonicalRead {
            record,
            outcome: Ok(_),
        } = event
        {
            lock(&self.records).push(record.index());
        }
        ControlFlow::Continue(())
    }

    fn with_record_paths(
        &self,
        _record: &Record<'_>,
        _tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        parse(Some(self))
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        assert!(
            site.location().is_some(),
            "the genuine XML attribute has a source coordinate"
        );
        lock(&self.attributes).push(Attribute {
            selection: selection.clone(),
            operation: site.observation().operation_key(),
            name: site.attribute_name().to_owned(),
        });
        ControlFlow::Continue(())
    }

    fn xml_port_replacements_used(
        &self,
        root: &LoadoutFileObservation,
        request: &LoadoutPath,
        replacements: &[XmlPortReplacement],
    ) -> ControlFlow<()> {
        let result = inspect_replacements(&lock(&self.attributes), root, request, replacements)
            .map_err(|error| error.to_string());
        lock(&self.used).push(result);
        if self.stop_on_use {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl LoadoutPathObserver for Probe {
    fn record_path(
        &self,
        _selection: &LoadoutSelection,
        _parent: &StructValue<'_>,
        _value: &Value<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn record_name(
        &self,
        _selection: &LoadoutSelection,
        _tree: &RecordTree<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

fn inspect_replacements(
    observations: &[Attribute],
    root: &LoadoutFileObservation,
    request: &LoadoutPath,
    replacements: &[XmlPortReplacement],
) -> TestResult<Used> {
    assert_eq!(request.value(), ROOT, "actual root request");
    let root_key = root.operation_key();
    let replacements = replacements
        .iter()
        .map(|replacement| {
            assert_eq!(
                replacement.document(),
                ROOT,
                "the merge is in the owning XML"
            );
            assert!(
                replacement.merge_document().is_same_operation(root),
                "the replacement names the actual merge operation"
            );
            assert_port(observations, root_key, replacement.displaced(), Some(BASE))?;
            let include = (replacement.ordinal() == 0).then_some(OVERRIDE);
            assert_port(observations, root_key, replacement.winner(), include)?;
            Ok((
                replacement.ordinal(),
                replacement.displaced().name().value().to_owned(),
                replacement.winner().name().value().to_owned(),
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    Ok(Used {
        root: root_key,
        request: request.selection().cloned().ok_or("physical DCB request")?,
        replacements,
    })
}

fn assert_port(
    observations: &[Attribute],
    root: LoadoutFileOperationKey,
    port: &XmlPortSelection,
    expected_include: Option<&str>,
) -> TestResult {
    let original = |path: &LoadoutPath, attribute: &str| -> TestResult<LoadoutFileOperationKey> {
        let selection = path.selection().ok_or("the original XML path selection")?;
        observations
            .iter()
            .find(|record| &record.selection == selection && record.name == attribute)
            .map(|record| record.operation)
            .ok_or_else(|| "the same-parse site".into())
    };
    let winning_operation = original(port.name(), "portName")?;
    let mut includes = port.includes();
    if let Some(expected) = expected_include {
        let (parent, child, selector) = includes.next().ok_or("the original Include edge")?;
        assert_eq!(selector.value(), expected, "actual Include attribute");
        assert!(
            parent.operation_key() == root,
            "the expected merge operation"
        );
        assert!(
            original(selector, "loadout")? == root,
            "the same parse supplied the Include selector"
        );
        assert!(
            child.operation_key() == winning_operation,
            "the Include targets this port's source"
        );
    } else {
        assert!(
            winning_operation == root,
            "a local port uses the root's original parse"
        );
    }
    assert!(includes.next().is_none(), "the fixture has direct Includes");
    Ok(())
}

#[test]
fn only_consumed_xml_replays_its_original_replacements_for_each_caller() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let sources = Files::default();
    let probe = Arc::new(Probe {
        enabled: true,
        ..Probe::default()
    });
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&probe),
    )
    .with_files(&sources);
    let first = forge.record(0).ok_or("first root")?;
    resolver.prefetch(first);
    assert!(
        lock(&probe.records).contains(&5),
        "the later displaced Entity has already been prefetched"
    );
    assert!(lock(&sources.0).is_empty(), "prefetch reads no XML");
    assert!(
        lock(&probe.used).is_empty(),
        "prefetch admits no replacement receipt"
    );

    for index in [0, 1] {
        let root = resolver.resolve(&forge.record(index).ok_or("physical root")?)?;
        let loadout = root.loadout();
        assert!(
            loadout
                .iter()
                .any(|entry| entry.path() == "branch/active/Pad"
                    && entry.class_name() == Some("Inert")),
            "the Include winner is attached"
        );
        assert!(
            loadout
                .iter()
                .any(|entry| entry.path() == "branch/active/Other"
                    && entry.class_name() == Some("Inert")),
            "the local document winner is attached"
        );
        assert!(
            loadout
                .iter()
                .all(|entry| entry.class_name() != Some("Cargo")),
            "no displaced target is attached"
        );
    }

    assert_eq!(
        &*lock(&sources.0),
        &[ROOT, BASE, OVERRIDE],
        "cache reuse opens no XML again"
    );
    check_used_comparisons(lock(&probe.used).as_slice())
}

fn check_used_comparisons(used: &[Result<Used, String>]) -> TestResult {
    let [first, second] = used else {
        return Err("one consumed receipt per physical caller".into());
    };
    let first = first.as_ref().map_err(String::as_str)?;
    let second = second.as_ref().map_err(String::as_str)?;
    assert_ne!(
        first.request, second.request,
        "cache reuse keeps the current original request"
    );
    assert!(
        first.root == second.root,
        "the physical XML operation is reused"
    );
    assert_eq!(
        first.replacements,
        [
            (0, "PAD".to_owned(), "Pad".to_owned()),
            (1, "other".to_owned(), "Other".to_owned())
        ],
        "the actual two replacements were retained in operation order"
    );
    assert_eq!(
        first.replacements, second.replacements,
        "exact physical winners on reuse"
    );
    Ok(())
}

#[test]
fn refusing_used_replacements_stops_the_current_resolution() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let sources = Files::default();
    let probe = Arc::new(Probe {
        enabled: true,
        stop_on_use: true,
        ..Probe::default()
    });
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&probe),
    )
    .with_files(&sources);
    let original = forge.record(0).ok_or("actual root")?;
    assert!(
        matches!(
            resolver.resolve(&original),
            Err(LoadoutError::ObservationStopped)
        ),
        "the observation refusal is fatal"
    );
    assert!(
        matches!(
            resolver.resolve(&original),
            Err(LoadoutError::ObservationStopped)
        ),
        "the stopped resolver refuses fresh calls"
    );
    assert_eq!(
        lock(&probe.used).len(),
        1,
        "a refused callback is never retried"
    );
    Ok(())
}

#[test]
fn observers_without_replacement_interest_keep_the_same_loadout() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let sources = Files::default();
    let probe = Arc::new(Probe::default());
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&probe),
    )
    .with_files(&sources);
    let original = forge.record(0).ok_or("actual root")?;
    let node = resolver.resolve(&original)?;
    assert!(
        node.loadout().iter().any(|entry|
            entry.path() == "branch/active/Pad" && entry.class_name() == Some("Inert")),
        "disabling evidence collection keeps the exact selected attachment"
    );
    assert!(
        lock(&probe.used).is_empty(),
        "the observer did not request receipts"
    );
    assert!(
        lock(&probe.attributes)
            .iter()
            .all(|fact| fact.name != "portName"),
        "replacement port-name evidence is opt-in"
    );
    Ok(())
}
