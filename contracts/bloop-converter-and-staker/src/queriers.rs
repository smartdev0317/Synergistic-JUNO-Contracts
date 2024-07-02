use cosmwasm_std::{
    QuerierWrapper, StdResult, Uint128
};
use cosmwasm_schema::{ cw_serde };

#[cw_serde]
pub struct UserRewardResponse {
    pub user_reward: Uint128,
    pub calculated_days_of_reward: u64,
    pub pending_reward: Uint128,
}

#[cw_serde]
pub enum QueryMsg {
    /// Return how many rewards are assigned for withdrawal from the given address. Returns
    /// `RewardsResponse`.
    QueryUserReward {
        wallet: String,
        duration: u64,
    },
}

pub fn query_loop_protocol_staking_rewards (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    account: impl Into<String>,
 ) -> StdResult<UserRewardResponse> {
    let query_response: UserRewardResponse =
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryUserReward { wallet: account.into(), duration: 12 }).unwrap_or(UserRewardResponse {
        user_reward: Uint128::zero(),
        calculated_days_of_reward: 0,
        pending_reward: Uint128::zero()
    });
    Ok(query_response)
}