use cosmwasm_std::{Coin, OverflowError, StdError, Uint128};
use thiserror::Error;

use cw_controllers::{AdminError, HookError};
use syne_curve_utils::CurveError;
use synedex::asset::AssetInfoValidated;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Admin(#[from] AdminError),

    #[error("{0}")]
    Hook(#[from] HookError),

    #[error("{0}")]
    Curve(#[from] CurveError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Cannot rebond to the same unbonding period")]
    SameUnbondingRebond {},

    #[error("Rebond amount is invalid")]
    NoRebondAmount {},

    #[error("No claims that can be released currently")]
    NothingToClaim {},

    #[error(
        "Sender's CW20 token contract address {got} does not match one from config {expected}"
    )]
    Cw20AddressesNotMatch { got: String, expected: String },

    #[error("Trying to mass delegate {total} tokens, but only sent {amount_sent}.")]
    MassDelegateTooMuch {
        total: Uint128,
        amount_sent: Uint128,
    },

    #[error("No funds sent")]
    NoFunds {},

    #[error("No data in ReceiveMsg")]
    NoData {},

    #[error("No unbonding period found: {0}")]
    NoUnbondingPeriodFound(u64),

    #[error("No members to distribute tokens to")]
    NoMembersToDistributeTo {},

    #[error("There already is a distribution for {0}")]
    DistributionAlreadyExists(AssetInfoValidated),

    #[error("Cannot distribute the staked token")]
    InvalidAsset {},

    #[error("No distribution flow for this token: {0}")]
    NoDistributionFlow(Coin),

    #[error("Cannot add more than {0} distributions")]
    TooManyDistributions(u32),

    #[error("Cannot create new distribution after someone staked")]
    ExistingStakes {},

    #[error("Invalid distribution rewards")]
    InvalidRewards {},

    #[error("No reward duration provided for rewards distribution")]
    ZeroRewardDuration {},
    
    #[error("Got reply with error, only handle success case")]
    ErrorReply {},

    #[error("Got reply with unknown ID: {0}")]
    UnknownReply(u64),

    #[error("Invalid amount")]
    InvalidAmount {},

    #[error("No withdrawable amont")]
    NoWithdrawable {},

    #[error("Previos action is not completed")]
    InvalidAction {},

    #[error("The sum of treasury and syne staking fee must be equal to total fee")]
    InvalidFee {},

    #[error("The treasury or syne staking reward fee rate is invalid")]
    InvalidFeeRate {},

    #[error("Syne staking reward distributor is not defined")]
    InvalidDistributor {},
}

impl From<OverflowError> for ContractError {
    fn from(e: OverflowError) -> Self {
        ContractError::Std(e.into())
    }
}
