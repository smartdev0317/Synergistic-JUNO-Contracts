use cosmwasm_std::{
    QuerierWrapper, StdResult, Uint128
};
use cosmwasm_schema::{ cw_serde };

#[cw_serde]
pub struct WithdrawableRewardsResponse {
    /// Amount of rewards assigned for withdrawal from the given address.
    pub rewards: Uint128,
}

#[cw_serde]
pub enum QueryMsg {
    /// Return how many rewards are assigned for withdrawal from the given address. Returns
    /// `RewardsResponse`.
    WithdrawableRewards {
        owner: String,
    },
}

pub fn query_wynd_staking_module_rewards (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    account: impl Into<String>,
 ) -> StdResult<WithdrawableRewardsResponse> {
    let query_response: WithdrawableRewardsResponse =
    querier.query_wasm_smart(contract_addr, &QueryMsg::WithdrawableRewards { owner: account.into() })?;
    Ok(query_response)
}