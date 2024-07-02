use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Uint128, Addr, Decimal};
use cw_storage_plus::{Item, Map};
use syneswap::asset::StakeableToken;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub loop_farm_contract: Addr,
    pub treasury_addr: Addr,
    pub treasury_fee: u64,
    pub fee_multiplier: u64,
    pub default_limit: u32,
    pub max_limit: u32
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RewardInfo {
    pub pool_reward_weight: Decimal,
    pub pending_reward: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CurrentUnstakeInfo {
    pub sender: String,
    pub pool_address: String,
    pub amount: Uint128,
    pub current_pending_rewards: Vec<(String, Uint128)>
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CurrentStakeInfo {
    pub sender: String,
    pub amount: Uint128,
    pub pool_address: String,
    pub current_pending_rewards: Vec<(String, Uint128)>
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CurrentClaimRewardInfo {
    pub account: String,
    pub pool_address: String,
    pub current_pending_rewards: Vec<(String, Uint128)>
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct UserAction {
    pub action: u64,
    pub account: String,
    pub pool_address: String,
    pub amount: Option<Uint128>,
    pub flp_token_address: Option<String>,
    pub is_reward_claimed: Option<bool>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct TreasuryRewardsInfo {
    pub token: String,
    pub amount: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const STAKEABLE_INFOS: Map<String, StakeableToken> = Map::new("stakeableInfos");
pub const UNCLAIMED_DISTRIBUTED_TOKEN_AMOUNT_MAP: Map<String, Uint128> =
    Map::new("unclaimedDistributedTokenAmountMap");
pub const USER_STAKED_AMOUNT: Map<String, Uint128> = Map::new("rewardTokenIssued");
pub const TOTAL_STAKED: Map<String, Uint128> = Map::new("totalStaked");
pub const TOTAL_REWARDS_IN_POOL: Map<String, Uint128> = Map::new("totalRewardsInPool");
pub const TOTAL_ACCUMULATED_DISTRIBUTED_AMOUNT_IN_POOL_MAP: Map<String, Uint128> =
    Map::new("totalAccumulatedDistributedTokenAmountMapInPools");
pub const POOL_REWARD_INDEX_MAP: Map<String, Uint128> = Map::new("rewardIndexMap");
pub const POOL_REWARD_WEIGHT_MAP: Map<String, Uint128> = Map::new("rewardWeightMap");
pub const USER_REWARD_INFO_MAP: Map<String, RewardInfo> = Map::new("userRewardInfoxMap");
pub const USER_REWARD_STARTING_TIME_MAP: Map<String, u64> = Map::new("userRewardStartingTimeMap");
pub const POOL_LAST_DISTRIBUTION_TIME_IN_SECONDS: Map<String, u64> =
    Map::new("liquidityAndDevTokenMap");
pub const USER_AUTO_COMPOUND_SUBSCRIPTION_MAP: Map<String, bool> =
    Map::new("UserAutoCompoundSubscriptionMap");
pub const POOL_TOTAL_COMPOUNDED_AMOUNT: Map<String, Uint128> = Map::new("totalCompoundedStaked");
pub const POOL_COMPOUNDED_INDEX_MAP: Map<String, Uint128> = Map::new("compoundedIndexMap");
pub const USER_COMPOUNDED_REWARD_INFO_MAP: Map<String, RewardInfo> =
    Map::new("userCompoundedInfoxMap");
pub const CURRENT_POOL_ADDRESS: Item<String> = Item::new("currentPoolAddress");
pub const LIQUIDITY_TOKEN_MAP: Map<String, String> = Map::new("liquidityTokenMap");
pub const LAST_CLAIMED_REWARD_TIME: Map<String, u64> = Map::new("lastClaimedRewardTime");
pub const CURRENT_UNSTAKE_INFO: Item<CurrentUnstakeInfo> = Item::new("currentUnstakeInfo");
pub const CURRENT_STAKE_INFO: Item<CurrentStakeInfo> = Item::new("currentStakeInfo");
pub const CURRENT_CLAIM_REWARD_INFO: Item<CurrentClaimRewardInfo> = Item::new("currentClaimRewardInfo");
pub const PENDING_REWARDS: Map<String, Vec<(String, Uint128)>> = Map::new("pendingReward");
pub const TOTAL_REWARDS: Map<String, Vec<(String, Uint128)>> = Map::new("totalReward");
pub const TOTAL_REWARDS_WEIGHT: Map<String, Vec<(String, Decimal)>> = Map::new("totalRewardWeight");
// store user's action for using after distribute rewards
pub const USER_ACTION: Item<UserAction> = Item::new("userAction");
// store treasury rewards
pub const TREASURY_REWARDS: Map<String, Uint128> = Map::new("treasuryRewards");

// for syne reward upgrading
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct SyneDistributionConfig {
    pub syne_addr: String,
    pub distribution_per_day: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CurrentSyneDistribution {
    pub total_distributed: Uint128,
    pub pending: Uint128,
    pub last_distributed_time: u64,
    pub power: Decimal,
}

pub const SYNE_DISTRIBUTION_CONFIG: Item<SyneDistributionConfig> = Item::new("syne_distribution_config");
pub const CURRENT_SYNE_DISTRIBUTION: Item<CurrentSyneDistribution> = Item::new("current_syne_distribution");
