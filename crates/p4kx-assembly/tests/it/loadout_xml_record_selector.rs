// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    Entities, LoadoutPathOperation, LoadoutReadEvent, LoadoutReadObserver, LoadoutRequest,
    LoadoutResolver, LoadoutSelection, LoadoutXmlRecordSelector,
};
use p4kx_dataforge::{DataForge, Record, Value};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestValue};

use crate::loadout_original_controls_support as original;

#[derive(Default)]
struct State {
    selected: Vec<Selector>,
    callers: Vec<LoadoutSelection>,
}

struct Selector {
    selection: LoadoutSelection,
    record: u32,
    ordinal: usize,
}

struct Probe<'source> {
    forge: &'source DataForge<'source>,
    foreign: &'source DataForge<'source>,
    state: Mutex<State>,
}

impl LoadoutReadObserver for Probe<'_> {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let (LoadoutReadEvent::FileRead { caller, .. }
        | LoadoutReadEvent::FileReused { caller, .. }) = event
        else {
            return ControlFlow::Continue(());
        };
        let LoadoutRequest::Xml { path } = caller else {
            return ControlFlow::Break(());
        };
        let (Some(selection), Ok(mut state)) = (path.selection(), self.state.lock()) else {
            return ControlFlow::Break(());
        };
        state.callers.push(selection.clone());
        ControlFlow::Continue(())
    }

    fn observes_xml_record_selectors(&self) -> bool {
        true
    }

    fn xml_record_selector(&self, site: &LoadoutXmlRecordSelector<'_, '_>) -> ControlFlow<()> {
        assert_eq!(
            site.selection().operation(),
            LoadoutPathOperation::Copy,
            "the exact selector supplies the whole XML path"
        );
        assert!(
            site.text_location(self.forge).is_some(),
            "the actual text bytes belong to the original Record owner"
        );
        assert_eq!(
            site.text_location(self.foreign),
            None,
            "a byte-identical DataForge cannot mint evidence for another source"
        );
        let Ok(mut state) = self.state.lock() else {
            return ControlFlow::Break(());
        };
        state.selected.push(Selector {
            selection: site.selection().clone(),
            record: site.record().index(),
            ordinal: site.ordinal(),
        });
        ControlFlow::Continue(())
    }
}

fn database() -> Vec<u8> {
    let path = original::CONFIGURED;
    let decoy = TestValue::Struct(
        TestInstance::new("SEntityComponentDefaultLoadoutParams").value(
            "loadout",
            TestValue::Struct(
                TestInstance::new("SItemPortLoadoutXMLParams")
                    .value("loadoutPath", TestValue::text(path)),
            ),
        ),
    );
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "First", "synthetic/first.xml")
                .loadout(TestLoadout::Xml(path.to_owned()))
                .component(decoy)
                .record(),
        )
        .record(
            TestEntity::new(2, "Second", "synthetic/second.xml")
                .loadout(TestLoadout::Xml(path.to_owned()))
                .record(),
        )
        .build()
}

fn matching_ordinals(record: &Record<'_>) -> Result<Vec<usize>, p4kx_dataforge::DataForgeError> {
    let tree = record.tree()?;
    let mut matching = Vec::new();
    tree.root().visit_owned_values(|visit| {
        if visit.field_name() == "loadoutPath"
            && let Value::String(text) = visit.value()
            && *text == original::CONFIGURED
        {
            matching.push(visit.ordinal());
        }
    });
    Ok(matching)
}

#[test]
fn original_xml_root_capture_distinguishes_equal_fields_and_cached_callers() -> original::TestResult
{
    let bytes = database();
    let foreign_bytes = bytes.clone();
    let forge = DataForge::open(&bytes)?;
    let foreign = DataForge::open(&foreign_bytes)?;
    let [first, second] = original::callers(&forge)?;
    let ordinals = matching_ordinals(&first)?;
    let [winner, other] = ordinals.as_slice() else {
        return Err("the fixture must have two same-type fields with identical text".into());
    };
    let second_ordinals = matching_ordinals(&second)?;
    let [expected] = second_ordinals.as_slice() else {
        return Err("the second Record must declare exactly one XML path".into());
    };
    let files = original::GenericFiles::default();
    let probe = Arc::new(Probe {
        forge: &forge,
        foreign: &foreign,
        state: Mutex::default(),
    });
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    resolver.resolve(&first)?;
    resolver.resolve(&second)?;
    verify_capture(&probe, *winner, *other, *expected)?;
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "both explicit original callers share the one cached file read"
    );
    Ok(())
}

fn verify_capture(
    probe: &Probe<'_>,
    winner: usize,
    other: usize,
    expected: usize,
) -> original::TestResult {
    let state = std::mem::take(
        &mut *probe
            .state
            .lock()
            .map_err(|_| "selector mutex is poisoned")?,
    );
    let [first_site, second_site] = state.selected.as_slice() else {
        return Err("two active getters must yield two original selectors".into());
    };
    let [first_caller, second_caller] = state.callers.as_slice() else {
        return Err("the first read and the cache reuse must each retain their caller".into());
    };
    assert_eq!(
        first_site.record, 0,
        "the first actual caller keeps its physical Record"
    );
    assert_eq!(
        first_site.ordinal, winner,
        "the accepted first component keeps its exact ordinal"
    );
    assert_ne!(
        first_site.ordinal, other,
        "an identical losing component is never recorded as a selector"
    );
    assert_eq!(
        (second_site.record, second_site.ordinal),
        (1, expected),
        "cache reuse retains the exact second original Record and Value"
    );
    assert_eq!(
        &first_site.selection, first_caller,
        "the first read links to its getter"
    );
    assert_eq!(
        &second_site.selection, second_caller,
        "the reused read links to its caller's getter"
    );
    assert_ne!(
        first_caller, second_caller,
        "identical paths preserve separate authorship"
    );
    Ok(())
}
