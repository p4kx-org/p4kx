// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, CanonicalReadObserver, DataForge, DataForgeError, DataType, Record, RecordTree,
    Struct, TreeBuilder,
};

use crate::catalog_queries::unlocalized_record_query;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::entity_selection::{self, Selected};
use crate::scan::{CRAFTER_FIELD, CRAFTER_STRUCT};
use crate::{CatalogError, Kind};

pub(in crate::build::catalog_queries) struct Owner<'source> {
    pub(in crate::build::catalog_queries) record: Record<'source>,
    pub(in crate::build::catalog_queries) tree: RecordTree<'source>,
}

pub(in crate::build::catalog_queries) struct Plan<'source> {
    pub(in crate::build::catalog_queries) owners: Vec<Owner<'source>>,
    pub(in crate::build::catalog_queries) coverage: CatalogRecordCoverage,
}

impl<'source> Plan<'source> {
    pub(in crate::build::catalog_queries) fn read(
        forge: &'source DataForge<'source>,
        kind: Kind,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        Self::read_with(forge, selection, cancel, observer, |record, _, coverage| {
            Ok(unlocalized_record_query::selected(record, kind, coverage))
        })
    }

    pub(in crate::build::catalog_queries) fn read_entities(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        Self::read_entities_matching(forge, selection, cancel, observer, |_, _| true)
    }

    pub(in crate::build::catalog_queries) fn read_entity_components(
        forge: &'source DataForge<'source>,
        component_types: &[&str],
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        let may_match = component_candidate(forge, component_types, cancel)?;
        Self::read_entities_matching(forge, selection, cancel, observer, may_match)
    }

    pub(in crate::build::catalog_queries) fn read_crafters(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let admits = crafter_admission(forge, cancel)?;
        Self::read_with(forge, selection, cancel, None, admits)
    }

    pub(in crate::build::catalog_queries) fn read_item_components(
        forge: &'source DataForge<'source>,
        component_types: &[&str],
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        let selected = item_selection(forge, Some(component_types), cancel)?;
        Self::read_with(forge, selection, cancel, observer, selected)
    }

    pub(in crate::build::catalog_queries) fn visit_items<E: From<CatalogError>>(
        forge: &'source DataForge<'source>,
        component_types: Option<&[&str]>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        visit: impl FnMut(Owner<'source>, &mut CatalogRecordCoverage) -> Result<(), E>,
    ) -> Result<CatalogRecordCoverage, E> {
        let selected = item_selection(forge, component_types, cancel)?;
        Self::visit_with(forge, selection, cancel, observer, selected, visit)
    }

    fn read_entities_matching(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        mut may_match: impl FnMut(&Record<'source>, &mut TreeBuilder<'source>) -> bool,
    ) -> Result<Self, CatalogError> {
        let crafter = forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT);
        Self::read_with(
            forge,
            selection,
            cancel,
            observer,
            |record, trees, coverage| {
                let classified = record.type_name().and_then(|source_type| {
                    if source_type == p4kx_assembly::ENTITY_TYPE {
                        entity_selection::select(record, None)
                    } else {
                        Ok(None)
                    }
                });
                match classified {
                    Err(_) => coverage.unreadable_headers.push(record.index()),
                    Ok(Some(Selected::Leftover(_))) => coverage.skipped_leftovers += 1,
                    Ok(Some(Selected::Entity(kind))) => {
                        if kind != Kind::Crafters {
                            return Ok(may_match(record, trees));
                        }
                        match crafter
                            .as_ref()
                            .map_or(Ok(false), |probe| trees.holds(probe, record))
                        {
                            Ok(holds) => return Ok(holds && may_match(record, trees)),
                            Err(_) => coverage.unreadable_headers.push(record.index()),
                        }
                    }
                    Ok(None) => {}
                }
                Ok(false)
            },
        )
    }

    pub(in crate::build::catalog_queries) fn read_type(
        forge: &'source DataForge<'source>,
        source_type: &str,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::read_with(forge, selection, cancel, None, |record, _, coverage| {
            let Ok(name) = record.type_name() else {
                coverage.unreadable_headers.push(record.index());
                return Ok(false);
            };
            Ok(name == source_type)
        })
    }

    pub(in crate::build::catalog_queries) fn read_rule(
        forge: &'source DataForge<'source>,
        rule: crate::selection::KindRule,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
    ) -> Result<Self, CatalogError> {
        Self::read_with(forge, selection, cancel, observer, |record, _, coverage| {
            let path = record.record_path();
            if path.as_ref().is_ok_and(|path| !rule.matches_path(path)) {
                return Ok(false);
            }
            match record.type_name() {
                Ok(source_type) if source_type != rule.type_name => Ok(false),
                Ok(_) if path.is_ok() => Ok(true),
                _ => {
                    coverage.unreadable_headers.push(record.index());
                    Ok(false)
                }
            }
        })
    }

    pub(in crate::build::catalog_queries) fn read_with(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        selected: impl FnMut(
            &Record<'source>,
            &mut TreeBuilder<'source>,
            &mut CatalogRecordCoverage,
        ) -> Result<bool, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let mut owners = Vec::new();
        let coverage = Self::visit_with::<CatalogError>(
            forge,
            selection,
            cancel,
            observer,
            selected,
            |owner, _| {
                owners.push(owner);
                Ok(())
            },
        )?;
        Ok(Self { owners, coverage })
    }

