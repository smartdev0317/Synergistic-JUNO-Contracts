use crate::query::ProposalResponse;
use crate::state::PROPOSAL_COUNT;
use crate::state::{
    PassedResponse,
};
use crate::status::Status;
use crate::threshold::{Threshold};
use crate::voting::{does_vote_count_pass, Votes};
use cosmwasm_std::{
    Addr, BlockInfo, CosmosMsg, Empty, StdResult, Storage, Uint128,
};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct SingleChoiceProposal {
    pub title: String,
    pub description: String,
    /// The address that created this proposal.
    pub proposer: Addr,
    /// The minimum amount of time this proposal must remain open for
    /// voting. The proposal may not pass unless this is expired or
    /// None.
    // pub min_voting_period: Option<Expiration>,
    /// The the time at which this proposal will expire and close for
    /// additional votes.
    pub expiration: Expiration,
    /// The threshold at which this proposal will pass.
    pub threshold: Threshold,
    /// The total amount of voting power at the time of this
    /// proposal's creation.
    // pub total_power: Uint128,
    /// The messages that will be executed should this proposal pass.
    pub msgs: Vec<CosmosMsg<Empty>>,
    pub status: Status,
    pub votes: Votes,
    pub allow_revoting: bool,
    /// The total amount of voting power at the time of this
    /// proposal's creation.
    pub total_power: Uint128,
    pub voting_start_time: u64,
}

pub fn advance_proposal_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1;
    PROPOSAL_COUNT.save(store, &id)?;
    Ok(id)
}

impl SingleChoiceProposal {
    /// Consumes the proposal and returns a version which may be used
    /// in a query response. The difference being that proposal
    /// statuses are only updated on vote, execute, and close
    /// events. It is possible though that since a vote has occured
    /// the proposal expiring has changed its status. This method
    /// recomputes the status so that queries get accurate
    /// information.
    pub fn into_response(mut self, block: &BlockInfo, id: u64, votes: usize) -> ProposalResponse {
        self.update_status(block);
        ProposalResponse { id, proposal: self, votes }
    }

    /// Gets the current status of the proposal.
    pub fn current_status(&self, block: &BlockInfo) -> Status {
        if self.status == Status::Open {
            if self.expiration.is_expired(block) && !self.is_passed(block).is_passed {
                return Status::Rejected;
            } else if self.is_passed(block).is_passed {
                return Status::Passed;
            }
            self.status.clone()
        } else {
            self.status.clone()
        }
    }

    /// Sets a proposals status to its current status.
    pub fn update_status(&mut self, block: &BlockInfo) {
        let new_status = self.current_status(block);
        self.status = new_status
    }

    /// Returns true iff this proposal is sure to pass (even before
    /// expiration if no future sequence of possible votes can cause
    /// it to fail).
    pub fn is_passed(&self, block: &BlockInfo) -> PassedResponse {
        // If re-voting is allowed nothing is known until the proposal
        // has expired.
        if self.allow_revoting && !self.expiration.is_expired(block) {
            return PassedResponse {
                is_passed: false,
                description: "".to_string(),
            };
        }
        // If the min voting period is set and not expired the
        // proposal can not yet be passed. This gives DAO members some
        // time to remove liquidity / scheme on a recovery plan if a
        // single actor accumulates enough tokens to unilaterally pass
        // proposals.

        // if let Some(min) = self.min_voting_period {
        //     if !min.is_expired(block) {
        //         return false;
        //     }
        // }

        match self.threshold {
            Threshold::AbsolutePercentage { percentage } => {
                if !self.expiration.is_expired(block) {
                    return PassedResponse {
                        is_passed: false,
                        description: "Voting time is not expired yet".to_string(),
                    };
                }

                let options = self.votes.total() - self.votes.abstain;
                if does_vote_count_pass(self.votes.yes, options, self.total_power, percentage) {
                    return PassedResponse {
                        is_passed: true,
                        description: "".to_string(),
                    };
                }
                return PassedResponse {
                    is_passed: false,
                    description: "".to_string(),
                };
            }
            Threshold::ThresholdQuorum { threshold, quorum } => {
                if !does_vote_count_pass(
                    self.votes.total(),
                    self.total_power,
                    self.total_power,
                    quorum,
                ) {
                    return PassedResponse {
                        is_passed: false,
                        description: "".to_string(),
                    };
                }

                if self.expiration.is_expired(block) {
                    // If the quorum is met and the proposal is
                    // expired the number of votes needed to pass a
                    // proposal is compared to the number of votes on
                    // the proposal.
                    let options = self.votes.total() - self.votes.abstain;
                    if does_vote_count_pass(self.votes.yes, options, self.total_power, threshold) {
                        return PassedResponse {
                            is_passed: true,
                            description: "".to_string(),
                        };
                    }
                    return PassedResponse {
                        is_passed: false,
                        description: "".to_string(),
                    };
                } else {
                    let options = self.total_power - self.votes.abstain;
                    if does_vote_count_pass(self.votes.yes, options, self.total_power, threshold) {
                        return PassedResponse {
                            is_passed: true,
                            description: "".to_string(),
                        };
                    }
                    return PassedResponse {
                        is_passed: false,
                        description: "".to_string(),
                    };
                }
            }
            Threshold::AbsoluteCount { threshold } => {
                if self.votes.yes >= threshold {
                    return PassedResponse {
                        is_passed: true,
                        description: "".to_string(),
                    };
                }
                return PassedResponse {
                    is_passed: false,
                    description: "".to_string(),
                };
            }
        }
    }

