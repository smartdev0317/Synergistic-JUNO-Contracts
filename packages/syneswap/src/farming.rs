use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::StakeableToken;

use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    // Token contract code id for initialization
    pub treasury_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<String>,
    },
    UpdateTreasuryAddr {
        treasury_addr: String,
    },
    UpdateTreasuryFee {
        treasury_fee: u64,
    },
    ClaimReward {
        pool_address: String,
        start_after: Option<String>,
    },
    UnstakeAndClaim {
        pool_address: String,
        amount: Uint128,
        start_after: Option<String>
    },
    AddStakeableToken {
        pool_address: String,
        liquidity_token: String,
    },
    AddStakeableTokens {
        pool_addresses: Vec<String>,
        liquidity_tokens: Vec<String>,
    },
    DistributeByLimit {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    WithdrawTreasuryReward {
        token: String,
        amount: Uint128
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoopFarmExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<String>,
    },
    UpdateTreasuryAddr {
        treasury_addr: String,
    },
    UpdateTreasuryFee {
        treasury_fee: u64,
    },
    ClaimReward {
        pool_address: String,
    },
    AddStakeableToken {
        pool_address: String,
        liquidity_token: String,
    },
    AddStakeableTokens {
        pool_addresses: Vec<String>,
        liquidity_tokens: Vec<String>,
    },
    DistributeByLimit {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    QueryRewardInPool {
        pool: String,
        distribution_token: String,
    },
    QueryStakedByUser {
        wallet: String,
        staked_token: String,
    },
    QueryTotalStaked {
        staked_token: String,
    },
    QueryListOfStakeableTokens {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    QueryListOfDistributableTokensByPool {
        pool: String,
    },
    // QueryStakeableInfo {
    //     start_after: Option<String>,
    //     limit: Option<u32>,
    // },
    QueryUserRewardInPool {
        wallet: String,
        pool: String,
    },
    QueryUserStakedTime {
        wallet: String,
        pool: String,
    },
    QueryDistributionWaitTime {},
    QueryLockTimeFrame {},
    QueryLastDistributionTime {
        pool_address: String,
    },
    // QuerySecondAdminAddress {},
    // QueryTotalDistributedAmountInPool {
    //     pool: String,
    //     dist_token_addr: String,
    // },
    QueryGetDistributeableTokenBalance {
        dist_token_addr: String,
    },
    QueryGetUserAutoCompoundSubription {
        user_address: String,
        pool_address: String,
    },
    // QueryLockTimeFrameForAutoCompound {},
    QueryGetTotalCompounded {
        pool_addr: String,
    },
    QueryFlpTokenFromPoolAddress { 
        pool_address: String,
    },
    QueryTreasuryAddress {},
    QueryTreasuryFee {},
    QueryFeeMultiplier {},
    TreasuryReward {
        token: String,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryRewardResponse {
    pub info: String,
    pub daily_reward: Uint128,
    //pub locked_for_distribution: Uint128,
}
// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryUserRewardInPoolResponse {
    pub pool: String,
    pub rewards_info: Vec<(String, Uint128)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StakeableResponse {
    pub stakes: Vec<StakeableToken>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Stake a given amount of asset
    Stake { start_after: Option<String> },

    // UnstakeWithoutClaim{ start_after: Option<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoopFarmCw20HookMsg {
    /// Stake a given amount of asset
    Stake {},

    UnstakeAndClaim{},

    UnstakeWithoutClaim{},
}
