use cosmwasm_std::{Decimal, StdError, StdResult, Uint128, Uint256};
use cw_utils::Duration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::threshold::PercentageThreshold;

// We multiply by this when calculating needed_votes in order to round
// up properly.
const PRECISION_FACTOR: u128 = 10u128.pow(9);

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]

pub struct Votes {
    pub power: Uint128,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Vote {
    /// Marks support for the proposal.
    Yes,
    /// Marks opposition to the proposal.
    No,
    /// Marks participation but does not count towards the ratio of
    /// support / opposed.
    Abstain,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MultipleChoiceVote {
    // A vote indicates which option the user has selected.
    pub option_id: u32,
}

impl std::fmt::Display for MultipleChoiceVote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.option_id)
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct MultipleChoiceVotes {
    // Vote counts is a vector of integers indicating the vote weight for each option
    // (the index corresponds to the option).
    pub vote_weights: Vec<Uint128>,
}

impl MultipleChoiceVotes {
    /// Sum of all vote weights
    pub fn total(&self) -> Uint128 {
        self.vote_weights.iter().sum()
    }

    pub fn add_vote(&mut self, vote: MultipleChoiceVote, weight: Uint128) -> StdResult<()> {
        self.vote_weights[vote.option_id as usize] = self.vote_weights[vote.option_id as usize]
            .checked_add(weight)
            .map_err(StdError::overflow)?;
        Ok(())
    }

    pub fn remove_vote(&mut self, vote: MultipleChoiceVote, weight: Uint128) -> StdResult<()> {
        self.vote_weights[vote.option_id as usize] = self.vote_weights[vote.option_id as usize]
            .checked_sub(weight)
            .map_err(StdError::overflow)?;
        Ok(())
    }

    pub fn zero(num_choices: usize) -> Self {
        Self {
            vote_weights: vec![Uint128::zero(); num_choices],
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub enum VoteCmp {
    Greater,
    Geq,
}

pub fn compare_vote_count(
    votes: Uint128,
    cmp: VoteCmp,
    total_power: Uint128,
    passing_percentage: Decimal,
) -> bool {
    let votes = votes.full_mul(PRECISION_FACTOR);
    let total_power = total_power.full_mul(PRECISION_FACTOR);
    let threshold = total_power.multiply_ratio(
        passing_percentage.atomics(),
        Uint256::from(10u64).pow(passing_percentage.decimal_places()),
    );

    println!(
        "{} {} {} {} {}",
        votes,
        total_power,
        threshold,
        Uint256::from(10u64).pow(passing_percentage.decimal_places()),
        passing_percentage.decimal_places()
    );
    match cmp {
        VoteCmp::Greater => votes > threshold,
        VoteCmp::Geq => votes >= threshold,
    }
}

pub fn does_vote_count_pass(
    yes_votes: Uint128,
    options: Uint128,
    percent: PercentageThreshold,
) -> bool {
    // Don't pass proposals if all the votes are abstain.
    if options.is_zero() {
        return false;
    }
    match percent {
        PercentageThreshold::Majority {} => yes_votes.full_mul(2u64) > options.into(),
        PercentageThreshold::Percent(percent) => {
            compare_vote_count(yes_votes, VoteCmp::Geq, options, percent)
        }
    }
}

pub fn does_vote_count_fail(
    no_votes: Uint128,
    options: Uint128,
    percent: PercentageThreshold,
) -> bool {
    // All abstain votes should result in a rejected proposal.
    if options.is_zero() {
        return true;
    }

    match percent {
        PercentageThreshold::Majority {} => {
            // Fails if no votes have >= half of all votes.
            no_votes.full_mul(2u64) >= options.into()
        }
        PercentageThreshold::Percent(percent) => compare_vote_count(
            no_votes,
            VoteCmp::Greater,
            options,
            Decimal::one() - percent,
        ),
    }
}

impl std::fmt::Display for Vote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vote::Yes => write!(f, "yes"),
            Vote::No => write!(f, "no"),
            Vote::Abstain => write!(f, "abstain"),
        }
    }
}

/// A height of None will query for the current block height.
// pub fn get_voting_power(
//     deps: Deps,
//     address: Addr,
//     dao: Addr,
//     height: Option<u64>,
// ) -> StdResult<Uint128> {
//     let response: voting::VotingPowerAtHeightResponse = deps.querier.query_wasm_smart(
//         dao,
//         &voting::Query::VotingPowerAtHeight {
//             address: address.into_string(),
//             height,
//         },
//     )?;
//     Ok(response.power)
// }

/// A height of None will query for the current block height.
// pub fn get_total_power(deps: Deps, dao: Addr, height: Option<u64>) -> StdResult<Uint128> {
//     let response: voting::TotalPowerAtHeightResponse = deps
//         .querier
//         .query_wasm_smart(dao, &voting::Query::TotalPowerAtHeight { height })?;
//     Ok(response.power)
// }

/// Validates that the min voting period is less than the max voting
/// period. Passes arguments through the function.
pub fn validate_voting_period(min: Duration, pending: Duration, max: Duration) -> StdResult<(Duration, Duration, Duration)> {
    let min = {
        let valid = match (min, pending, max) {
            (Duration::Time(min), Duration::Time(pending), Duration::Time(max)) => min <= max && pending <= max,
            (Duration::Height(min), Duration::Height(pending), Duration::Height(max)) => min <= max && pending <= max,
            _ => return Err(StdError::generic_err("Duration Minutes conflict")),
        };
        if valid {
            min
        } else {
            return Err(StdError::generic_err("Invalid voting period"));
        }
    };

    Ok((min, pending, max))
}
