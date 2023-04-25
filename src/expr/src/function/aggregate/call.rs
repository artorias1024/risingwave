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

use std::sync::Arc;

use risingwave_common::types::DataType;
use risingwave_common::util::sort_util::{ColumnOrder, OrderType};
use risingwave_pb::expr::PbAggCall;

use super::AggKind;
use crate::expr::{build_from_prost, ExpressionRef};
use crate::function::aggregate::AggArgs;
use crate::Result;

/// Represents an aggregation function.
#[derive(Clone, Debug)]
pub struct AggCall {
    /// Aggregation kind for constructing agg state.
    pub kind: AggKind,
    /// Arguments of aggregation function input.
    pub args: AggArgs,
    /// The return type of aggregation function.
    pub return_type: DataType,

    /// Order requirements specified in order by clause of agg call
    pub column_orders: Vec<ColumnOrder>,

    /// Filter of aggregation.
    pub filter: Option<ExpressionRef>,

    /// Should deduplicate the input before aggregation.
    pub distinct: bool,
}

impl AggCall {
    pub fn from_protobuf(agg_call: &PbAggCall) -> Result<Self> {
        let agg_kind = AggKind::from_protobuf(agg_call.get_type()?)?;
        let args = AggArgs::from_protobuf(agg_call.get_args())?;
        let column_orders = agg_call
            .get_order_by()
            .iter()
            .map(|col_order| {
                let col_idx = col_order.get_column_index() as usize;
                let order_type = OrderType::from_protobuf(col_order.get_order_type().unwrap());
                ColumnOrder::new(col_idx, order_type)
            })
            .collect();
        let filter = match agg_call.filter {
            Some(ref pb_filter) => Some(Arc::from(build_from_prost(pb_filter)?)),
            None => None,
        };
        Ok(AggCall {
            kind: agg_kind,
            args,
            return_type: DataType::from(agg_call.get_return_type()?),
            column_orders,
            filter,
            distinct: agg_call.distinct,
        })
    }
}
