use cosmwasm_std::{Addr};
use cw_storage_plus::{Item};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub cw20_code_id: u64,
    pub vault_code_id: u64, 
    pub loop_token: Addr,
    pub loop_protocol_staking: Addr,
    pub duration: u64,
    pub bloop_token: Option<Addr>,
    pub bloop_vault: Option<Addr>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MultipleChoiceVote {
    // A vote indicates which option the user has selected.
    pub pool: String,
    pub percentage: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct GaugeConfig {
    pub loop_gauge_contract: String,
    pub synergistic_loop_gauge_contract: Option<String>,
}

// put the length bytes at the first for compatibility with legacy singleton store
pub const CONFIG: Item<Config> = Item::new("config");
pub const GAUGE_CONFIG: Item<GaugeConfig> = Item::new("gauge_config");
