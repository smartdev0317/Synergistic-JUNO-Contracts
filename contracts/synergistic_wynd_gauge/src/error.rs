use cosmwasm_std::{Decimal, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Gauge with ID {0} does not exists")]
    GaugeMissing(u64),

    #[error("Voted for {0} times total voting power. Limit 1.0")]
    TooMuchVotingWeight(Decimal),

    #[error("User {0} has no voting power")]
    NoVotingPower(String),

    #[error("Option {option} already exists for gauge ID {gauge_id}")]
    OptionAlreadyExists { option: String, gauge_id: u64 },

    #[error("Option {option} has been judged as invalid by gauge adapter of gauge ID {gauge_id}")]
    OptionInvalidByAdapter { option: String, gauge_id: u64 },

    #[error("Option {option} has been judged as valid by gauge adapter of gauge ID {gauge_id} and cannot be removed")]
    OptionValidByAdapter { option: String, gauge_id: u64 },

    #[error("Option {option} does not exists for gauge ID {gauge_id}")]
    OptionDoesNotExists { option: String, gauge_id: u64 },

    #[error("Gauge ID {gauge_id} cannot execute because next_epoch is not yet reached: current {current_epoch}, next_epoch: {next_epoch}")]
    EpochNotReached {
        gauge_id: u64,
        current_epoch: u64,
        next_epoch: u64,
    },

    #[error("Reset epoch has not passed yet")]
    ResetEpochNotPassed {},

    #[error("Gauge ID {0} cannot execute because it is stopped")]
    GaugeStopped(u64),

    #[error("Gauge ID {0} is currently pending, please try again later")]
    GaugePending(u64),

    #[error("Gauge ID {0} is currently not pending, please try again later")]
    GaugeNoPending(u64),

    #[error("Gauge ID {0} is currently resetting, please try again later")]
    GaugeResetting(u64),

    #[error("Trying to remove vote that does not exists")]
    CannotRemoveNonexistingVote {},

    #[error("Epoch size must be bigger then 60 seconds")]
    EpochSizeTooShort {},

    #[error("Minimum percent selected parameter needs to be smaller then 1.0")]
    MinPercentSelectedTooBig {},

    #[error("Maximum options selected parameter needs to be bigger then 0")]
    MaxOptionsSelectedTooSmall {},

    #[error("Maximum percentage available parameter needs to be smaller then 1.0")]
    MaxAvailablePercentTooBig {},

    #[error("Gauge ID {0} is cannot create because it is stopped")]
    GaugeCannotCreate(u64)
}
