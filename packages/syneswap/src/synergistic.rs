use cosmwasm_std::{Uint128};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyneExecuteMsg {
    Claim {
        amount: Uint128,
    }
}