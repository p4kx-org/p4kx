// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_character::{ChfNameKind, NameTable};
use p4kx_extraction::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

pub(crate) type NameKey = (ChfNameKind, u32);

/// CHF name winners together with the source selectors and values they consumed.
pub struct ObservedChfNames {
    pub(crate) names: NameTable,
    pub(crate) choices: HashMap<NameKey, NameChoice>,
}

pub(crate) enum NameChoice {
    Port {
        selector: String,
        record: Option<u32>,
        component: usize,
        port: usize,
    },
    Material {
        selector: usize,
        record: u32,
        lookup: usize,
        input: LibraryInput,
        material: usize,
        parameter: Option<usize>,
    },
}

#[derive(Clone)]
pub(crate) struct LibraryInput {
    pub(crate) scope: SourceScope,
    pub(crate) complete: bool,
}

/// Actual additional contributions to one emitted archive-derived name.
#[non_exhaustive]
pub struct ChfNameContributions {
    /// Uses of the source selectors and observed Archive name.
    pub uses: Vec<SourceUse>,
    /// Whether the owning input and each contribution were retained.
    pub complete: bool,
}

impl ObservedChfNames {
    /// Returns the exact winning table used by the CHF formatter.
    #[must_use]
    pub const fn names(&self) -> &NameTable {
        &self.names
    }

    /// Resolves the already-selected source uses for one emitted Archive label.
    ///
    /// # Errors
    /// Returns an error when the source-use registry cannot record a contribution.
    pub fn contributions(
        &self,
        kind: ChfNameKind,
        hash: u32,
        chf: &SourceScope,
        forge: &SourceScope,
    ) -> Result<Option<ChfNameContributions>, SourceEvidenceError> {
        self.choices
            .get(&(kind, hash))
            .map(|choice| choice.contributions(chf, forge))
            .transpose()
    }
}

impl NameChoice {
    fn contributions(
        &self,
        chf: &SourceScope,
        forge: &SourceScope,
    ) -> Result<ChfNameContributions, SourceEvidenceError> {
        match self {
            Self::Port {
                selector,
                record,
                component,
                port,
            } => {
                let mut uses = vec![chf_selector(chf, selector.clone())?];
                if let Some(record) = record {
                    uses.push(forge.at(
                        SourceLocation::DataForge {
                            record_index: *record,
                            field: format!("Components[{component}].Ports[{port}].Name"),
                        },
                        SourceUseRole::Conversion,
                    )?);
                }
                Ok(ChfNameContributions {
                    uses,
                    complete: record.is_some(),
                })
            }
            Self::Material {
                selector,
                record,
                lookup,
                input,
                material,
                parameter,
            } => material_contributions(
                chf,
                forge,
                MaterialNameSite {
                    selector: *selector,
                    record: *record,
                    lookup: *lookup,
                    input,
                    material: *material,
                    parameter: *parameter,
                },
            ),
        }
    }
}

#[derive(Clone, Copy)]
struct MaterialNameSite<'input> {
    selector: usize,
    record: u32,
    lookup: usize,
    input: &'input LibraryInput,
    material: usize,
    parameter: Option<usize>,
}

fn material_contributions(
    chf: &SourceScope,
    forge: &SourceScope,
    site: MaterialNameSite<'_>,
) -> Result<ChfNameContributions, SourceEvidenceError> {
    let mut uses = vec![chf_selector(
        chf,
        format!("/character/materials/{}/base_material", site.selector),
    )?];
    for field in ["guid", "filePath"] {
        uses.push(forge.at(
            SourceLocation::DataForge {
                record_index: site.record,
                field: format!("materialLookupTable.materials[{}].{field}", site.lookup),
            },
            SourceUseRole::Selector,
        )?);
    }
    let mut indices = vec![site.material as u64];
    let field = site.parameter.map_or("Name", |parameter| {
        indices.push(parameter as u64);
        "publicParameterName"
    });
    uses.push(site.input.scope.at(
        SourceLocation::Subresource {
            namespace: "mtlMaterial".to_owned(),
            indices,
            field: field.to_owned(),
        },
        SourceUseRole::Payload,
    )?);
    Ok(ChfNameContributions {
        uses,
        complete: site.input.complete,
    })
}

fn chf_selector(chf: &SourceScope, pointer: String) -> Result<SourceUse, SourceEvidenceError> {
    chf.at(
        SourceLocation::Subresource {
            namespace: "chf".to_owned(),
            indices: Vec::new(),
            field: pointer,
        },
        SourceUseRole::Selector,
    )
}
