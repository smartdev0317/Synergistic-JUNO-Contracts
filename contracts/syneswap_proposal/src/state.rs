use crate::proposal::SingleChoiceProposal;
use crate::threshold::Threshold;
use crate::voting::Vote;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub const PASSED_STATUS: &str = "passed";
pub const FAILED_STATUS: &str = "failed";
pub const OPEN_STATUS: &str = "open";
pub const CLOSED_STATUS: &str = "closed";
pub const VOTING_CLOSED_STATUS: &str = "voting closed";
pub const EXECUTED_STATUS: &str = "executed";

/// A vote cast for a proposal.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Ballot {
    /// The amount of voting power behind the vote.
    pub power: Uint128,
    /// The position.
    pub vote: Vote,
}
/// The governance module's configuration.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    pub threshold: Threshold,
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// The minimum amount of time a proposal must be open before
    /// passing. A proposal may fail before this amount of time has
    /// elapsed, but it will not pass. This can be useful for
    /// preventing governance attacks wherein an attacker aquires a
    /// large number of tokens and forces a proposal through.
    pub min_voting_period: Duration,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: String,

    pub admin: String,

    pub proposal_creation_token_limit: Uint128,

    pub token_hold_duration: u64,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct PassedResponse {
    pub is_passed: bool,
    pub description: String,
}

/// The current top level config for the module.  The "config" key was
/// previously used to store configs for v1 DAOs.
pub const CONFIG: Item<Config> = Item::new("config_v2");
/// The number of proposals that have been created.
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, SingleChoiceProposal> = Map::new("proposals_v2");
pub const BALLOTS: Map<(u64, Addr), Ballot> = Map::new("ballots");
pub const PROPOSERS_INFO: Map<String, Uint128> = Map::new("Proposer Amount");
