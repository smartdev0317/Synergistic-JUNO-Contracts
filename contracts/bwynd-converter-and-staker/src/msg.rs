use cw20::{Cw20ReceiveMsg, Cw20Coin, MinterResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use syneswap::staking::InstantiateMarketingInfo;
use crate::state::{Config, Vote, GaugeConfig};
use cosmwasm_std::{CosmosMsg, Uint128, Addr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: String,
    pub cw20_code_id: u64,
    pub vault_code_id: u64,
    pub synergistic_wynd_gauge_contract: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ExecuteCosmosMsgs {
        msgs: Vec<CosmosMsg>,
    },
    Receive(Cw20ReceiveMsg),
    UpdateAdmin {
        address: String,
    },
    UpdateConfig {
        unbonding_period: Option<u64>,
    },
    UpdateGaugeConfig {
        wynd_gauge_contract: Option<String>,
        synergistic_wynd_gauge_contract: Option<String>
    },
    // from bToken Vault
    WithdrawRewards {},
    Mint {
        recipient: String,
        amount: Uint128
    },
    PlaceVotes {
        gauge: u64,
        votes: Option<Vec<Vote>>,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    QueryConfig {},
    QueryGaugeConfig {},
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GaugeConfigResponse {
    pub gauge_config: GaugeConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Convert {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveDelegationMsg {
    Delegate {
        /// Unbonding period in seconds
        unbonding_period: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[cfg_attr(test, derive(Default))]
pub struct Cw20InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20Coin>,
    pub mint: Option<MinterResponse>,
    pub marketing: Option<InstantiateMarketingInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VaultCw20HookMsg {
    DistributeRewards { address: Option<Addr> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VaultExecuteMsg {
    DistributeRewards { address: Addr },
}
