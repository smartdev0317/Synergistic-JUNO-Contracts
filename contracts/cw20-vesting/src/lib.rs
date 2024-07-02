/// cw-20 allowance
pub mod allowances;

/// Main cw-20 Module
pub mod contract;

/// paginated query Module
pub mod enumerable;

/// custom error handler
mod error;

/// custom input output messages
pub mod msg;

/// state on the blockchain
pub mod state;
pub use crate::error::ContractError;
pub use crate::msg::{InitBalance, InstantiateMsg, MinterInfo};
