// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::{Entities, SocLightClass, SocLightClassSelector, SocLightClassSource};
use p4kx_extraction::{
    CancelToken, DataForge, Guid, Record, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, SourceValue, SourceValueState, SourcedValue, TreeBuilder, Value,
};
use p4kx_model::{SocEntity, SocLightSource};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::{at, check};
use crate::soc_scene_light_defaults::{DefaultComponent, EffectiveField};
use crate::soc_scene_origin::XmlOrigin;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Selector {
    Guid(Guid),
    Name(String),
}

impl Selector {
    pub(crate) fn read(value: SocLightClassSelector<'_>) -> Option<Self> {
        match value {
            SocLightClassSelector::Guid(guid) => Some(Self::Guid(guid)),
            SocLightClassSelector::Name(name) => Some(Self::Name(name.to_owned())),
            _ => None,
        }
    }

    const fn as_source(&self) -> SocLightClassSelector<'_> {
        match self {
            Self::Guid(guid) => SocLightClassSelector::Guid(*guid),
            Self::Name(name) => SocLightClassSelector::Name(name.as_str()),
        }
    }
}

#[derive(Default)]
pub(crate) struct LightClasses {
    selectors: BTreeMap<Selector, Candidates>,
    classes: BTreeMap<u32, DefaultClass>,
    lookup_use: Option<SourceUse>,
}

enum Candidates {
    Missing,
    Ambiguous(Vec<ClassSite>),
    UnexpectedType(ClassSite),
    Matched(ClassSite),
}

