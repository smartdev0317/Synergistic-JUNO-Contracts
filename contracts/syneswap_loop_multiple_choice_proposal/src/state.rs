use crate::proposal::MultipleChoiceOptionCustom;
use crate::proposal::MultipleChoiceProposalCustom;
use crate::proposal::MultipleChoiceVote;
use crate::status::Status;
use cosmwasm_std::Uint128;
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use cw_utils::Expiration;
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
    pub vote: MultipleChoiceVote,
    pub time: u64,
}
/// The governance module's configuration.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    // pub threshold: Threshold,
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// The minimum amount of time a proposal must be open before
    /// passing. A proposal may fail before this amount of time has
    /// elapsed, but it will not pass. This can be useful for
    /// preventing governance attacks wherein an attacker aquires a
    /// large number of tokens and forces a proposal through.
    pub min_voting_period: Duration,
    pub min_pending_period: Duration,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: String,

    pub admin: String,

    pub proposal_creation_token_limit: Uint128,

    pub token_hold_duration: u64,

    pub loop_gauge: String,

    pub loop_staker: String,

    pub only_members_execute: bool,

    pub default_limit: u64,

    pub max_limit: u64,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct PassedResponse {
    pub is_passed: bool,
    pub description: String,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalHistoryInfo {
    pub version: u128,
    pub info: ProposalHistory,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalHistoryResponse {
    pub history: Vec<ProposalHistoryInfo>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalVersionResponse {
    pub version: u128,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalExecutionsInfo {
    pub time: u64,
    pub version: u128,
    pub status: Status,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalExecusionsResponse {
    pub executions: Vec<ProposalExecutionsInfo>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalHistory {
    pub time: u64,
    pub status: Status,
    pub total_power: Uint128,
    pub choices: Vec<MultipleChoiceOptionCustom>,
    pub voting_start_time: u64,
    pub expiration: Expiration,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct ProposalExecution {
    pub version: u128,
    pub status: Status,
}

/// The current top level config for the module.  The "config" key was
/// previously used to store configs for v1 DAOs.
pub const CONFIG: Item<Config> = Item::new("config_v2");
/// The number of proposals that have been created.
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSAL_VERSION: Map<u64, u128> = Map::new("proposal_version");
pub const PROPOSALS: Map<u64, MultipleChoiceProposalCustom> = Map::new("proposals_v2");
pub const BALLOTS: Map<(String, String, String), Ballot> = Map::new("ballots");
pub const PROPOSERS_INFO: Map<String, Uint128> = Map::new("Proposer Amount");
pub const POOL_AMOUNTS: Map<(u64, u64), Uint128> = Map::new("Pool Amounts");
pub const PROPOSAL_EXECUTIONS: Map<(u64, u64), ProposalExecution> = Map::new("proposal_executions");

pub const PROPOSAL_HISTORY: Map<(u64, u128), ProposalHistory> = Map::new("proposal_history");
