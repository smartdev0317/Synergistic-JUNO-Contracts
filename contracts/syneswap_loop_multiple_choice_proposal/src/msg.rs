use crate::proposal::{MultipleChoiceVote};
use cosmwasm_std::{CosmosMsg, Empty, Uint128};
use cw_utils::Duration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
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
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// Allows changing votes before the proposal expires. If this is
    /// enabled proposals will not be able to complete early as final
    /// vote information is not known until the time of proposal
    /// expiration.
    pub allow_revoting: bool,
    /// If set to true proposals will be closed if their execution
    /// fails. Otherwise, proposals will remain open after execution
    /// failure. For example, with this enabled a proposal to send 5
    /// tokens out of a DAO's treasury with 4 tokens would be closed when
    /// it is executed. With this disabled, that same proposal would
    /// remain open until the DAO's treasury was large enough for it to be
    /// executed.
    pub close_proposal_on_execution_failure: bool,

    pub dao: String,

    pub proposal_creation_token_limit: Uint128,

    pub token_hold_duration: u64,

    pub loop_staker: String,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Creates a proposal in the module.
    Propose {
        /// The title of the proposal.
        title: String,
        /// A description of the proposal.
        description: String,
        /// The messages that should be executed in response to this
        /// proposal passing.
        options: Vec<String>,
        pending_period: Duration,
        // voting_strategy: VotingStrategy,
        amount: Uint128,
        loop_gauge_proposal_id: u64,
    },
    /// Votes on a proposal. Voting power is determined by the DAO's
    /// voting power module.
    Vote {
        /// The ID of the proposal to vote on.
        proposal_id: u64,
        /// The senders position on the proposal.
        vote: Vec<MultipleChoiceVote>,
    },
    /// Causes the messages associated with a passed proposal to be
    /// executed by the DAO.
    Execute {
        /// The ID of the proposal to execute.
        proposal_id: u64,
    },
    Reject {
        /// The ID of the proposal to reject.
        proposal_id: u64,
    },
    /// Closes a proposal that has failed (either not passed or timed
    /// out). If applicable this will cause the proposal deposit
    /// associated wth said proposal to be returned.
    Close {
        /// The ID of the proposal to close.
        proposal_id: u64,
    },
    /// Updates the governance module's config.
    UpdateConfig {
        /// The new proposal passing threshold. This will only apply
        /// to proposals created after the config update.
        // threshold: Option<Threshold>,
        /// The default maximum amount of time a proposal may be voted
        /// on before expiring. This will only apply to proposals
        /// created after the config update.
        max_voting_period: Option<Duration>,
        /// The minimum amount of time a proposal must be open before
        /// passing. A proposal may fail before this amount of time has
        /// elapsed, but it will not pass. This can be useful for
        /// preventing governance attacks wherein an attacker aquires a
        /// large number of tokens and forces a proposal through.
        min_voting_period: Option<Duration>,
        min_pending_period: Option<Duration>,
        /// The address if tge DAO that this governance module is
        /// associated with.
        dao: Option<String>,
        token_hold_duration: Option<u64>,
        proposal_creation_token_limit: Option<Uint128>,
        loop_gauge: Option<String>,
        loop_staker: Option<String>,
        only_members_execute: Option<bool>,
        default_limit: Option<u64>,
        max_limit: Option<u64>,
    },

    AddMultipleChoiceOptions {
        proposal_id: u64,
        options: Vec<String>,
    },

    RemoveMultipleChoiceOptions {
        proposal_id: u64,
        options: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MultipleChoiceOptionMsg {
    pub title: String,
    pub description: String,
    pub msgs: Option<Vec<CosmosMsg<Empty>>>,
    pub address: Option<String>,
    pub pool: Option<String>,
    pub reward_token: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Info {},

    Dao {},
    /// Gets the proposal module's config.
    Config {},
    /// Gets information about a proposal.
    Proposal {
        proposal_id: u64,
    },
    /// Lists all the proposals that have been cast in this
    /// module.
    ListProposals {
        /// The proposal ID to start listing proposals after. For
        /// example, if this is set to 2 proposals with IDs 3 and
        /// higher will be returned.
        start_after: Option<u64>,
        /// The maximum number of proposals to return as part of this
        /// query. If no limit is set a max of 30 proposals will be
        /// returned.
        limit: Option<u64>,
    },
    // Lists all of the proposals that have been cast in this module in decending order of proposal ID.

    // ReverseProposals {
    //     /// The proposal ID to start listing proposals before. For
    //     /// example, if this is set to 6 proposals with IDs 5 and
    //     /// lower will be returned.
    //     start_before: Option<u64>,
    //     /// The maximum number of proposals to return as part of this
    //     /// query. If no limit is set a max of 30 proposals will be
    //     /// returned.
    //     limit: Option<u64>,
    // },
    // /// Returns the number of proposals that have been created in this
    // /// module.
    ProposalCount {},
    // /// Returns a voters position on a propsal.
    GetVote {
        proposal_id: u64,
        voter: String,
        version: Option<u128>,
    },
    // // /// Lists all of the votes that have been cast on a
    // // /// proposal.
    ListVotes {
        /// The proposal to list the votes of.
        proposal_id: u64,
        /// The voter to start listing votes after. Ordering is done
        /// alphabetically.
        start_after: Option<(String, String)>,
        /// The maximum number of votes to return in response to this
        /// query. If no limit is specified a max of 30 are returned.
        limit: Option<u64>,
    },

    ProposalVersion {
        proposal_id: u64,
    },
    // /// Gets the current proposal creation policy for this
    // /// module.

    // ProposalCreationPolicy {},
    // /// Lists all of the consumers of proposal hooks for this module.

    // ProposalHooks {},
    // /// Lists all of the consumers of vote hooks for this
    // /// module.

    // VoteHooks {},
    HoldAmount {
        address: String,
    },

    ProposalHistory {
        proposal_id: u64,
        start_after: Option<u128>,
        limit: Option<u64>,
        reverse: Option<bool>,
    },

    ProposalExecutions {
        proposal_id: u64,
        start_after: Option<u64>,
        limit: Option<u64>,
    }
}

// #[cw_serde]
// pub enum MigrateMsg {
//     FromV1 {
//         /// This field was not present in DAO DAO v1. To migrate, a
//         /// value must be specified.
//         ///
//         /// If set to true proposals will be closed if their execution
//         /// fails. Otherwise, proposals will remain open after execution
//         /// failure. For example, with this enabled a proposal to send 5
//         /// tokens out of a DAO's treasury with 4 tokens would be closed when
//         /// it is executed. With this disabled, that same proposal would
//         /// remain open until the DAO's treasury was large enough for it to be
//         /// executed.
//         close_proposal_on_execution_failure: bool,
//         /// This field was not present in DAO DAO v1. To migrate, a
//         /// value must be specified.
//         ///
//         /// This contains information about how a pre-propose module may be configured.
//         /// If set to "AnyoneMayPropose", there will be no pre-propose module and consequently,
//         /// no deposit or membership checks when submitting a proposal. The "ModuleMayPropose"
//         /// option allows for instantiating a prepropose module which will handle deposit verification and return logic.
//         pre_propose_info: PreProposeInfo,
//     },
//     FromCompatible {},
// }
