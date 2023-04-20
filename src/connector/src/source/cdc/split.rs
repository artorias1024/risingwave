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

use anyhow::anyhow;
use risingwave_common::types::JsonbVal;
use serde::{Deserialize, Serialize};

use crate::source::{SplitId, SplitMetaData};

/// The states of a CDC split, which will be persisted to checkpoint.
/// CDC source only has single split, so we use the `source_id` to identify the split.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Hash)]
pub struct CdcSplit {
    pub split_id: u32,
    // the hostname and port of a node that holding shard tables
    pub server_addr: Option<String>,
    pub start_offset: Option<String>,
}

impl SplitMetaData for CdcSplit {
    fn id(&self) -> SplitId {
        format!("{}", self.split_id).into()
    }

    fn restore_from_json(value: JsonbVal) -> anyhow::Result<Self> {
        serde_json::from_value(value.take()).map_err(|e| anyhow!(e))
    }

    fn encode_to_json(&self) -> JsonbVal {
        serde_json::to_value(self.clone()).unwrap().into()
    }
}

impl CdcSplit {
    pub fn new(split_id: u32, start_offset: String) -> CdcSplit {
        Self {
            split_id,
            server_addr: None,
            start_offset: Some(start_offset),
        }
    }

    pub fn copy_with_offset(&self, start_offset: String) -> Self {
        Self {
            split_id: self.split_id,
            server_addr: self.server_addr.clone(),
            start_offset: Some(start_offset),
        }
    }
}
