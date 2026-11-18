// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashSet;

use p4kx_extraction::{CancelToken, DecimalU64, Guid, RecordTree, SourceUseRole, SourceValue};

use super::{BlueprintRewardRead, BlueprintRewardSource, scope as identity};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::authored_fields::Observed;
use crate::crafting_sources;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    wanted: &HashSet<Guid>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintRewardRead<'source>, CatalogError> {
    let mut steps = crafting_sources::walk(tree);
    let first = loop {
        crate::catalog_query_session::checkpoint(cancel)?;
        let Some(step) = steps.next() else {
            return Ok(BlueprintRewardRead::default());
        };
        if step.reward(tree, wanted).is_some() {
            break step;
        }
    };
    let scope = input.record_scope(admission.record(), tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&fields, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let source = Observed::configuration(&fields, input, &capture, &numbers);
    let header = CanonicalRecordHeader::read(tree, input.origins())?;
    let mut result = BlueprintRewardRead::default();
    for step in std::iter::once(first).chain(steps) {
        fields.check_cancel()?;
        if let Some(interpreted) = step.reward(tree, wanted) {
            let owner = fields.owner_of(step.node)?;
            let observed = owner.map_or(source, |owner| source.at_owner(owner));
            let chance = observed.authored_number("chance", &mut result.notes)?;
            let fact = fields.fact(step.value)?;
            let pool = super::pool::read(&fields, fact)?;
            let selected = identity::read(&fields, owner)?;
            result.rows.push(BlueprintRewardSource {
                source: header.clone(),
                source_ordinal: fact
                    .bind(
                        OriginalValuePart::Value,
                        SourceValue::Present(DecimalU64::new(fact.ordinal())),
                        SourceUseRole::Conversion,
                        true,
                    )?
                    .into_cell(fields.sources()),
                pool: fact
                    .bind(
                        OriginalValuePart::Value,
                        SourceValue::Present(pool.value),
                        pool.role,
                        true,
                    )?
                    .into_cell(fields.sources()),
                pool_storage: pool.storage,
                contract_id: selected.id,
                contract_name: selected.name,
                chance,
                chance_ordinal: fields
                    .field(owner, "chance")?
                    .map(|(_, fact)| fact.ordinal()),
            });
            result.interpreted.push(interpreted);
        }
    }
    result.numbers = NumberCapture::deduplicate(numbers.into_inner())?;
    Ok(result)
}
