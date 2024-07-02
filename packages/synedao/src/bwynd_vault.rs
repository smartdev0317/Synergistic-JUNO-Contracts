use cosmwasm_schema::{cw_serde};
use cosmwasm_std::{Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// address of bwynd token
    pub token: String,
    // 1 000 000
    pub wynd_staking_module: String,
    pub min_bond: Uint128,
}