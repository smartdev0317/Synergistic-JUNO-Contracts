//#![warn(missing_docs)]
#![doc(html_logo_url = "../../../uml/logo.png")]
//! # SYNEDEX Staking
//!
//! ## Description
//!
//! We need a project that allow LP tokens to be staked.
//!
//! ## Objectives
//!
//! The main goal of the **SYNEDEX staking** is to:
//!   - Allow the LP TOKEN to be staked with a proper curve and time.
//!

/// Main contract logic
pub mod contract;

/// custom error handler
mod error;

/// custom input output messages
pub mod msg;

/// state on the blockchain
pub mod state;

pub mod queriers;

pub use crate::error::ContractError;
