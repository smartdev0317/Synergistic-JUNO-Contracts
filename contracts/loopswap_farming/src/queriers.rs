use cosmwasm_std::{
    QuerierWrapper, StdResult, Uint128
};
use syneswap::{farming::{ QueryMsg, QueryUserRewardInPoolResponse, QueryRewardResponse }, asset::{StakeableToken, StakeablePairedDistributionTokenInfo}};

pub fn query_loop_farm_stakable_token (
    querier: QuerierWrapper,
    contract_addr: String,
    pool_addr: String,
 ) -> StdResult<StakeableToken> {
    let flp_token_address: String = querier.query_wasm_smart(contract_addr.clone(), &QueryMsg::QueryFlpTokenFromPoolAddress { pool_address: pool_addr.clone() })?;
    let distribute_config: Vec<QueryRewardResponse> = querier.query_wasm_smart(contract_addr.clone(), &QueryMsg::QueryListOfDistributableTokensByPool { pool: pool_addr.clone() })?;
    let mut distribution: Vec<StakeablePairedDistributionTokenInfo> = vec![];
    for distribute in distribute_config.into_iter() {
        distribution.push(
            StakeablePairedDistributionTokenInfo {
                token: distribute.info,
                amount: distribute.daily_reward,
                reserve_amount: Uint128::zero()
            }
        );
    };
    Ok(
        StakeableToken {
            liquidity_token: flp_token_address,
            token: pool_addr,
            distribution: distribution
        }
    )
}

pub fn query_loop_farm_pool_rewards (
    querier: QuerierWrapper,
    contract_addr: String,
    pool: String
) -> StdResult<Vec<QueryRewardResponse>> {
    querier.query_wasm_smart(contract_addr.clone(), &QueryMsg::QueryListOfDistributableTokensByPool { pool: pool })
}

pub fn query_loop_farm_pending_rewards (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    account: String,
    pool_addr: String,
) -> StdResult<Vec<(String, Uint128)>> {
    let query_response: StdResult<Vec<QueryUserRewardInPoolResponse>> = querier.query_wasm_smart(contract_addr, &QueryMsg::QueryUserRewardInPool { wallet: account, pool: pool_addr.clone() });
    match query_response {
        Err(e) => Err(e),
        Ok(user_reward_in_pool) => {
            let reward_data = user_reward_in_pool.into_iter().find(|data| data.pool == pool_addr.clone()).unwrap();
            Ok(reward_data.rewards_info.clone())
        }
    }
}

pub fn query_loop_farm_reward_in_pool (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    pool: String,
    distribution_token: String
 ) -> StdResult<Uint128> {
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryRewardInPool { pool, distribution_token })
}

pub fn query_loop_farm_staked_by_user (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    wallet: String,
    staked_token: String
 ) -> StdResult<Uint128> {
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryStakedByUser { wallet, staked_token })
}

pub fn query_loop_farm_distribution_wait_time (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
 ) -> StdResult<u64> {
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryDistributionWaitTime {})
}

pub fn query_loop_farm_lock_time_frame (
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
 ) -> StdResult<u64> {
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryLockTimeFrame {})
}

pub fn query_loop_farm_last_distribution_time(
    querier: QuerierWrapper,
    contract_addr: impl Into<String>,
    pool_address: String,
) -> StdResult<u64> {
    querier.query_wasm_smart(contract_addr, &QueryMsg::QueryLastDistributionTime { pool_address })
}
