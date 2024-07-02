use cosmwasm_std::{
    QuerierWrapper, StdResult, Uint128
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct WithdrawableRewardsResponse {
    /// Amount of rewards assigned for withdrawal from the given address.
    pub rewards: Uint128,
}


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Return how many rewards are assigned for withdrawal from the given address. Returns
    /// `RewardsResponse`.
    WithdrawableRewards {
        owner: String,
    },
}

pub fn query_wynd_dao_core_rewards (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    account: impl Into<String>,
 ) -> StdResult<WithdrawableRewardsResponse> {
    let query_response: WithdrawableRewardsResponse =
    querier.query_wasm_smart(contract_addr, &QueryMsg::WithdrawableRewards { owner: account.into() })?;
    Ok(query_response)
}