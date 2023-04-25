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

use parse_display::{Display, FromStr};

use crate::function::aggregate::AggKind;

/// Kind of window functions.
#[derive(Debug, Display, FromStr, Copy, Clone, PartialEq, Eq, Hash)]
#[display(style = "snake_case")]
pub enum WindowFuncKind {
    // General-purpose window functions.
    RowNumber,
    Rank,
    DenseRank,
    Lag,
    Lead,
    // FirstValue,
    // LastValue,
    // NthValue,

    // Aggregate functions that are used with `OVER`.
    #[display("{0}")]
    Aggregate(AggKind),
}

impl WindowFuncKind {
    pub fn is_rank(&self) -> bool {
        matches!(self, Self::RowNumber | Self::Rank | Self::DenseRank)
    }
}
