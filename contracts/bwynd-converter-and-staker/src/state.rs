use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::{Item};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub cw20_code_id: u64,
    pub vault_code_id: u64, 
    pub wynd_token: Addr,
    pub wynd_staking_module: Addr,
    pub bwynd: Option<Addr>,
    pub bwynd_vault: Option<Addr>,
    pub unbonding_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct GaugeConfig {
    pub wynd_gauge_contract: String,
    pub synergistic_wynd_gauge_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct Vote {
    /// Option voted for.
    pub option: String,
    /// The weight of the power given to this vote
    pub weight: Decimal,
}

// put the length bytes at the first for compatibility with legacy singleton store
pub const CONFIG: Item<Config> = Item::new("config");
pub const GAUGE_CONFIG: Item<GaugeConfig> = Item::new("gauge_config");
