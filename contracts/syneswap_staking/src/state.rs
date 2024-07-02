use cosmwasm_std::{Addr, Uint128};
use cw20::{AllowanceResponse, Logo, MarketingInfoResponse};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub token_addr: Addr,
    pub owner_addr: Addr,
    pub community_addr: Option<Addr>,
    pub last_distributed: u64,
    pub freeze: bool,
    pub freeze_lock_time: u64,
    pub freeze_start_time: u64,
    pub lock_time_frame: u64,
    pub wait_time_for_distribution_in_seconds: u64,
    pub restake_reset_flag: bool,
    pub syne_power_constant: u128,
    pub latest_syne_power_date: u64,
    pub day_factor_in_seconds: u64,
    pub vault_address: String,
    pub duration_values_vector: Vec<u64>,
    pub last_syne_power_date: u64,
    pub second_owner: Option<String>,
    // pub total_user_days: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub reward_index: Uint128,
    pub pending_reward: Uint128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SynePowerIndex {
    pub first_reward_index: Uint128,
    pub last_reward_index: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserRewardResponse {
    pub user_reward: Uint128,
    pub calculated_days_of_reward: u64,
    pub pending_reward: Uint128,
    //     pub start_time: u64,
    //     pub end_time: u64,
    //     pub last_claimed_time: u64,
    //     pub mint_time: u64,
    //     pub initial_start_time: u64,
    //     pub latest_syne_power_date: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserStakedTimeResponse {
    pub staked_time: u64,
    pub last_claimed_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserStakedTime {
    pub user_staked_time: u64,
    pub user_start_time: u64,
    pub user_end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolRewardIndex {
    pub pool_reward_index: Uint128,
}

// put the length bytes at the first for compatibility with legacy singleton store
pub const CONFIG: Item<Config> = Item::new("config");
// pub const TOTAL_ACTUAL_STAKED: Item<Uint128> = Item::new("total_actual_staked");
pub const TOTAL_STAKED_DURATION_WISE: Map<u64, Uint128> = Map::new("total_staked_duration");
pub const TOTAL_REWARD: Item<Uint128> = Item::new("total_reward");
pub const TOTAL_REWARD_IN_CONTRACT: Item<Uint128> = Item::new("total_reward_in_contract");
pub const DISTRIBUTION_REWARD: Item<Uint128> = Item::new("distribution_reward");
pub const REWARD_INDEX: Item<PoolRewardIndex> = Item::new("reward_index");
// pub const STAKED_AMOUNT_TO_BE_ADDED: Map<String, Uint128> = Map::new("STAKED_AMOUNT_TO_BE_ADDED");
// pub const USER_STAKED: Map<(String, u64), Uint128> = Map::new("userStaked");
pub const USER_REWARD_INFO: Map<(String, u64), RewardInfo> = Map::new("userRewardInfo");
// pub const USER_STAKED_TIME: Map<String, UserStakedTime> = Map::new("userStakedTime");
pub const SYNE_POWER_DATE_WISE_MAP: Map<u64, SynePowerIndex> = Map::new("synePowerDateWiseMap");
// pub const PREVIOUS_DAYS_STAKED_AMOUNT: Map<u64, Uint128> = Map::new("PREVIOUS_DAYS_STAKED_AMOUNT");
pub const VAULT_AMOUNT: Item<Uint128> = Item::new("vault amount");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: Uint128,
    pub mint: Option<MinterData>,
}
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub balance: Uint128,
    pub actual_balance: Uint128,
    pub mint_time: u64,
    pub last_claimed_time: u64,
}
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalanceInfo {
    pub balance: Uint128,
    pub mint_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MinterData {
    pub minter: Addr,
    /// cap is how many more tokens can be issued by the minter
    pub cap: Option<Uint128>,
}

impl TokenInfo {
    pub fn get_cap(&self) -> Option<Uint128> {
        self.mint.as_ref().and_then(|v| v.cap)
    }
}

pub const TOKEN_INFO: Item<TokenInfo> = Item::new("token_info");
pub const MARKETING_INFO: Item<MarketingInfoResponse> = Item::new("marketing_info");
pub const LOGO: Item<Logo> = Item::new("logo");
pub const TOTAL_BALANCES: Map<u64, BalanceInfo> = Map::new("balance");
pub const BALANCES: Map<&Addr, Uint128> = Map::new("balance");
pub const ALLOWANCES: Map<(&Addr, &Addr), AllowanceResponse> = Map::new("allowance");
// TODO: After https://github.com/CosmWasm/cw-plus/issues/670 is implemented, replace this with a `MultiIndex` over `ALLOWANCES`
pub const ALLOWANCES_SPENDER: Map<(&Addr, &Addr), AllowanceResponse> =
    Map::new("allowance_spender");
pub const MINT_TIME: Map<(&Addr, u64), UserInfo> = Map::new("mint_time");
// pub const LOCK_TIME: Item<Vec<u64>> = Item::new("lock_time");