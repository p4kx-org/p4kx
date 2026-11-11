// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use crate::CatalogError;
use crate::contract_facts::RequiredIntro;
use crate::contract_missions::{ContractBase, ContractEntry, ContractWithTitles, StandingLevel};
use crate::guid_text::CatalogGuid;
use crate::mission_titles::MissionTitles;

#[path = "mission_intro_grant_read.rs"]
mod grant_read;
#[path = "mission_intro_grants.rs"]
mod grants;
#[path = "mission_intro_read.rs"]
mod read;
#[path = "mission_intro_requirements.rs"]
mod requirements;

pub(crate) use self::grants::{Candidates, Grants, INTRO_CONTRACTS};
pub(crate) use self::requirements::Requirements;

use self::read::Lookup;

#[derive(Clone, Debug)]
pub(crate) struct AuthoredIntro {
    level: SourcedValue<StandingLevel>,
    tag_guid: SourcedValue<CatalogGuid>,
    lookup: Lookup,
}

/// A contract row whose authored prerequisites still need the admitted grant join.
pub(crate) struct PendingContract<'source> {
    pub(crate) base: ContractBase<'source>,
    pub(crate) titles: MissionTitles<'source>,
    pub(crate) requirements: SourcedValue<Vec<AuthoredIntro>>,
}

/// Joins authored requirements to successful physical intro grant witnesses.
pub(crate) fn resolve(
    entries: Vec<PendingContract<'_>>,
    grants: Grants,
) -> Result<Vec<ContractWithTitles<'_>>, CatalogError> {
    let mut resolver = grants.resolver();
    entries
        .into_iter()
        .map(|entry| {
            let required_intros = entry.requirements.try_map(|requirements| {
                requirements
                    .into_iter()
                    .map(
                        |AuthoredIntro {
                             level,
                             tag_guid,
                             lookup,
                         }| {
                            Ok::<_, CatalogError>(RequiredIntro {
                                level,
                                tag_guid,
                                intro_contracts: resolver.grants(&lookup)?,
                            })
                        },
                    )
                    .collect::<Result<Vec<_>, CatalogError>>()
            })?;
            Ok(ContractWithTitles {
                row: ContractEntry {
                    base: entry.base,
                    required_intros,
                },
                titles: entry.titles,
            })
        })
        .collect()
}
