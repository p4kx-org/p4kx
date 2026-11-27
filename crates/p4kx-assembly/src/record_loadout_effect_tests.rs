// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use p4kx_dataforge::{DataForge, Record, RecordTree, StructValue, Value};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort, entity_schema,
};

use crate::{
    Entities, LoadoutError, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver, LoadoutSelection, RecordLoadoutEffect, RecordPortSelection, RecordPortTarget,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const GRID: u128 = 1;
const INERT: u128 = 2;
const CARRIER: u128 = 3;
const DOCKED: u128 = 4;
const HOST_RECORD: u32 = 4;
const CONTROL_RECORD: u32 = 5;

#[derive(Default)]
struct Probe {
    enabled: bool,
    refuse: bool,
    names: Mutex<HashMap<LoadoutSelection, Port>>,
    effects: Mutex<Vec<Result<Effect, String>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Port {
    record: u32,
    name: String,
    empty: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Override {
        owner: u32,
        ordinal: usize,
        displaced: Port,
        winner: Port,
    },
    SuppressDefault {
        owner: u32,
        declared: Port,
        winner: Port,
    },
}

impl LoadoutReadObserver for Probe {
    fn observes_record_loadout_effects(&self) -> bool {
        self.enabled
    }

    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        _tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        parse(Some(&RecordFrame {
            probe: self,
            record: record.index(),
        }))
    }

    fn record_loadout_effect(&self, effect: &RecordLoadoutEffect<'_>) -> ControlFlow<()> {
        lock(&self.effects).push(self.effect(effect).map_err(|error| error.to_string()));
        if self.refuse {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl Probe {
    fn effect(&self, effect: &RecordLoadoutEffect<'_>) -> TestResult<Effect> {
        match effect {
            RecordLoadoutEffect::Override {
                owner_record_index,
                ordinal,
                displaced,
                winner,
            } => Ok(Effect::Override {
                owner: *owner_record_index,
                ordinal: *ordinal,
                displaced: self.port(displaced)?,
                winner: self.port(winner)?,
            }),
            RecordLoadoutEffect::SuppressDefault {
                owner_record_index,
                declared,
                winner,
            } => {
                assert!(
                    matches!(declared.target(), RecordPortTarget::Reference(_)),
                    "a declared item was masked"
                );
                assert!(
                    matches!(winner.target(), RecordPortTarget::Empty),
                    "the actual selected entry is empty"
                );
                Ok(Effect::SuppressDefault {
                    owner: *owner_record_index,
                    declared: self.port(declared)?,
                    winner: self.port(winner)?,
                })
            }
        }
    }

    fn port(&self, port: &RecordPortSelection<'_>) -> TestResult<Port> {
        let selection = port
            .name_proof()
            .and_then(crate::LoadoutPath::selection)
            .ok_or("the selected original port name")?;
        let chosen = lock(&self.names)
            .get(selection)
            .cloned()
            .ok_or("the same-frame original name")?;
        assert_eq!(
            chosen.name,
            port.name(),
            "the exact spelling used by the merge"
        );
        Ok(chosen)
    }
}

struct RecordFrame<'read> {
    probe: &'read Probe,
    record: u32,
}

impl LoadoutPathObserver for RecordFrame<'_> {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if selection.operation() == crate::LoadoutPathOperation::PortName {
            let field = if parent.type_name() == "SItemPortDef" {
                "Name"
            } else {
                "itemPortName"
            };
            assert!(
                parent
                    .field(field)
                    .is_some_and(|original| std::ptr::addr_eq(original, value)),
                "the name is the original raw field of this struct"
            );
            lock(&self.probe.names).insert(
                selection.clone(),
                Port {
                    record: self.record,
                    name: crate::read::value_text(value).to_owned(),
                    empty: false,
                },
            );
        }
        ControlFlow::Continue(())
    }

    fn record_empty_loadout_entry(
        &self,
        selection: &LoadoutSelection,
        entry: &StructValue<'_>,
    ) -> ControlFlow<()> {
        assert!(
            matches!(
                entry.field("entityClassReference"),
                Some(Value::Null) | None
            ),
            "the original empty entry did not provide a reference"
        );
        assert_eq!(
            crate::read::text(entry, "entityClassName"),
            "",
            "the consulted class is empty"
        );
        if let Some(port) = lock(&self.probe.names).get_mut(selection) {
            assert_eq!(
                port.record, self.record,
                "the proved empty decision belongs to the original Record"
            );
            assert_eq!(
                port.name,
                crate::read::text(entry, "itemPortName"),
                "the same authored occurrence"
            );
            port.empty = true;
        }
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

#[test]
fn direct_masking_observes_only_the_used_record_override_and_empty_default_shadow() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let probe = Arc::new(Probe {
        enabled: true,
        ..Probe::default()
    });
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&probe),
    )
    .stopping_at_docked_vehicles();
    let masked = forge.record(HOST_RECORD).ok_or("masked ship")?;
    resolver.prefetch(masked);
    assert!(
        lock(&probe.effects).is_empty(),
        "prefetch does not make any use decision"
    );

    let node = resolver.resolve(&masked)?;
    assert_eq!(
        cargo_mounts(&node),
        0,
        "the two actual overrides and the dock boundary leave no mounts"
    );
    let effects = lock(&probe.effects);
    let [override_, default] = effects.as_slice() else {
        return Err("one inline overwrite and one declared default suppression".into());
    };
    assert_eq!(
        override_.as_ref().map_err(String::as_str)?,
        &Effect::Override {
            owner: 2,
            ordinal: 0,
            displaced: port(2, "pad", false),
            winner: port(HOST_RECORD, "PAD", false),
        },
        "the chosen Record override retains both original owners and spellings"
    );
    assert_eq!(
        default.as_ref().map_err(String::as_str)?,
        &Effect::SuppressDefault {
            owner: HOST_RECORD,
            declared: port(HOST_RECORD, "Suppressed", false),
            winner: port(HOST_RECORD, "SUPPRESSED", true),
        },
        "the selected empty entry supplants the original declared default"
    );
    drop(effects);

    let control = resolver.resolve(&forge.record(CONTROL_RECORD).ok_or("control ship")?)?;
    assert_eq!(
        cargo_mounts(&control),
        2,
        "both original plain choices contribute without masking"
    );
    assert_eq!(
        lock(&probe.effects).len(),
        2,
        "docked or unmasked ports introduce no effect receipt"
    );
    Ok(())
}

