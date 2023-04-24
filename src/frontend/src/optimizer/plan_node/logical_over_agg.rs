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

use fixedbitset::FixedBitSet;
use itertools::Itertools;
use risingwave_common::error::{ErrorCode, Result};
use risingwave_common::types::DataType;
use risingwave_common::util::sort_util::ColumnOrder;
use risingwave_expr::function::window::{Frame, FrameBound, WindowFuncKind};

use super::generic::{OverWindow, PlanWindowFunction};
use super::{
    gen_filter_and_pushdown, ColPrunable, ExprRewritable, LogicalProject, PlanBase, PlanRef,
    PlanTreeNodeUnary, PredicatePushdown, ToBatch, ToStream,
};
use crate::expr::{Expr, ExprImpl, InputRef, WindowFunction};
use crate::optimizer::plan_node::{
    ColumnPruningContext, PredicatePushdownContext, RewriteStreamContext, ToStreamContext,
};
use crate::utils::{ColIndexMapping, Condition};

/// `LogicalOverAgg` performs `OVER` window aggregates ([`WindowFunction`]) to its input.
///
/// The output schema is the input schema plus the window functions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogicalOverAgg {
    pub base: PlanBase,
    core: OverWindow<PlanRef>,
}

impl LogicalOverAgg {
    fn new(calls: Vec<PlanWindowFunction>, input: PlanRef) -> Self {
        let core = OverWindow::new(calls, input);
        let base = PlanBase::new_logical_with_core(&core);
        Self { base, core }
    }

    pub fn create(
        input: PlanRef,
        mut select_exprs: Vec<ExprImpl>,
    ) -> Result<(PlanRef, Vec<ExprImpl>)> {
        let input_len = input.schema().len();
        let mut window_funcs = vec![];
        for expr in &mut select_exprs {
            if let ExprImpl::WindowFunction(_) = expr {
                let new_expr =
                    InputRef::new(input_len + window_funcs.len(), expr.return_type().clone())
                        .into();
                let f = std::mem::replace(expr, new_expr)
                    .into_window_function()
                    .unwrap();
                window_funcs.push(*f);
            }
            if expr.has_window_function() {
                return Err(ErrorCode::NotImplemented(
                    format!("window function in expression: {:?}", expr),
                    None.into(),
                )
                .into());
            }
        }
        for f in &window_funcs {
            if f.kind.is_rank() {
                if f.order_by.sort_exprs.is_empty() {
                    return Err(ErrorCode::InvalidInputSyntax(format!(
                        "window rank function without order by: {:?}",
                        f
                    ))
                    .into());
                }
                if f.kind == WindowFuncKind::DenseRank {
                    return Err(ErrorCode::NotImplemented(
                        format!("window rank function: {}", f.kind),
                        4847.into(),
                    )
                    .into());
                }
            }
        }

        let plan_window_funcs = window_funcs
            .into_iter()
            .map(Self::convert_window_function)
            .try_collect()?;

        let over_agg = Self::new(plan_window_funcs, input);
        Ok((over_agg.into(), select_exprs))
    }

