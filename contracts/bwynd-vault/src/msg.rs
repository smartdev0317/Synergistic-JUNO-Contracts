use cosmwasm_schema::{cw_serde, QueryResponses};
use cw20::Cw20ReceiveMsg;

use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub enum ExecuteMsg {
    /// Change the admin
    UpdateAdmin { admin: Option<String> },
    
    UpdateConfig {
        min_bond: Option<Uint128>,
        treasury_wallet: Option<String>,
        treasury_withdrawer: Option<String>,
        syne_staking_reward_distributor: Option<String>,
        treasury_fee: Option<Decimal>,
        syne_staking_fee: Option<Decimal>,
    },

    /// This accepts a properly-encoded ReceiveMsg from a cw20 contract
    Receive(Cw20ReceiveMsg),

    /// Withdraws rewards which were previously distributed and assigned to sender.
    WithdrawRewards {
        address: String,
    },

    WithdrawTreasuryRewards {
        amount: Option<Uint128>,
    },

    WithdrawSyneStakingRewards {
        amount: Option<Uint128>,
    },

    Unstake {
        amount: Uint128,
    },

    DistributeRewards {
        address: Addr,
    }
}

#[cw_serde]
pub enum Cw20HookMsg {
    Stake {},
    DistributeRewards { address: Option<Addr> }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(AdminResponse)]
    Admin {},
    #[returns(ConfigResponse)]
    Config {},
    /// Show the number of tokens currently staked.
    #[returns(TotalStakedResponse)]
    TotalStake {},
    /// Show the number of tokens currently staked by this address
    #[returns(StakedResponse)]
    Stake { address: String },
    #[returns(RewardResponse)]
    Reward { address: String },
    #[returns(RewardResponse)]
    TotalPendingReward {},
    #[returns(RewardResponse)]
    TreasuryReward {},
    #[returns(RewardResponse)]
    SyneStakingReward {},
}

#[cw_serde]
pub struct MigrateMsg {
}

#[cw_serde]
pub enum WithdrawMsg {
    WithdrawRewards {},
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: Addr,
}

#[cw_serde]
pub struct ConfigResponse {
    pub token: Addr,
    pub bwynd_converter_and_staker: Addr,
    pub wynd_staking_module: Addr,
    pub min_bond: Uint128,
    pub treasury_wallet: Option<Addr>,
    pub treasury_withdrawer: Option<Addr>,
    pub treasury_fee: Decimal,
    pub syne_staking_reward_distributor: Option<Addr>,
    pub syne_staking_fee: Decimal,
}

#[cw_serde]
pub struct StakedResponse {
    pub stake: Uint128,
    pub power_diff: Decimal,
}

#[cw_serde]
pub struct TotalStakedResponse {
    pub total_staked: Uint128,
    pub power: Decimal,
    pub pending_treasury_rewards: Uint128
}

#[cw_serde]
pub struct RewardResponse {
    pub rewards: Uint128,
}
