use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Cannot set to own account")]
    CannotSetOwnAccount {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Allowance is expired")]
    Expired {},

    #[error("No allowance for this account")]
    NoAllowance {},

    #[error("Action is required")]
    NoAction {},

    #[error("Minting cannot exceed the cap")]
    CannotExceedCap {},

    #[error("Logo binary data exceeds 5KB limit")]
    LogoTooBig {},

    #[error("Invalid xml preamble for SVG")]
    InvalidXmlPreamble {},

    #[error("Invalid png header")]
    InvalidPngHeader {},

    #[error("Duplicate initial balance addresses")]
    DuplicateInitialBalanceAddresses {},

    #[error("Invalid token")]
    InvalidToken {},

    #[error("Invalid address")]
    InvalidAddr {},
    
    #[error("Got reply with error, only handle success case")]
    ErrorReply {},

    #[error("Got reply with unknown ID: {0}")]
    UnknownReply(u64),

    #[error("Undefined")]
    GenericErr {},

    #[error("Unbonding period is not set")]
    UnbondingPeriodErr {},

    #[error("Invalid parameters")]
    InvalidParams {}
}
