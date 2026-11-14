// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, FieldOrigin, PhysicalSourceRef, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

#[path = "player_adb_declaration_pending.rs"]
mod pending;

pub(crate) use pending::{Draft, FragmentInput, FragmentSite, ordered_children};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdbDeclarations {
    pub(crate) document: usize,
    pub(crate) source: PhysicalSourceRef,
    pub(crate) fragments: Vec<Fragment>,
    #[serde(skip)]
    pub(crate) branches: Vec<Branch>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Fragment {
    list_ordinal: usize,
    group_ordinal: usize,
    option_ordinal: usize,
    group_position: SourcedValue<SocXmlPosition>,
    pub(crate) name: SourcedValue<String>,
    #[serde(skip)]
    pub(crate) tags: AttributeSelection,
    #[serde(skip)]
    pub(crate) local_tags: AttributeSelection,
    position: SourcedValue<SocXmlPosition>,
    guid: AttributeSelection,
    layers: Vec<Layer>,
}

#[derive(Debug)]
pub(crate) struct Branch {
    pub(crate) ordinal: usize,
    pub(crate) position: SourcedValue<SocXmlPosition>,
    pub(crate) tags: AttributeSelection,
    pub(crate) fragments: Vec<BranchFragment>,
}

#[derive(Debug)]
pub(crate) struct BranchFragment {
    pub(crate) ordinal: usize,
    pub(crate) position: SourcedValue<SocXmlPosition>,
    pub(crate) name: AttributeSelection,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Layer {
    ordinal: usize,
    position: SourcedValue<SocXmlPosition>,
    animations: Vec<Animation>,
    blends: Vec<Blend>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Animation {
    ordinal: usize,
    position: SourcedValue<SocXmlPosition>,
    name: AttributeSelection,
    alias_key: Option<SourcedValue<String>>,
    flags: AttributeSelection,
    preceding_blend: SourcedValue<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Blend {
    ordinal: usize,
    position: SourcedValue<SocXmlPosition>,
    duration: AttributeSelection,
    start_time: AttributeSelection,
    exit_time: AttributeSelection,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttributeSelection {
    #[serde(flatten)]
    pub(crate) origin: FieldOrigin,
    state: AttributeState,
    pub(crate) occurrences: Vec<Attribute>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum AttributeState {
    Absent,
    Present,
    Invalid,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Attribute {
    position: SocXmlPosition,
    pub(crate) value: SourcedValue<String>,
}

impl AdbDeclarations {
    pub(crate) fn bind(
        document: usize,
        draft: Draft,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let origin = XmlOrigin {
            scope: draft.scope,
            sources,
            chunk: None,
            archive_member: None,
        };
        let mut fragments = Vec::with_capacity(draft.fragments.len());
        for fragment in draft.fragments {
            pending::check(cancel)?;
            fragments.push(Fragment {
                list_ordinal: fragment.site.list,
                group_ordinal: fragment.site.group,
                option_ordinal: fragment.site.option,
                group_position: origin.sources.value(
                    SourceValue::Present(fragment.group_position),
                    std::slice::from_ref(&fragment.group_use),
                    true,
                ),
                name: origin.sources.value(
                    SourceValue::Present(fragment.name),
                    std::slice::from_ref(&fragment.group_use),
                    true,
                ),
                guid: AttributeSelection::bind(fragment.guid, &fragment.node_use, &origin)?,
                tags: AttributeSelection::bind(fragment.tags, &fragment.node_use, &origin)?,
                local_tags: AttributeSelection::bind(
                    fragment.local_tags,
                    &fragment.node_use,
                    &origin,
                )?,
                position: origin.sources.value(
                    SourceValue::Present(fragment.position),
                    std::slice::from_ref(&fragment.node_use),
                    true,
                ),
                layers: layers(fragment.layers, &origin, cancel)?,
            });
        }
        Ok(Self {
            document,
            source: origin.source(),
            fragments,
            branches: branches(draft.branches, &origin, cancel)?,
        })
    }
}

fn branches(
    drafts: Vec<pending::Branch>,
    origin: &XmlOrigin<'_>,
    cancel: &CancelToken,
) -> Result<Vec<Branch>, SocSceneError> {
    drafts
        .into_iter()
        .map(|branch| {
            pending::check(cancel)?;
            let (position, owner) = node_position(branch.position, origin)?;
            let fragments = branch
                .fragments
                .into_iter()
                .map(|fragment| {
                    pending::check(cancel)?;
                    let (position, used) = node_position(fragment.position, origin)?;
                    Ok(BranchFragment {
                        ordinal: fragment.ordinal,
                        position,
                        name: AttributeSelection::bind(fragment.name, &used, origin)?,
                    })
                })
                .collect::<Result<_, SocSceneError>>()?;
            Ok(Branch {
                ordinal: branch.ordinal,
                position,
                tags: AttributeSelection::bind(branch.tags, &owner, origin)?,
                fragments,
            })
        })
        .collect()
}

fn layers(
    drafts: Vec<pending::Layer>,
    origin: &XmlOrigin<'_>,
    cancel: &CancelToken,
) -> Result<Vec<Layer>, SocSceneError> {
    let mut layers = Vec::with_capacity(drafts.len());
    for layer in drafts {
        pending::check(cancel)?;
        let (position, layer_use) = node_position(layer.position, origin)?;
        let blends = layer
            .blends
            .into_iter()
            .map(|blend| {
                pending::check(cancel)?;
                Blend::bind(blend, origin)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let animations = layer
            .animations
            .into_iter()
            .map(|animation| {
                pending::check(cancel)?;
                Animation::bind(animation, &layer_use, &blends, origin)
            })
            .collect::<Result<Vec<_>, _>>()?;
        layers.push(Layer {
            ordinal: layer.ordinal,
            position,
            animations,
            blends,
        });
    }
    Ok(layers)
}

impl Animation {
    fn bind(
        animation: pending::Animation,
        layer: &SourceUse,
        blends: &[Blend],
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let (position, used) = node_position(animation.position, origin)?;
        let name = AttributeSelection::bind(animation.name, &used, origin)?;
        let alias_key = match name.occurrences.as_slice() {
            [attribute] => attribute
                .value
                .value()
                .and_then(|value| crate::player_rig_clip::alias_key(value))
                .map(|key| attribute.value.map_ref(|_| key)),
            _ => None,
        };
        let flags = AttributeSelection::bind(animation.flags, &used, origin)?;
        let preceding_blend = match animation.preceding_blend {
            Some(index) => {
                let blend = blends.get(index).ok_or(SocSceneError::Position)?;
                let ordered_children = origin.sources.origin(&[layer.clone(), used], true);
                origin.sources.value_with_origins(
                    SourceValue::Present(blend.ordinal),
                    &[&ordered_children, blend.position.origin()],
                )?
            }
            None => origin
                .sources
                .value(SourceValue::Absent, &[layer.clone(), used], true),
        };
        Ok(Self {
            ordinal: animation.ordinal,
            position,
            name,
            alias_key,
            flags,
            preceding_blend,
        })
    }
}

impl Blend {
    fn bind(blend: pending::Blend, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        let (position, used) = node_position(blend.position, origin)?;
        Ok(Self {
            ordinal: blend.ordinal,
            position,
            duration: AttributeSelection::bind(blend.duration, &used, origin)?,
            start_time: AttributeSelection::bind(blend.start_time, &used, origin)?,
            exit_time: AttributeSelection::bind(blend.exit_time, &used, origin)?,
        })
    }
}

fn node_position(
    position: SocXmlPosition,
    origin: &XmlOrigin<'_>,
) -> Result<(SourcedValue<SocXmlPosition>, SourceUse), SocSceneError> {
    let used = origin.at(&position, "$element", SourceUseRole::Payload)?;
    let value = origin.sources.value(
        SourceValue::Present(position),
        std::slice::from_ref(&used),
        true,
    );
    Ok((value, used))
}

impl AttributeSelection {
    fn bind(
        draft: pending::Selector,
        owner: &SourceUse,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let mut uses = vec![owner.clone()];
        let mut occurrences = Vec::with_capacity(draft.occurrences.len());
        for occurrence in draft.occurrences {
            let attribute_use =
                origin.at(&occurrence.position, draft.name, SourceUseRole::Payload)?;
            let value = origin.sources.value(
                SourceValue::Present(occurrence.value),
                std::slice::from_ref(&attribute_use),
                true,
            );
            uses.push(attribute_use);
            occurrences.push(Attribute {
                position: occurrence.position,
                value,
            });
        }
        Ok(Self {
            state: match occurrences.len() {
                0 => AttributeState::Absent,
                1 => AttributeState::Present,
                _ => AttributeState::Invalid,
            },
            origin: origin.sources.origin(&uses, true),
            occurrences,
        })
    }
}