    fn visit_with<E: From<CatalogError>>(
        forge: &'source DataForge<'source>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        observer: Option<Arc<dyn CanonicalReadObserver>>,
        mut selected: impl FnMut(
            &Record<'source>,
            &mut TreeBuilder<'source>,
            &mut CatalogRecordCoverage,
        ) -> Result<bool, CatalogError>,
        mut visit: impl FnMut(Owner<'source>, &mut CatalogRecordCoverage) -> Result<(), E>,
    ) -> Result<CatalogRecordCoverage, E> {
        let mut coverage = CatalogRecordCoverage::default();
        let mut trees = TreeBuilder::new();
        trees.set_read_observer(observer);
        selection.try_for_each::<E>(forge, cancel, |record| {
            checkpoint(cancel)?;
            if !selected(record, &mut trees, &mut coverage)? {
                return Ok(());
            }
            checkpoint(cancel)?;
            if let Some(owner) = read_owner(&mut trees, record, &mut coverage)? {
                visit(owner, &mut coverage)?;
            }
            Ok(())
        })?;
        Ok(coverage)
    }
}

fn item_selection<'source>(
    forge: &'source DataForge<'source>,
    component_types: Option<&[&str]>,
    cancel: Option<&CancelToken>,
) -> Result<
    impl FnMut(
        &Record<'source>,
        &mut TreeBuilder<'source>,
        &mut CatalogRecordCoverage,
    ) -> Result<bool, CatalogError>
    + use<'source>,
    CatalogError,
> {
    let mut candidate = component_types
        .map(|types| component_candidate(forge, types, cancel))
        .transpose()?;
    Ok(
        move |record: &Record<'source>,
              trees: &mut TreeBuilder<'source>,
              coverage: &mut CatalogRecordCoverage| {
            Ok(
                unlocalized_record_query::selected(record, Kind::Items, coverage)
                    && candidate.as_mut().is_none_or(|probe| probe(record, trees)),
            )
        },
    )
}

const COMPONENTS: &str = "Components";

fn component_candidate<'source>(
    forge: &'source DataForge<'source>,
    component_types: &[&str],
    cancel: Option<&CancelToken>,
) -> Result<
    impl FnMut(&Record<'source>, &mut TreeBuilder<'source>) -> bool + use<'source>,
    CatalogError,
> {
    checkpoint(cancel)?;
    let probes: Vec<_> = component_types
        .iter()
        .filter_map(|name| forge.strict_pointer_probe(COMPONENTS, name))
        .collect();
    let strong_arrays: HashSet<_> = forge
        .structs()
        .filter(|structure| {
            structure
                .name()
                .is_ok_and(|name| name == p4kx_assembly::ENTITY_TYPE)
                && strong_component_array(forge, *structure)
        })
        .map(|structure| structure.index())
        .collect();
    Ok(
        move |record: &Record<'source>, trees: &mut TreeBuilder<'source>| {
            // Only a proven negative can exclude a Record before the canonical body read.
            !record
                .struct_type()
                .is_ok_and(|structure| strong_arrays.contains(&structure.index()))
                || probes
                    .iter()
                    .any(|probe| !matches!(trees.holds(probe, record), Ok(false)))
        },
    )
}

fn strong_component_array(forge: &DataForge<'_>, mut structure: Struct<'_>) -> bool {
    let mut found = false;
    for _ in 0..forge.structs().len() {
        let mut count = 0;
        for property in structure.properties() {
            count += 1;
            let Ok(name) = property.name() else {
                return false;
            };
            if name == COMPONENTS {
                if found
                    || !property.is_array()
                    || !matches!(property.data_type(), Ok(DataType::StrongPointer))
                {
                    return false;
                }
                found = true;
            }
        }
        if count != structure.property_range().len() {
            return false;
        }
        let Some(parent) = structure.parent() else {
            return found;
        };
        structure = parent;
    }
    false
}

pub(in crate::build::catalog_queries) fn crafter_admission<'source>(
    forge: &'source DataForge<'source>,
    cancel: Option<&CancelToken>,
) -> Result<
    impl FnMut(
        &Record<'source>,
        &mut TreeBuilder<'source>,
        &mut CatalogRecordCoverage,
    ) -> Result<bool, CatalogError>
    + use<'source>,
    CatalogError,
> {
    let mut may_match = component_candidate(forge, &[CRAFTER_STRUCT], cancel)?;
    let standalone = forge.pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT);
    Ok(
        move |record: &Record<'source>,
              trees: &mut TreeBuilder<'source>,
              coverage: &mut CatalogRecordCoverage| {
            match record.type_name() {
                Ok(p4kx_assembly::ENTITY_TYPE) => {}
                Ok(_) => return Ok(false),
                Err(_) => {
                    coverage.unreadable_headers.push(record.index());
                    return Ok(false);
                }
            }
            match entity_selection::select(record, None) {
                Ok(Some(
                    selected @ (Selected::Entity(Kind::Crafters)
                    | Selected::Leftover(Kind::Crafters)),
                )) => {
                    let leftover = matches!(selected, Selected::Leftover(Kind::Crafters));
                    match standalone
                        .as_ref()
                        .map_or(Ok(false), |probe| trees.holds(probe, record))
                    {
                        Ok(true) if leftover => coverage.skipped_leftovers += 1,
                        Ok(holds) => return Ok(holds),
                        Err(_) => coverage.unreadable_bodies.push(record.index()),
                    }
                }
                Ok(Some(Selected::Entity(kind)))
                    if crate::catalog_v3::industry_modules::crafter_definition::candidate(kind) =>
                {
                    return Ok(may_match(record, trees));
                }
                Ok(_) => {}
                Err(_) => coverage.unreadable_headers.push(record.index()),
            }
            Ok(false)
        },
    )
}

pub(in crate::build::catalog_queries) fn read_owner<'source>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<Owner<'source>>, CatalogError> {
    match trees.build(record) {
        Ok(tree) => Ok(Some(Owner {
            record: *record,
            tree,
        })),
        Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
        Err(_) => {
            coverage.unreadable_bodies.push(record.index());
            Ok(None)
        }
    }
}
