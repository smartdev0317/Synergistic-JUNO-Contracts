use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    /// address of bloop token
    pub token: String,
    // 1 000 000
    pub loop_protocol_staking: String,
    pub min_bond: Uint128,
}
