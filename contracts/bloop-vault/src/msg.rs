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
    DistributeRewards {
        address: Option<Addr>
    }
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
    // /// Claims shows the tokens in process of unbonding for this address
    // #[returns(cw_controllers::ClaimsResponse)]
    // Claims { address: String },
    // /// Show the number of tokens currently staked by this address.
    // #[returns(StakedResponse)]
    // Staked {
    //     address: String,
    //     /// Unbonding period in seconds
    //     unbonding_period: u64,
    // },
    // /// Show the number of tokens currently staked by this address for all unbonding periods
    // #[returns(AllStakedResponse)]
    // AllStaked { address: String },
    // /// Show the number of all, not unbonded tokens delegated by all users for all unbonding periods
    // #[returns(TotalStakedResponse)]
    // TotalStaked {},
    // /// Show the number of all tokens being unbonded for all unbonding periods
    // #[returns(TotalUnbondingResponse)]
    // TotalUnbonding {},
    // /// Show the total number of outstanding rewards
    // #[returns(RewardsPowerResponse)]
    // TotalRewardsPower {},
    // /// Show the outstanding rewards for this address
    // #[returns(RewardsPowerResponse)]
    // RewardsPower { address: String },
    // /// Return AdminResponse
    // #[returns(cw_controllers::AdminResponse)]
    // Admin {},
    // #[returns(BondingInfoResponse)]
    // BondingInfo {},

    // /// Return how many rewards will be received per token in each unbonding period in one year
    // #[returns(AnnualizedRewardsResponse)]
    // AnnualizedRewards {},
    // /// Return how many rewards are assigned for withdrawal from the given address. Returns
    // /// `RewardsResponse`.
    // #[returns(WithdrawableRewardsResponse)]
    // WithdrawableRewards { owner: String },
    // /// Return how many rewards were distributed in total by this contract. Returns
    // /// `RewardsResponse`.
    // #[returns(DistributedRewardsResponse)]
    // DistributedRewards {},
    // /// Return how many funds were sent to this contract since last `ExecuteMsg::DistributeFunds`,
    // /// and await for distribution. Returns `RewardsResponse`.
    // #[returns(UndistributedRewardsResponse)]
    // UndistributedRewards {},
    // /// Return address allowed for withdrawal of the funds assigned to owner. Returns `DelegatedResponse`
    // #[returns(DelegatedResponse)]
    // Delegated { owner: String },
    // /// Returns rewards distribution data
    // #[returns(DistributionDataResponse)]
    // DistributionData {},
    // /// Returns withdraw adjustment data
    // #[returns(WithdrawAdjustmentDataResponse)]
    // WithdrawAdjustmentData { addr: String, asset: AssetInfo },
}

#[cw_serde]
pub struct MigrateMsg {
    // /// Address of the account that can call [`ExecuteMsg::QuickUnbond`]
    // pub unbonder: Option<String>,
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
    pub bloop_converter_and_staker: Addr,
    pub loop_protocol_staking: Addr,
    pub min_bond: Uint128,
    pub treasury_wallet: Option<Addr>,
    pub treasury_withdrawer: Option<Addr>,
    pub treasury_fee: Decimal,
    pub syne_staking_reward_distributor: Option<Addr>,
    pub syne_staking_fee: Decimal,
    pub duration: u64,
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