#[derive(Debug)]
struct ClassSite {
    record: u32,
    uses: [SourceUse; 2],
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DefaultClass {
    record_index: u32,
    components: SourcedValue<Vec<DefaultComponent>>,
    #[serde(skip)]
    #[schemars(skip)]
    component_use: Option<SourceUse>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectiveLight {
    class: SourcedValue<ClassSelection>,
    fields: Vec<EffectiveField>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ClassSelection {
    Missing,
    Ambiguous { record_indices: Vec<u32> },
    UnexpectedType { record_index: u32 },
    Matched { record_index: u32 },
    NoComponent { record_index: u32 },
    AmbiguousComponents { record_index: u32 },
}

impl LightClasses {
    pub(crate) fn read(
        wanted: BTreeSet<Selector>,
        forge: &DataForge<'_>,
        scope: &SourceScope,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        if wanted.is_empty() {
            return Ok(Self::default());
        }
        let entities = Entities::new(forge);
        let lookup_use = scope.at(
            SourceLocation::Metadata {
                field: "recordClassLookup".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        let mut classes = Self {
            lookup_use: Some(lookup_use),
            ..Self::default()
        };
        let mut builder = TreeBuilder::new();
        for selector in wanted {
            check(cancel)?;
            let candidates = match SocLightClass::resolve_selector(selector.as_source(), &entities)
            {
                SocLightClass::Matched(record) => {
                    if let std::collections::btree_map::Entry::Vacant(entry) =
                        classes.classes.entry(record.index())
                    {
                        entry.insert(DefaultClass::read(&record, scope, sources, &mut builder)?);
                    }
                    Candidates::Matched(ClassSite::read(record.index(), &selector, scope)?)
                }
                SocLightClass::UnexpectedRecordType(record) => {
                    Candidates::UnexpectedType(ClassSite::read(record.index(), &selector, scope)?)
                }
                SocLightClass::Ambiguous(records) => Candidates::Ambiguous(
                    records
                        .into_iter()
                        .map(|record| ClassSite::read(record.index(), &selector, scope))
                        .collect::<Result<_, _>>()?,
                ),
                _ => Candidates::Missing,
            };
            classes.selectors.insert(selector, candidates);
        }
        Ok(classes)
    }

    pub(crate) fn effective(
        &self,
        entity: SocEntity<'_>,
        light: SocLightSource<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<EffectiveLight, SocSceneError> {
        let selector = SocLightClassSelector::read(entity);
        let mut uses = vec![origin.node(entity.element(), SourceUseRole::Selector)?];
        for attribute in entity.class_guid_attributes().chain(
            entity
                .class_name_attributes()
                .filter(|_| !matches!(selector, SocLightClassSelector::Guid(_))),
        ) {
            uses.push(origin.attribute(&attribute, SourceUseRole::Selector)?.1);
        }
        let mut fields = Vec::new();
        let selection = match selector {
            SocLightClassSelector::Absent => SourceValue::Absent,
            SocLightClassSelector::Invalid => SourceValue::Invalid,
            _ => {
                match Selector::read(selector).and_then(|selector| self.selectors.get(&selector)) {
                    Some(Candidates::Matched(site)) => {
                        uses.extend(site.uses.iter().cloned());
                        self.resolve(site.record, light, origin, &mut uses, &mut fields)?
                    }
                    Some(Candidates::Missing) => {
                        uses.extend(self.lookup_use.iter().cloned());
                        SourceValue::Present(ClassSelection::Missing)
                    }
                    Some(Candidates::UnexpectedType(site)) => {
                        uses.extend(site.uses.iter().cloned());
                        SourceValue::Present(ClassSelection::UnexpectedType {
                            record_index: site.record,
                        })
                    }
                    Some(Candidates::Ambiguous(sites)) => {
                        uses.extend(sites.iter().flat_map(|site| site.uses.iter().cloned()));
                        SourceValue::Present(ClassSelection::Ambiguous {
                            record_indices: sites.iter().map(|site| site.record).collect(),
                        })
                    }
                    None => SourceValue::Unreadable,
                }
            }
        };
        let complete = !matches!(selection, SourceValue::Unreadable);
        Ok(EffectiveLight {
            class: origin.sources.value(selection, &uses, complete),
            fields,
        })
    }

    fn resolve(
        &self,
        record: u32,
        light: SocLightSource<'_>,
        origin: &XmlOrigin<'_>,
        uses: &mut Vec<SourceUse>,
        fields: &mut Vec<EffectiveField>,
    ) -> Result<SourceValue<ClassSelection>, SocSceneError> {
        let Some(class) = self.classes.get(&record) else {
            return Err(SocSceneError::Position);
        };
        uses.extend(class.component_use.iter().cloned());
        let Some(components) = class.components.value() else {
            return Ok(match class.components.state() {
                SourceValueState::Absent | SourceValueState::Null => {
                    SourceValue::Present(ClassSelection::NoComponent {
                        record_index: record,
                    })
                }
                SourceValueState::Invalid => SourceValue::Invalid,
                SourceValueState::Unsupported => SourceValue::Unsupported,
                _ => SourceValue::Unreadable,
            });
        };
        let selection = match components.as_slice() {
            [] => ClassSelection::NoComponent {
                record_index: record,
            },
            [component] => {
                *fields = component.resolve(light.element(), origin, uses)?;
                ClassSelection::Matched {
                    record_index: record,
                }
            }
            _ => ClassSelection::AmbiguousComponents {
                record_index: record,
            },
        };
        Ok(SourceValue::Present(selection))
    }

    pub(crate) fn finish(self) -> Vec<DefaultClass> {
        self.classes.into_values().collect()
    }
}

impl DefaultClass {
    fn read<'source>(
        record: &Record<'source>,
        scope: &SourceScope,
        sources: &SourceUses,
        builder: &mut TreeBuilder<'source>,
    ) -> Result<Self, SocSceneError> {
        let index = record.index();
        let source = SocLightClassSource::read(record, builder);
        let value = if let Ok(source) = &source {
            let mut fields = source
                .tree()
                .fields()
                .iter()
                .filter(|field| field.name() == "Components");
            match (
                fields.next().map(p4kx_extraction::Field::value),
                fields.next(),
            ) {
                (_, Some(_)) => SourceValue::Invalid,
                (Some(Value::Array(slots)), None)
                    if slots.iter().any(|slot| matches!(slot, Value::Error(_))) =>
                {
                    SourceValue::Unreadable
                }
                (Some(Value::Array(_)), None) => SourceValue::Present(
                    source
                        .components()
                        .map(|(ordinal, component)| {
                            DefaultComponent::read(index, ordinal, component, scope, sources)
                        })
                        .collect::<Result<_, _>>()?,
                ),
                (Some(Value::Null), None) => SourceValue::Null,
                (Some(Value::Error(_)), None) => SourceValue::Unreadable,
                (Some(_), None) => SourceValue::Unsupported,
                (None, None) => SourceValue::Absent,
            }
        } else {
            SourceValue::Unreadable
        };
        let used = at(
            scope,
            index,
            "Components".to_owned(),
            SourceUseRole::Payload,
        )?;
        let complete = !matches!(value, SourceValue::Unreadable);
        let component_use = source.is_ok().then_some(used);
        Ok(Self {
            record_index: index,
            components: sources.value(value, component_use.as_slice(), complete),
            component_use,
        })
    }
}

impl ClassSite {
    fn read(record: u32, selector: &Selector, scope: &SourceScope) -> Result<Self, SocSceneError> {
        let key = match selector {
            Selector::Guid(_) => "$guid",
            Selector::Name(_) => "$name",
        };
        Ok(Self {
            record,
            uses: [
                at(scope, record, key.to_owned(), SourceUseRole::Selector)?,
                at(scope, record, "$type".to_owned(), SourceUseRole::Payload)?,
            ],
        })
    }
}