#[test]
fn direct_effect_observation_is_opt_in_and_can_stop_resolution() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let masked = forge.record(HOST_RECORD).ok_or("masked ship")?;
    let disabled = Arc::new(Probe::default());
    let mut resolver = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&disabled),
    )
    .stopping_at_docked_vehicles();
    assert_eq!(
        cargo_mounts(&resolver.resolve(&masked)?),
        0,
        "the canonical result is identical"
    );
    assert!(
        lock(&disabled.effects).is_empty(),
        "uninterested observers receive no effect"
    );
    assert!(
        lock(&disabled.names).is_empty(),
        "the optional port selectors were not captured"
    );

    let refusal = Arc::new(Probe {
        enabled: true,
        refuse: true,
        ..Probe::default()
    });
    let mut stopped = LoadoutResolver::with_observed_entities(
        Arc::new(Entities::new(&forge)),
        Arc::<Probe>::clone(&refusal),
    )
    .stopping_at_docked_vehicles();
    assert!(
        matches!(
            stopped.resolve(&masked),
            Err(LoadoutError::ObservationStopped)
        ),
        "refusing evidence prevents publishing the selected result"
    );
    assert_eq!(
        lock(&refusal.effects).len(),
        1,
        "resolution stops before a second effect is admitted"
    );
    Ok(())
}

fn port(record: u32, name: &str, empty: bool) -> Port {
    Port {
        record,
        name: name.to_owned(),
        empty,
    }
}

fn cargo_mounts(node: &crate::EntityNode) -> usize {
    node.loadout()
        .iter()
        .filter_map(crate::LoadoutEntry::entity)
        .filter(|child| child.attach_type() == "CargoGrid")
        .count()
}

fn database() -> Vec<u8> {
    fn entity(id: u128, class: &str) -> TestEntity {
        TestEntity::new(id, class, format!("synthetic/{class}.xml"))
    }

    let carrier = || TestLoadoutEntry::reference("bay", CARRIER);
    let docked = || TestLoadoutEntry::reference("docked", DOCKED);
    let host = |id, class| entity(id, class).port(TestPort::new("Suppressed").default_item(GRID));
    entity_schema(DataForgeBuilder::new().version(8))
        .record(entity(GRID, "Grid").attach_type("CargoGrid").record())
        .record(entity(INERT, "Inert").attach_type("WeaponGun").record())
        .record(
            entity(CARRIER, "Carrier")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                    "pad", GRID,
                )]))
                .record(),
        )
        .record(
            entity(DOCKED, "Docked")
                .attach_type("NOITEM_Vehicle")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                    "dock_grid",
                    GRID,
                )]))
                .record(),
        )
        .record(
            host(5, "Masked")
                .loadout(TestLoadout::Manual(vec![
                    TestLoadoutEntry::empty("SUPPRESSED"),
                    carrier().loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                        "PAD", INERT,
                    )])),
                    docked(),
                ]))
                .record(),
        )
        .record(
            host(6, "Control")
                .loadout(TestLoadout::Manual(vec![carrier(), docked()]))
                .record(),
        )
        .build()
}
