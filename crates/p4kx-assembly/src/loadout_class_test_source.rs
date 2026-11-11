// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::ptr;
use std::sync::Mutex;

use p4kx_dataforge::{DataForge, Guid, Record, RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry};

use crate::{
    LoadoutClassSelection, LoadoutFileObservation, LoadoutPathObserver, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutSelection, LoadoutXmlDecoding, XmlLoadoutClassSite,
};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn database() -> Vec<u8> {
    let own = |path| {
        TestEntity::new(1, "Self", path)
            .loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("one", "Self"),
                TestLoadoutEntry::class("two", "Self"),
            ]))
            .record()
    };
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(own("synthetic/first.xml"))
        .record(own("synthetic/second.xml"))
        .build()
}

pub(super) fn entries<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Vec<&'tree StructValue<'source>> {
    let mut result = Vec::new();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return result;
    };
    for value in components {
        let Value::Struct(component) = value else {
            continue;
        };
        let Some(Value::Struct(loadout)) = component.field("loadout") else {
            continue;
        };
        let Some(Value::Array(entries)) = loadout.field("entries") else {
            continue;
        };
        result.extend(entries.iter().filter_map(|value| match value {
            Value::Struct(entry) => Some(entry),
            _ => None,
        }));
    }
    result
}

pub(super) fn original(tree: &RecordTree<'_>) {
    assert_eq!(
        tree.error_count(),
        0,
        "the actual canonical source is fully readable"
    );
    let entries = entries(tree);
    assert_eq!(
        entries.len(),
        2,
        "both original class-name slots are retained before projection"
    );
    for entry in entries {
        assert_eq!(
            entry.type_name(),
            "SItemPortLoadoutEntryParams",
            "the actual source parent"
        );
        assert_eq!(
            entry.field("entityClassName"),
            Some(&Value::String("Self")),
            "the actual original String"
        );
        assert_eq!(
            entry.field("entityClassReference"),
            Some(&Value::Null),
            "the actual Reference does not override the class fallback"
        );
    }
}

#[derive(Clone, Copy, Default)]
pub(super) enum Stop {
    #[default]
    Never,
    Getter,
    Chosen,
    Xml,
}

pub(super) struct Getter {
    pub(super) selection: LoadoutClassSelection,
    pub(super) record_index: u32,
}
pub(super) struct Chosen {
    pub(super) selection: LoadoutClassSelection,
    pub(super) record_index: u32,
    pub(super) guid: Guid,
}
pub(super) struct XmlGetter {
    pub(super) selection: LoadoutClassSelection,
    pub(super) node_index: u32,
    pub(super) attribute_index: u64,
    pub(super) decoding: LoadoutXmlDecoding,
    pub(super) operation: LoadoutFileObservation,
}
#[derive(Default)]
pub(super) struct Log {
    pub(super) getters: Vec<Getter>,
    pub(super) chosen: Vec<Chosen>,
    pub(super) xml: Vec<XmlGetter>,
}

pub(super) struct Recorder<'forge> {
    pub(super) forge: &'forge DataForge<'forge>,
    pub(super) stop: Stop,
    pub(super) log: Mutex<Log>,
}

impl<'forge> Recorder<'forge> {
    pub(super) const fn new(forge: &'forge DataForge<'forge>, stop: Stop) -> Self {
        Self {
            forge,
            stop,
            log: Mutex::new(Log {
                getters: Vec::new(),
                chosen: Vec::new(),
                xml: Vec::new(),
            }),
        }
    }
}

struct Borrowed<'tree, 'source, 'forge> {
    tree: &'tree RecordTree<'source>,
    recorder: &'tree Recorder<'forge>,
}

impl LoadoutPathObserver for Borrowed<'_, '_, '_> {
    fn record_path(
        &self,
        _: &LoadoutSelection,
        _: &StructValue<'_>,
        _: &Value<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
    fn record_name(&self, _: &LoadoutSelection, _: &RecordTree<'_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
    fn record_class(
        &self,
        selection: &LoadoutClassSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        assert!(
            entries(self.tree)
                .iter()
                .any(|entry| ptr::addr_eq(*entry, parent)),
            "the callback's actual parent belongs to this borrowed tree"
        );
        assert!(
            parent
                .field("entityClassName")
                .is_some_and(|original| ptr::addr_eq(original, value)),
            "the actual selected class field is borrowed before copy"
        );
        assert_eq!(
            value,
            &Value::String("Self"),
            "the real accepted source value"
        );
        self.recorder
            .log
            .lock()
            .expect("source log")
            .getters
            .push(Getter {
                selection: selection.clone(),
                record_index: self.tree.record_index(),
            });
        if matches!(self.recorder.stop, Stop::Getter) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl LoadoutReadObserver for Recorder<'_> {
    fn observe(&self, _: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        assert!(
            record.is_from(self.forge) && tree.is_from(record),
            "the scoped source has actual Forge/Record ownership"
        );
        parse(Some(&Borrowed {
            tree,
            recorder: self,
        }))
    }
    fn class_record(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> ControlFlow<()> {
        assert!(
            record.is_from(self.forge),
            "the chosen Record is borrowed from the actual Forge before its GUID copy"
        );
        assert_eq!(
            record.index(),
            0,
            "first-definition class selection retains its actual physical index"
        );
        assert_eq!(
            record.name().ok(),
            Some("EntityClassDefinition.Self"),
            "the selected class header is independently checked"
        );
        self.log.lock().expect("source log").chosen.push(Chosen {
            selection: selection.clone(),
            record_index: record.index(),
            guid: record.guid(),
        });
        if matches!(self.stop, Stop::Chosen) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        assert_eq!(
            site.attribute_name(),
            "itemName",
            "the actual winning class attribute"
        );
        assert_eq!(
            site.value(),
            "Self",
            "the original class text precedes its copy"
        );
        assert!(
            site.observation().read_outcome().is_ok(),
            "the callback belongs to the genuine current file operation"
        );
        self.log.lock().expect("source log").xml.push(XmlGetter {
            selection: selection.clone(),
            node_index: site.node_index(),
            attribute_index: site.attribute_index(),
            decoding: site.decoding(),
            operation: site.observation().clone(),
        });
        if matches!(self.stop, Stop::Xml) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}