    // As above for the passed check, used to check if a proposal is
    // already rejected.
    // pub fn is_rejected(&self, block: &BlockInfo) -> bool {
    //     // If re-voting is allowed and the proposal is not expired no
    //     // information is known.
    //     if self.allow_revoting && !self.expiration.is_expired(block) {
    //         return false;
    //     }

    //     match self.threshold {
    //         Threshold::AbsolutePercentage {
    //             percentage: percentage_needed,
    //         } => {
    //             println!(
    //                 "Status1 {:?} percentage_needed{:?}",
    //                 self.status, percentage_needed
    //             );
    //             let options = self.total_power - self.votes.abstain;

    //             // If there is a 100% passing threshold..
    //             if percentage_needed == PercentageThreshold::Percent(Decimal::percent(100)) {
    //                 if options == Uint128::zero() {
    //                     // and there are no possible votes (zero
    //                     // voting power or all abstain), then this
    //                     // proposal has been rejected.
    //                     return true;
    //                 } else {
    //                     // and there are possible votes, then this is
    //                     // rejected if there is a single no vote.
    //                     //
    //                     // We need this check becuase otherwise when
    //                     // we invert the threshold (`Decimal::one() -
    //                     // threshold`) we get a 0% requirement for no
    //                     // votes. Zero no votes do indeed meet a 0%
    //                     // threshold.
    //                     return self.votes.no >= Uint128::new(1);
    //                 }
    //             }

    //             does_vote_count_fail(self.votes.no, options, percentage_needed)
    //         }
    //         Threshold::ThresholdQuorum { threshold, quorum } => {
    //             match (
    //                 does_vote_count_pass(self.votes.yes, self.total_power, quorum),
    //                 self.expiration.is_expired(block),
    //             ) {
    //                 // Has met quorum and is expired.
    //                 (true, true) => {
    //                     // => consider only votes cast and see if no
    //                     //    votes meet threshold.
    //                     let options = self.votes.total() - self.votes.abstain;

    //                     // If there is a 100% passing threshold..
    //                     if threshold == PercentageThreshold::Percent(Decimal::percent(100)) {
    //                         if options == Uint128::zero() {
    //                             // and there are no possible votes (zero
    //                             // voting power or all abstain), then this
    //                             // proposal has been rejected.
    //                             return true;
    //                         } else {
    //                             // and there are possible votes, then this is
    //                             // rejected if there is a single no vote.
    //                             //
    //                             // We need this check becuase
    //                             // otherwise when we invert the
    //                             // threshold (`Decimal::one() -
    //                             // threshold`) we get a 0% requirement
    //                             // for no votes. Zero no votes do
    //                             // indeed meet a 0% threshold.
    //                             return self.votes.no >= Uint128::new(1);
    //                         }
    //                     }
    //                     does_vote_count_fail(self.votes.no, options, threshold)
    //                 }
    //                 // Has met quorum and is not expired.
    //                 // | Hasn't met quorum and is not expired.
    //                 (true, false) | (false, false) => {
    //                     // => consider all possible votes and see if
    //                     //    no votes meet threshold.
    //                     let options = self.total_power - self.votes.abstain;

    //                     // If there is a 100% passing threshold..
    //                     if threshold == PercentageThreshold::Percent(Decimal::percent(100)) {
    //                         if options == Uint128::zero() {
    //                             // and there are no possible votes (zero
    //                             // voting power or all abstain), then this
    //                             // proposal has been rejected.
    //                             return true;
    //                         } else {
    //                             // and there are possible votes, then this is
    //                             // rejected if there is a single no vote.
    //                             //
    //                             // We need this check because otherwise
    //                             // when we invert the threshold
    //                             // (`Decimal::one() - threshold`) we
    //                             // get a 0% requirement for no
    //                             // votes. Zero no votes do indeed meet
    //                             // a 0% threshold.
    //                             return self.votes.no >= Uint128::new(1);
    //                         }
    //                     }

    //                     does_vote_count_fail(self.votes.no, options, threshold)
    //                 }
    //                 // Hasn't met quorum requirement and voting has closed => rejected.
    //                 (false, true) => true,
    //             }
    //         }
    //         Threshold::AbsoluteCount { threshold } => {
    //             // If all the outstanding votes voting yes would not
    //             // cause this proposal to pass then it is rejected.
    //             let outstanding_votes = self.total_power - self.votes.total();
    //             self.votes.yes + outstanding_votes < threshold
    //         }
    //     }
    // }
}
