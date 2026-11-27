// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataType;
use crate::query::reader::{Link, Step};
use crate::query::resolve::{At, Resolver};
use crate::query::syntax::Name;
use crate::query::{QueryError, QueryErrorKind, Span};

impl Resolver<'_, '_, '_> {
    /// Follows a Reference or weak pointer with `->` to the field `next` of the struct its field
    /// declares.
    pub(super) fn follow(&mut self, arrow: Span, next: &Name) -> Result<(), QueryError> {
        let (data_type, target) = match self.at {
            At::Link { data_type, target } => (data_type, target),
            At::Array { data_type, .. } => {
                return Err(self.error(
                    |name, owner| QueryErrorKind::ArrayIsNoLink {
                        name,
                        owner,
                        data_type,
                    },
                    arrow,
                ));
            }
            At::Struct { holds, .. } => return Err(self.not_a_link(holds, arrow)),
            At::Pointer { .. } => return Err(self.not_a_link(DataType::StrongPointer, arrow)),
            At::Scalar { data_type, .. } => return Err(self.not_a_link(data_type, arrow)),
        };
        let index = usize::from(target);
        if index >= self.structs.len() {
            let next = next.as_str().to_owned();
            return Err(self.error(
                |name, owner| QueryErrorKind::UntypedLink {
                    name,
                    owner,
                    data_type,
                    next,
                },
                arrow,
            ));
        }
        let link = if data_type == DataType::Reference {
            Link::Reference
        } else {
            Link::Weak
        };
        self.leave(Step::Follow {
            at: self.offset,
            link,
            declared: Box::new([index]),
        });
        self.at = At::Struct {
            index,
            holds: data_type,
        };
        self.field(next)
    }

    fn not_a_link(&self, data_type: DataType, arrow: Span) -> QueryError {
        self.error(
            |name, owner| QueryErrorKind::NotALink {
                name,
                owner,
                data_type,
            },
            arrow,
        )
    }
}
