// Copyright 2023 RisingWave Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt;
use std::rc::Rc;

use fixedbitset::FixedBitSet;
use itertools::Itertools;
use risingwave_pb::stream_plan::stream_node::PbNodeBody;
use risingwave_pb::stream_plan::{PbStreamSource, SourceNode};

use super::{generic, ExprRewritable, PlanBase, StreamNode};
use crate::catalog::source_catalog::SourceCatalog;
use crate::optimizer::property::Distribution;
use crate::stream_fragmenter::BuildFragmentGraphState;

/// [`StreamSource`] represents a table/connector source at the very beginning of the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamSource {
    pub base: PlanBase,
    logical: generic::Source,
}

impl StreamSource {
    pub fn new(logical: generic::Source) -> Self {
        let base = PlanBase::new_logical_with_core(&logical);
        let mut watermark_columns = FixedBitSet::with_capacity(base.schema.len());
        if let Some(catalog) = &logical.catalog {
            catalog
                .watermark_descs
                .iter()
                .for_each(|desc| watermark_columns.insert(desc.watermark_idx as usize))
        }

        let base = PlanBase::new_stream(
            base.ctx,
            base.schema,
            base.logical_pk,
            base.functional_dependency,
            Distribution::SomeShard,
            logical.catalog.as_ref().map_or(true, |s| s.append_only),
            watermark_columns,
        );
        Self { base, logical }
    }

    pub fn source_catalog(&self) -> Option<Rc<SourceCatalog>> {
        self.logical.catalog.clone()
    }

    pub fn column_names(&self) -> Vec<String> {
        self.schema()
            .fields()
            .iter()
            .map(|f| f.name.clone())
            .collect()
    }
}

impl_plan_tree_node_for_leaf! { StreamSource }

impl fmt::Display for StreamSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("StreamSource");
        if let Some(catalog) = self.source_catalog() {
            builder
                .field("source", &catalog.name)
                .field("columns", &self.column_names());
        }
        builder.finish()
    }
}

impl StreamNode for StreamSource {
    fn to_stream_prost_body(&self, state: &mut BuildFragmentGraphState) -> PbNodeBody {
        let source_catalog = self.source_catalog();
        let source_inner = source_catalog.map(|source_catalog| PbStreamSource {
            source_id: source_catalog.id,
            source_name: source_catalog.name.clone(),
            state_table: Some(
                generic::Source::infer_internal_table_catalog()
                    .with_id(state.gen_table_id_wrapped())
                    .to_internal_table_prost(),
            ),
            info: Some(source_catalog.info.clone()),
            row_id_index: self.logical.row_id_index.map(|index| index as _),
            columns: self
                .logical
                .column_catalog
                .iter()
                .map(|c| c.to_protobuf())
                .collect_vec(),
            properties: source_catalog.properties.clone().into_iter().collect(),
        });
        PbNodeBody::Source(SourceNode { source_inner })
    }
}

impl ExprRewritable for StreamSource {}