    fn convert_window_function(window_function: WindowFunction) -> Result<PlanWindowFunction> {
        // TODO: rewrite expressions in `ORDER BY`, `PARTITION BY` and arguments to `InputRef` like
        // in `LogicalAgg`
        let order_by: Vec<_> = window_function
            .order_by
            .sort_exprs
            .into_iter()
            .map(|e| match e.expr.as_input_ref() {
                Some(i) => Ok(ColumnOrder::new(i.index(), e.order_type)),
                None => Err(ErrorCode::NotImplemented(
                    "ORDER BY expression in window function".to_string(),
                    None.into(),
                )),
            })
            .try_collect()?;
        let partition_by: Vec<_> = window_function
            .partition_by
            .into_iter()
            .map(|e| match e.as_input_ref() {
                Some(i) => Ok(*i.clone()),
                None => Err(ErrorCode::NotImplemented(
                    "PARTITION BY expression in window function".to_string(),
                    None.into(),
                )),
            })
            .try_collect()?;

        let mut args = window_function.args;
        let frame = match window_function.kind {
            WindowFuncKind::RowNumber | WindowFuncKind::Rank | WindowFuncKind::DenseRank => {
                // ignore frame for rank functions
                None
            }
            WindowFuncKind::Lag | WindowFuncKind::Lead => {
                let offset = if args.len() > 1 {
                    let offset_expr = args.remove(1);
                    if !offset_expr.return_type().is_int() {
                        return Err(ErrorCode::InvalidInputSyntax(format!(
                            "the `offset` of `{}` function should be integer",
                            window_function.kind
                        ))
                        .into());
                    }
                    offset_expr
                        .cast_implicit(DataType::Int64)?
                        .eval_row_const()?
                        .map(|v| *v.as_int64() as usize)
                        .unwrap_or(1usize)
                } else {
                    1usize
                };

                // override the frame
                // TODO(rc): We can only do the optimization for constant offset.
                Some(if window_function.kind == WindowFuncKind::Lag {
                    Frame::Rows(FrameBound::Preceding(offset), FrameBound::CurrentRow)
                } else {
                    Frame::Rows(FrameBound::CurrentRow, FrameBound::Following(offset))
                })
            }
            _ => window_function.frame,
        };

        let args = args
            .into_iter()
            .map(|e| match e.as_input_ref() {
                Some(i) => Ok(*i.clone()),
                None => Err(ErrorCode::NotImplemented(
                    "expression arguments in window function".to_string(),
                    None.into(),
                )),
            })
            .try_collect()?;

        Ok(PlanWindowFunction {
            kind: window_function.kind,
            return_type: window_function.return_type,
            args,
            partition_by,
            order_by,
            frame,
        })
    }

    pub fn window_functions(&self) -> &[PlanWindowFunction] {
        &self.core.window_functions
    }
}

impl PlanTreeNodeUnary for LogicalOverAgg {
    fn input(&self) -> PlanRef {
        self.core.input.clone()
    }

    fn clone_with_input(&self, input: PlanRef) -> Self {
        Self::new(self.core.window_functions.clone(), input)
    }
}

impl_plan_tree_node_for_unary! { LogicalOverAgg }

impl fmt::Display for LogicalOverAgg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.core.fmt_with_name(f, "LogicalOverAgg")
    }
}

impl ColPrunable for LogicalOverAgg {
    fn prune_col(&self, required_cols: &[usize], ctx: &mut ColumnPruningContext) -> PlanRef {
        let mapping = ColIndexMapping::with_remaining_columns(required_cols, self.schema().len());
        let new_input = {
            let input = self.input();
            let required = (0..input.schema().len()).collect_vec(); // TODO(rc): real pruning
            input.prune_col(&required, ctx)
        };
        LogicalProject::with_mapping(self.clone_with_input(new_input).into(), mapping).into()
    }
}

impl ExprRewritable for LogicalOverAgg {}

impl PredicatePushdown for LogicalOverAgg {
    fn predicate_pushdown(
        &self,
        predicate: Condition,
        ctx: &mut PredicatePushdownContext,
    ) -> PlanRef {
        let mut window_col = FixedBitSet::with_capacity(self.schema().len());
        window_col.insert_range(self.core.input.schema().len()..self.schema().len());
        let (window_pred, other_pred) = predicate.split_disjoint(&window_col);
        gen_filter_and_pushdown(self, window_pred, other_pred, ctx)
    }
}

impl ToBatch for LogicalOverAgg {
    fn to_batch(&self) -> Result<PlanRef> {
        Err(ErrorCode::NotImplemented("OverAgg to batch".to_string(), 9124.into()).into())
    }
}

impl ToStream for LogicalOverAgg {
    fn to_stream(&self, _ctx: &mut ToStreamContext) -> Result<PlanRef> {
        Err(ErrorCode::NotImplemented("OverAgg to stream".to_string(), 9124.into()).into())
    }

    fn logical_rewrite_for_stream(
        &self,
        _ctx: &mut RewriteStreamContext,
    ) -> Result<(PlanRef, ColIndexMapping)> {
        Err(ErrorCode::NotImplemented("OverAgg to stream".to_string(), 9124.into()).into())
    }
}
