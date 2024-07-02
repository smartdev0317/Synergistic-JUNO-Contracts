use cosmwasm_schema::cw_serde;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_controllers::{Admin};
use cw_storage_plus::{Item, Map};


#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// address of cw20 contract token to stake
    pub token: Addr,
    pub bloop_converter_and_staker: Addr,
    pub min_bond: Uint128,
    pub loop_protocol_staking: Addr,
    pub treasury_wallet: Option<Addr>,
    pub treasury_withdrawer: Option<Addr>,
    pub syne_staking_reward_distributor: Option<Addr>,
    pub treasury_fee: Decimal,
    pub syne_staking_fee: Decimal,
    pub total_fee_cap: Decimal,
    pub treasury_fee_limit: Decimal,
    pub duration: u64,
}

pub const ADMIN: Admin = Admin::new("admin");
pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Default, Serialize, Deserialize)]
pub struct TokenInfo {
    pub staked: Uint128,
    pub pending_treasury_rewards: Uint128,
    pub pending_syne_staking_rewards: Uint128,
    pub power: Decimal,
}

#[derive(Default, Serialize, Deserialize)]
pub struct StakingInfo {
    pub stake: Uint128,
    pub power_diff: Decimal,
    pub syne_power: Decimal,
}

#[derive(Serialize, Deserialize)]
pub enum RewardAction {
    NoAction {},
    Reward {
        address: Addr,
    },
    Stake {
        address: Addr,
        amount: Uint128,
    },
    TreasuryWithdraw {
        amount: Option<Uint128>,
    },
    SyneStakingRewardWithdraw {
        amount: Option<Uint128>,
    },
    Unstake {
        address: Addr,
        amount: Uint128,
    },
}

pub const TOTAL_STAKED: Item<TokenInfo> = Item::new("total_staked");
pub const STAKE: Map<&Addr, StakingInfo> = Map::new("stake");
pub const REWARD_ACTION: Item<RewardAction> = Item::new("reward_action");
// for syne reward upgrading
#[cw_serde]
pub struct SyneDistributionConfig {
    pub syne_addr: String,
    pub distribution_per_day: u64,
}

#[cw_serde]
pub struct CurrentSyneDistribution {
    pub total_distributed: Uint128,
    pub pending: Uint128,
    pub last_distributed_time: u64,
    pub power: Decimal,
}

pub const SYNE_DISTRIBUTION_CONFIG: Item<SyneDistributionConfig> = Item::new("syne_distribution_config");
pub const CURRENT_SYNE_DISTRIBUTION: Item<CurrentSyneDistribution> = Item::new("current_syne_distribution");
