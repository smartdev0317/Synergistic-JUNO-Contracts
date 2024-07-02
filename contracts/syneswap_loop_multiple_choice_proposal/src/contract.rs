use std::cmp::{Ordering, min};

#[cfg(not(feature = "library"))]
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::proposal::{
    advance_proposal_id, advance_proposal_version, MultipleChoiceProposal,
    MultipleChoiceVote, MultipleChoiceProposalCustom, MultipleChoiceOptionCustom,
};
use crate::queriers::query_loop_gauge_proposal_by_id;
use crate::query::{
    VoteInfo, VoteListResponse, ProposalCustomResponse, ProposalListCustomResponse,
};
use crate::state::{
    Ballot, Config, BALLOTS, CONFIG, POOL_AMOUNTS, PROPOSALS, PROPOSAL_COUNT, PROPOSAL_VERSION, PROPOSERS_INFO, PROPOSAL_HISTORY, ProposalHistory, ProposalHistoryResponse, ProposalHistoryInfo, ProposalVersionResponse, PROPOSAL_EXECUTIONS, ProposalExecutionsInfo, ProposalExecusionsResponse, ProposalExecution,
};

use crate::status::Status;
use crate::voting::{validate_voting_period, Votes};
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Response, StdError, StdResult, Storage, Uint128,
    WasmQuery, ensure_eq, ensure_ne,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use cw_utils::Duration;
use syneswap::factory::MigrateMsg;
use syneswap::staking::QueryMsg as stakingMsg;
use syneswap_staking::{msg::Cw20QueryMsg as stakingMsg_, state::Config as StakingConfig};

pub const DEFAULT_LIMIT: u64 = 30;
pub const MAX_PROPOSAL_SIZE: u64 = 30_000;

pub(crate) const CONTRACT_NAME: &str = "crates.io:cwd-proposal-single";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let (min_voting_period, min_pending_period, max_voting_period) =
        validate_voting_period(msg.min_voting_period, msg.min_pending_period, msg.max_voting_period)?;

    let config = Config {
        // threshold: msg.threshold,
        max_voting_period,
        min_voting_period,
        min_pending_period,
        default_limit: 10,
        max_limit: 30,
        dao: msg.dao, // veSYNE
        admin: info.sender.to_string(),
        only_members_execute: true,
        proposal_creation_token_limit: msg.proposal_creation_token_limit,
        token_hold_duration: msg.token_hold_duration,
        loop_gauge: "juno18kvahfjnn2kmjvae3hmmgff8gn65swcf8tk83twlfu5hr2qrjwns7k4x4z".to_string(),
        loop_staker: msg.loop_staker, // bloop-converter-and-staker
    };

    // Initialize proposal count to zero so that queries return zero
    // instead of None.
    PROPOSAL_COUNT.save(deps.storage, &0)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "instantiate")
        .add_attribute("dao", config.dao))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Propose {
            title,
            description,
            options,
            pending_period,
            amount,
            loop_gauge_proposal_id,
        } => {
            let config = CONFIG.load(deps.storage)?;
            ensure_eq!(info.sender.to_string(), config.admin, StdError::generic_err("Unauthorized"));
            execute_propose(
                deps,
                env,
                info.sender,
                title,
                description,
                pending_period,
                options,
                amount,
                loop_gauge_proposal_id,
            )
        },
        ExecuteMsg::Vote { proposal_id, vote } => execute_vote(deps, env, info, proposal_id, vote),
        ExecuteMsg::Execute { proposal_id } => execute_execute(deps, env, info, proposal_id),
        ExecuteMsg::Reject { proposal_id } => execute_reject(deps, env, info, proposal_id),
        ExecuteMsg::Close { proposal_id } => execute_close(deps, env, info, proposal_id),
        ExecuteMsg::UpdateConfig {
            max_voting_period,
            min_voting_period,
            min_pending_period,
            dao,
            token_hold_duration,
            proposal_creation_token_limit,
            loop_gauge,
            loop_staker,
            only_members_execute,
            default_limit,
            max_limit
        } => execute_update_config(
            deps,
            info,
            max_voting_period,
            min_voting_period,
            min_pending_period,
            dao,
            token_hold_duration,
            proposal_creation_token_limit,
            loop_gauge,
            loop_staker,
            only_members_execute,
            default_limit,
            max_limit
        ),
        ExecuteMsg::AddMultipleChoiceOptions {
            proposal_id,
            options,
        } => execute_add_multiple_choice_options(deps, info, proposal_id, options),
        ExecuteMsg::RemoveMultipleChoiceOptions {
            proposal_id,
            options,
        } => execute_remove_multiple_choice_options(deps, info, proposal_id, options),
    }
}

pub fn execute_propose(
    deps: DepsMut,
    _env: Env,
    sender: Addr,
    title: String,
    description: String,
    pending_period: Duration,
    options: Vec<String>,
    amount: Uint128,
    loop_gauge_proposal_id: u64,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    // check voting power
    let vote_power = get_voting_power(
        &deps.querier,
        deps.storage,
        sender.clone(),
        config.dao.to_string(),
        0u64,
    )?;
    if vote_power.is_zero() {
        return Err(StdError::generic_err("Power is zero can't create proposal"));
    }

    // read loop_gauge proposal by id
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge.clone(),
        loop_gauge_proposal_id
    );

    ensure_ne!(loop_gauge_proposal.status, Status::Closed, StdError::generic_err("Closed proposal id"));

    // get loop_gauge_proposal expiration time
    let expiration = loop_gauge_proposal.expiration;

    // verify pending_period
    if !verifying_voting_period(
        &pending_period,
        &config.min_pending_period,
        &loop_gauge_proposal.voting_period,
    ) {
        return Err(StdError::generic_err("invalid active voting period"));
    }

    // check user has enough veSYNE for locking
    let mut locked_amount = PROPOSERS_INFO
        .load(deps.storage, sender.to_string())
        .unwrap_or(Uint128::zero());

    if vote_power - locked_amount < config.proposal_creation_token_limit {
        return Err(StdError::generic_err(
            "Power is not enough to create proposal",
        ));
    }
    // update user's locked token amount
    locked_amount += config.proposal_creation_token_limit;

    PROPOSERS_INFO.save(deps.storage, sender.to_string(), &locked_amount)?;

    // check voting options
    for option in &options {
        let mut is_contained = false;
        for choice in &loop_gauge_proposal.multiple_choice_options {
            if choice.pool.is_some() && choice.pool.clone().unwrap().eq(option) {
                is_contained = true;
                break;
            }
        }
        if !is_contained {
            return Err(StdError::generic_err("invalid option"));
        }
    }

    // get choice
    let mut choices: Vec<MultipleChoiceOptionCustom> = vec![];
    for option in &options {
        choices.push(MultipleChoiceOptionCustom { 
            pool: option.to_string(), 
            votes: Votes { 
                power: Uint128::zero()
            } 
        })
    }

    let proposal = {
        // Limit mutability to this block.
        let proposal = MultipleChoiceProposalCustom {
            title,
            description,
            proposer: sender.clone(),
            expiration,
            total_power: Uint128::zero(),
            status: Status::Open,
            allow_revoting: false,
            voting_start_time: loop_gauge_proposal.voting_start_time,
            choices,
            amount,
            voting_period: loop_gauge_proposal.voting_period,
            pending_period,
            loop_gauge: config.loop_gauge,
            loop_gauge_proposal_id,
        };
        // Update the proposal's status. Addresses case where proposal
        // expires on the same block as it is created.
        // proposal.update_status(&env.block)?; // status comes from loop gauge.
        proposal
    };
    let id = advance_proposal_id(deps.storage)?;
    advance_proposal_version(deps.storage, id.clone())?;
    // Limit the size of proposals.
    //
    // The Juno mainnet has a larger limit for data that can be
    // uploaded as part of an execute message than it does for data
    // that can be queried as part of a query. This means that without
    // this check it is possible to create a proposal that can not be
    // queried.
    //
    // The size selected was determined by uploading versions of this
    // contract to the Juno mainnet until queries worked within a
    // reasonable margin of error.
    //
    // `to_vec` is the method used by cosmwasm to convert a struct
    // into it's byte representation in storage.
    let proposal_size = cosmwasm_std::to_vec(&proposal)?.len() as u64;
    if proposal_size > MAX_PROPOSAL_SIZE {
        return Err(StdError::generic_err("Proposal size is too large"));
    }

    PROPOSALS.save(deps.storage, id, &proposal)?;

    Ok(Response::default()
        .add_attribute("action", "propose")
        .add_attribute("sender", sender)
        .add_attribute("proposal_id", id.to_string())
        .add_attribute("status", proposal.status.to_string()))
}
// pub fn get_multiple_choice_options(
//     choices: Vec<MultipleChoiceOptionMsg>,
// ) -> Vec<MultipleChoiceOption> {
//     let mut multiple_choice_options: Vec<MultipleChoiceOption> = vec![];
//     for choice in choices {
//         let multiple_choice_option = MultipleChoiceOption {
//             address: choice.address,
//             description: choice.description,
//             msgs: choice.msgs,
//             pool: choice.pool,
//             reward_token: choice.reward_token,
//             title: choice.title,
//             votes: Votes {
//                 power: Uint128::zero(),
//             },
//         };
//         multiple_choice_options.push(multiple_choice_option);
//     }
//     multiple_choice_options
// }

pub fn execute_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // get proposal by proposal_id
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No Such Proposal"))?;
    let vote_power = get_voting_power(
        &deps.querier,
        deps.storage,
        info.sender.clone(),
        config.dao.to_string(),
        proposal_id,
    )?;
    if vote_power.is_zero() {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // fetch loop_gauge_proposal by loop_gauge_proposal_id
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge,
        prop.loop_gauge_proposal_id
    );

    // check status
    let status = prop.current_status_custom(&env.block)?;
    if status != Status::VotingClosed {
        if status == Status::Open {
            return Err(StdError::generic_err("Voting not closed yet"));
        } else {
            return Err(StdError::generic_err(format!(
                "Proposal is in {} state",
                status
            )));
        }
    }

    // check expiration
    if let Some(order) = loop_gauge_proposal.expiration.partial_cmp(&prop.expiration) {
        // if proposal version is upgraded, close execution and start new proposal
        if order == Ordering::Greater {
            advance_proposal_version(deps.storage, proposal_id.clone())?;
            update_props(deps.storage, env, &loop_gauge_proposal, proposal_id.clone(), &mut prop)?;
            let response = Response::new()
                .add_attribute("action", "execute")
                .add_attribute("sender", info.sender)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("dao", config.dao);
            return Ok(response);
        }
    }
    // get execution_msg
    let msgs = prop.get_execution_message(&config.loop_staker)?;
    prop.status = Status::Executed;
    println!("msgs {:?}", msgs);
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    upgrade_proposal_history(deps.storage, env, proposal_id, prop)?;

    let response = Response::new()
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("dao", config.dao)
        .add_messages(msgs);

    Ok(response)
}

pub fn upgrade_proposal_history(
    storage: &mut dyn Storage,
    env: Env,
    proposal_id: u64,
    proposal: MultipleChoiceProposalCustom
) -> StdResult<bool> {
    let version = PROPOSAL_VERSION.load(storage, proposal_id)?;

    let proposal_history = ProposalHistory {
        time: env.block.time.seconds(),
        status: proposal.status.clone(),
        total_power: proposal.total_power,
        choices: proposal.choices,
        voting_start_time: proposal.voting_start_time,
        expiration: proposal.expiration,
    };

    PROPOSAL_HISTORY.save(storage, (proposal_id, version), &proposal_history)?;

    let proposal_execution = ProposalExecution {
        version: version,
        status: proposal.status.clone()
    };
    PROPOSAL_EXECUTIONS.save(storage, (proposal_id, env.block.time.seconds()), &proposal_execution)?;

    Ok(true)

}

pub fn execute_reject(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // get proposal by proposal_id
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No Such Proposal"))?;
    let vote_power = get_voting_power(
        &deps.querier,
        deps.storage,
        info.sender.clone(),
        config.dao.to_string(),
        proposal_id,
    )?;
    if vote_power.is_zero() {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // fetch loop_gauge_proposal by loop_gauge_proposal_id
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge,
        prop.loop_gauge_proposal_id
    );

    // check status
    let status = prop.current_status_custom(&env.block)?;
    if status != Status::VotingClosed {
        if status == Status::Open {
            return Err(StdError::generic_err("Voting not closed yet"));
        } else {
            return Err(StdError::generic_err(format!(
                "Proposal is in {} state",
                status
            )));
        }
    }

    // check expiration
    if let Some(order) = loop_gauge_proposal.expiration.partial_cmp(&prop.expiration) {
        // if proposal version is upgraded, close execution and start new proposal
        if order == Ordering::Greater {
            advance_proposal_version(deps.storage, proposal_id.clone())?;
            update_props(deps.storage, env, &loop_gauge_proposal, proposal_id.clone(), &mut prop)?;
            let response = Response::new()
                .add_attribute("action", "execute")
                .add_attribute("sender", info.sender)
                .add_attribute("proposal_id", proposal_id.to_string())
                .add_attribute("dao", config.dao);
            return Ok(response);
        }
    }
    prop.status = Status::Rejected;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;
    let response = Response::new()
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("dao", config.dao);

    upgrade_proposal_history(deps.storage, env, proposal_id, prop)?;

    Ok(response)
}

pub fn execute_add_multiple_choice_options(
    deps: DepsMut,
    _info: MessageInfo,
    proposal_id: u64,
    options: Vec<String>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No Such Proposal"))?;
    
    // read loop_gauge proposal by id
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge.clone(),
        prop.loop_gauge_proposal_id
    );

    // check voting options
    for option in &options {
        let mut is_contained = false;
        for choice in &loop_gauge_proposal.multiple_choice_options {
            if choice.pool.is_some() && choice.pool.clone().unwrap().eq(option) {
                is_contained = true;
                break;
            }
        }
        if !is_contained {
            return Err(StdError::generic_err("Invalid option"));
        }
        for choice in &prop.choices {
            if choice.pool.clone().eq(option) {
                return Err(StdError::generic_err("Duplicated lp address"));
            }
        }
    }
    
    for option in options {
        let choice = MultipleChoiceOptionCustom {
            pool: option,
            votes: Votes {
                power: Uint128::zero(),
            },
        };
        prop.choices.push(choice);
    }
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    Ok(Response::default())
}

pub fn execute_remove_multiple_choice_options(
    deps: DepsMut,
    _info: MessageInfo,
    proposal_id: u64,
    options: Vec<String>,
) -> StdResult<Response> {
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No Such Proposal"))?;
    let mut choices = vec![];
    for choice in prop.choices {
        let mut is_contained = false;
        for option in &options {
            if choice.pool.eq(option) {
                is_contained = true;
                break;
            }
        }
        if !is_contained {
            choices.push(choice);
        }
    }
    prop.choices = choices;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    Ok(Response::default())
}

pub fn update_props(
    storage: &mut dyn Storage,
    env: Env,
    loop_gauge_proposal: &MultipleChoiceProposal,
    proposal_id: u64,
    prop: &mut MultipleChoiceProposalCustom
) -> StdResult<()> {

    prop.status = Status::TimeOut;
    upgrade_proposal_history(storage, env, proposal_id, prop.clone())?;

    prop.status = Status::Open;
    prop.voting_start_time = loop_gauge_proposal.voting_start_time;
    prop.expiration = loop_gauge_proposal.expiration;
    prop.total_power = Uint128::zero();
    let mut choices: Vec<MultipleChoiceOptionCustom> = vec![];
    for mut choice in prop.choices.iter_mut() {
        choice.votes.power = Uint128::zero();
        choices.push(choice.clone());
    }
    prop.choices = choices;
    Ok(())
}

pub fn execute_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    votes: Vec<MultipleChoiceVote>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    // get proposal data
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No such proposal"))?;

    // get user's voting power and check voting power
    let vote_power = get_voting_power(
        &deps.querier,
        deps.storage,
        info.sender.clone(),
        config.dao.to_string(),
        proposal_id,
    )?;
    validate_options_custom(&votes, &prop)?;
    if vote_power.is_zero() {
        let mut message = "Not Registered".to_string();
        message.push_str(&format!(
            "sender {} dao {}",
            info.sender.clone(),
            config.dao.to_string(),
        ));
        return Err(StdError::generic_err(&message));
    }

    // get loop gauge proposal
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge,
        prop.loop_gauge_proposal_id
    );

    // check proposal status
    let status = prop.current_status_custom(&env.block)?;
    if status == Status::Closed {
        return Err(StdError::generic_err(format!(
            "Proposal is in {} state",
            status
        )));
    } else if status == Status::Executed || status == Status::ExecutionFailed || status == Status::Rejected || status == Status::VotingClosed {
        if let Some(order) = loop_gauge_proposal.expiration.partial_cmp(&prop.expiration) {
            // if proposal version is upgraded, close execution and start new proposal
            if order == Ordering::Greater {
                advance_proposal_version(deps.storage, proposal_id.clone())?;
                update_props(deps.storage, env.clone(), &loop_gauge_proposal, proposal_id.clone(), &mut prop)?;
            } else {
                return Err(StdError::generic_err(format!(
                    "Proposal is in {} state",
                    status
                )));
            }
        } else {
            return Err(StdError::generic_err(format!(
                "Proposal is in {} state",
                status
            )));
        }
    }
    let version = PROPOSAL_VERSION.load(deps.storage, proposal_id)?;
    let mut proposal_version = String::from(&proposal_id.to_string());
    proposal_version.push_str(".");
    proposal_version.push_str(&version.to_string());
    // sum of votes percentage should be less than 100%
    validate_votes_percentage(votes.clone())?;
    // update BALLOTS and prop's total power
    for options in votes {
        let index = prop.choices
            .iter()
            .position(
                |choice| 
                choice.pool == options.pool
            ).unwrap();
        BALLOTS.update(
            deps.storage,
            (
                proposal_version.clone(),
                info.sender.clone().to_string(),
                options.pool.clone(),
            ),
            |bal| match bal {
                Some(current_ballot) => {
                    if prop.allow_revoting {
                        if current_ballot.vote == options {
                            // Don't allow casting the same vote more than
                            // once. This seems liable to be confusing
                            // behavior.
                            Err(StdError::generic_err("Already Casted"))
                        } else {
                            // Remove the old vote if this is a re-vote.
                            prop.choices
                                .get_mut(index)
                                .unwrap()
                                .remove_vote(current_ballot.power);
                            prop.total_power = prop.total_power.checked_sub(current_ballot.power).unwrap();
                            Ok(Ballot {
                                power: vote_power.checked_multiply_ratio(
                                    options.percentage, 
                                    100u32
                                ).unwrap(),
                                vote: options.clone(),
                                time: env.block.time.seconds(),
                            })
                        }
                    } else {
                        prop.choices
                            .get_mut(index)
                            .unwrap()
                            .remove_vote(current_ballot.power);
                        prop.total_power = prop.total_power.checked_sub(current_ballot.power).unwrap();
                        Ok(Ballot {
                            power: vote_power.checked_multiply_ratio(
                                options.percentage, 
                                100u32
                            ).unwrap(),
                            vote: options.clone(),
                            time: env.block.time.seconds(),
                        })
                    }
                }
                None => Ok(Ballot {
                    power: vote_power.checked_multiply_ratio(
                        options.percentage, 
                        100u32
                    ).unwrap(),
                    vote: options.clone(),
                    time: env.block.time.seconds(),
                }),
            },
        )?;
        prop.choices
            .get_mut(index)
            .unwrap()
            .add_vote(vote_power.clone(), options.percentage.clone());
    }

    prop.total_power += vote_power;
    prop.update_status(&env.block)?;

    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // let new_status = prop.clone().status;

    Ok(Response::default()
        .add_attribute("action", "vote")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("status", prop.status.to_string())
        .add_attribute("power added", vote_power))
}

pub fn validate_votes_percentage(
    votes: Vec<MultipleChoiceVote>,
) -> StdResult<bool> {
    let mut vote_percentage = 0u32;
    for vote in votes {
        vote_percentage += vote.percentage;
    }
    if vote_percentage > 100u32 {
        return Err(StdError::generic_err("total percenatge voting is invalid"));
    }
    Ok(true)
}

pub fn validate_options(
    vote: &Vec<MultipleChoiceVote>,
    prop: &MultipleChoiceProposal,
) -> StdResult<()> {
    if vote.len() > prop.multiple_choice_options.len() {
        return Err(StdError::generic_err("not valid options provided"));
    }
    Ok(())
}
pub fn validate_options_custom(
    vote: &Vec<MultipleChoiceVote>,
    prop: &MultipleChoiceProposalCustom,
) -> StdResult<()> {
    if vote.len() > prop.choices.len() {
        return Err(StdError::generic_err("not valid options provided"));
    }
    Ok(())
}
pub fn execute_close(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> StdResult<Response> {
    let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
    ensure_eq!(info.sender, prop.proposer, StdError::generic_err("Unauthorization"));
    let config: Config = CONFIG.load(deps.storage)?;

    // Update status to ensure that proposals which were open and have
    // expired are moved to "rejected."

    let mut locked_amount = PROPOSERS_INFO
        .load(deps.storage, prop.proposer.to_string())
        .unwrap_or(Uint128::zero());
    if prop.voting_start_time + config.token_hold_duration < env.block.time.seconds() {
        return Err(StdError::generic_err(
            "tokens are hold in the contract kindly wait for the specified time",
        ));
    }

    prop.update_status(&env.block)?;
    locked_amount -= config.proposal_creation_token_limit;

    PROPOSERS_INFO.save(deps.storage, prop.proposer.to_string(), &locked_amount)?;

    prop.status = Status::Closed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // Add prepropose / deposit module hook which will handle deposit refunds.

    Ok(Response::default()
        .add_attribute("action", "close")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string()))
}

pub fn get_voting_power(
    querier: &QuerierWrapper,
    store: &mut dyn Storage,
    sender: Addr,
    dao: String,
    proposal_id: u64,
) -> StdResult<Uint128> {
    let mut total_power = Uint128::zero();

    let config: StakingConfig = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: dao.to_string(),
        msg: to_binary(&stakingMsg::QueryConfig {})?,
    }))?;

    let durations = config.duration_values_vector;
    for duration in durations {
        let power: Uint128 = querier
            .query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: dao.to_string(),
                msg: to_binary(&stakingMsg_::BalanceByDuration {
                    address: sender.to_string(),
                    duration,
                })?,
            }))
            .unwrap_or(Uint128::from(0u128));
        total_power += power;
        POOL_AMOUNTS.save(store, (proposal_id, duration), &power)?;
    }
    Ok(total_power)
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    // threshold_: Option<Threshold>,
    max_voting_period_: Option<Duration>,
    min_voting_period_: Option<Duration>,
    min_pending_period_: Option<Duration>,
    dao_: Option<String>,
    token_hold_duration_: Option<u64>,
    proposal_creation_token_limit_: Option<Uint128>,
    loop_gauge_: Option<String>,
    loop_staker_: Option<String>,
    only_members_execute_: Option<bool>,
    default_limit_: Option<u64>,
    max_limit_: Option<u64>,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Only the DAO may call this method.
    if info.sender != config.admin {
        return Err(StdError::generic_err("Unauthorized"));
    }
    let mut min_voting_period = config.min_voting_period;
    let mut max_voting_period = config.max_voting_period;
    let mut min_pending_period = config.min_pending_period;
    if min_voting_period_.is_some() {
        min_voting_period = min_voting_period_.unwrap();
    }
    if max_voting_period_.is_some() {
        max_voting_period = max_voting_period_.unwrap();
    }
    if min_pending_period_.is_some() {
        min_pending_period = min_pending_period_.unwrap();
    }
    let (min_voting_period, min_pending_period, max_voting_period) =
        validate_voting_period(min_voting_period, min_pending_period, max_voting_period)?;
    let mut dao = config.dao;
    if dao_.is_some() {
        dao = dao_.unwrap();
    }
    let mut token_hold_duration = config.token_hold_duration;
    if token_hold_duration_.is_some() {
        token_hold_duration = token_hold_duration_.unwrap();
    }

    let mut proposal_creation_token_limit = config.proposal_creation_token_limit;
    if proposal_creation_token_limit_.is_some() {
        proposal_creation_token_limit = proposal_creation_token_limit_.unwrap();
    }

    let mut loop_gauge = config.loop_gauge;
    if loop_gauge_.is_some() {
        loop_gauge = loop_gauge_.unwrap();
    }

    let mut loop_staker = config.loop_staker;
    if loop_staker_.is_some() {
        loop_staker = loop_staker_.unwrap();
    }

    let mut only_members_execute = config.only_members_execute;
    if only_members_execute_.is_some() {
        only_members_execute = only_members_execute_.unwrap();
    }

    let mut default_limit = config.default_limit;
    if default_limit_.is_some() {
        default_limit = default_limit_.unwrap();
    }

    let mut max_limit = config.max_limit;
    if max_limit_.is_some() {
        max_limit = max_limit_.unwrap();
    }

    CONFIG.save(
        deps.storage,
        &Config {
            // threshold,
            max_voting_period,
            min_voting_period,
            min_pending_period,
            dao,
            admin: config.admin,
            token_hold_duration,
            proposal_creation_token_limit,
            loop_gauge,
            loop_staker,
            only_members_execute,
            default_limit,
            max_limit,
        },
    )?;

    Ok(Response::default()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn verifying_voting_period(
    voting_period: &Duration,
    min_vp: &Duration,
    max_vp: &Duration,
) -> bool {
    match (voting_period, min_vp, max_vp) {
        // compare if both height or both time
        (Duration::Time(vp), Duration::Time(mvp1), Duration::Time(mvp2)) => {
            vp >= mvp1 && vp <= mvp2
        }

        // if they are mis-matched finite ends, no compare possible
        _ => false,
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => query_config(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::Proposal {
            proposal_id
        } => query_proposal(deps, env, proposal_id),
        QueryMsg::ListProposals {
            start_after,
            limit
        } => {
            query_list_proposals(deps, env, start_after, limit)
        }
        QueryMsg::ProposalCount {} => query_proposal_count(deps),
        QueryMsg::GetVote {
            proposal_id,
            voter,
            version,
        } => query_user_list_vote(deps, proposal_id, voter, version),
        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => query_list_votes(deps, proposal_id, start_after, limit),
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::HoldAmount {
            address
        } => query_hold_amount(deps, address),
        QueryMsg::ProposalHistory {
            proposal_id,
            start_after,
            limit,
            reverse,
        } => query_proposal_history(deps, proposal_id, start_after, limit, reverse),
        QueryMsg::ProposalVersion {
            proposal_id
        } => query_proposal_version(deps, proposal_id),
        QueryMsg::ProposalExecutions {
            proposal_id,
            start_after,
            limit
        } => query_proposal_executions(deps, proposal_id, start_after, limit),
        // QueryMsg::ReverseProposals {
        //     start_before,
        //     limit,
        // } => query_reverse_proposals(deps, env, start_before, limit),
        // QueryMsg::ProposalCreationPolicy {} => query_creation_policy(deps),
        // QueryMsg::ProposalHooks {} => to_binary(&PROPOSAL_HOOKS.query_hooks(deps)?),
        // QueryMsg::VoteHooks {} => to_binary(&VOTE_HOOKS.query_hooks(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_binary(&config)
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_binary(&config.dao)
}

pub fn query_proposal(deps: Deps, env: Env, id: u64) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    let mut proposal = PROPOSALS.load(deps.storage, id)?;
    let loop_gauge_proposal = query_loop_gauge_proposal_by_id(
        &deps.querier,
        config.loop_gauge,
        proposal.loop_gauge_proposal_id
    );
    let loop_gauge_status = loop_gauge_proposal.current_status(&env.block)?;
    if let Some(order) = loop_gauge_proposal.expiration.partial_cmp(&proposal.expiration) {
        // if proposal version is upgraded, close execution and start new proposal
        if order == Ordering::Greater {
            proposal.status = loop_gauge_status;
            proposal.voting_start_time = loop_gauge_proposal.voting_start_time;
            proposal.expiration = loop_gauge_proposal.expiration;
            proposal.total_power = Uint128::zero();
            let mut choices: Vec<MultipleChoiceOptionCustom> = vec![];
            for mut choice in proposal.choices.iter_mut() {
                choice.votes.power = Uint128::zero();
                choices.push(choice.clone());
            }
            proposal.choices = choices;
        } else {
            proposal.status = proposal.current_status_custom(&env.block)?;
        }
    } else {
        proposal.status = proposal.current_status_custom(&env.block)?;
    }
    to_binary(&proposal.into_response(&env.block, id)?)
}

pub fn query_hold_amount(deps: Deps, address: String) -> StdResult<Binary> {
    to_binary(
        &PROPOSERS_INFO
            .load(deps.storage, address)
            .unwrap_or(Uint128::zero()),
    )
}

// pub fn query_creation_policy(deps: Deps) -> StdResult<Binary> {
//     let policy = CREATION_POLICY.load(deps.storage)?;
//     to_binary(&policy)
// }

pub fn query_list_proposals(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let min = start_after.map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let props= PROPOSALS
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, MultipleChoiceProposalCustom)>, _>>()?
        .into_iter()
        .map(|(id, _)| {
            deps.querier.query_wasm_smart(
                env.contract.address.to_string(),
                &QueryMsg::Proposal { proposal_id: id },
            )
        })
        .collect::<StdResult<Vec<ProposalCustomResponse>>>()?;

    to_binary(&ProposalListCustomResponse { proposals: props })
}

pub fn query_reverse_proposals(
    deps: Deps,
    env: Env,
    start_before: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let max = start_before.map(Bound::exclusive);
    let props: Vec<ProposalCustomResponse> = PROPOSALS
        .range(deps.storage, None, max, cosmwasm_std::Order::Descending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, MultipleChoiceProposalCustom)>, _>>()?
        .into_iter()
        .map(|(id, _)| {
            deps.querier.query_wasm_smart(
                env.contract.address.to_string(),
                &QueryMsg::Proposal { proposal_id: id },
            )
        })
        .collect::<StdResult<Vec<ProposalCustomResponse>>>()?;

    to_binary(&ProposalListCustomResponse { proposals: props })
}

pub fn query_proposal_count(deps: Deps) -> StdResult<Binary> {
    let proposal_count = PROPOSAL_COUNT.load(deps.storage)?;
    to_binary(&proposal_count)
}

pub fn query_user_list_vote(deps: Deps, proposal_id: u64, voter: String, version_: Option<u128>) -> StdResult<Binary> {
    let mut version = PROPOSAL_VERSION.load(deps.storage, proposal_id)?;
    if version_.is_some() {
        version = version_.unwrap();
    }
    let mut proposal_version = String::from(&proposal_id.to_string());
    proposal_version.push_str(".");
    proposal_version.push_str(&version.to_string());
    let votes = BALLOTS
        .prefix((proposal_version, voter.clone()))
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .take(30 as usize)
        .map(|item| {
            let (pool, ballot) = item?;
            Ok(VoteInfo {
                voter: voter.clone(),
                power: ballot.power,
                pool,
                percentage: ballot.vote.percentage,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&VoteListResponse { votes })
}

pub fn query_list_votes(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<(String, String)>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);

    let min = start_after.map(Bound::<(String, String)>::exclusive);
    let version = PROPOSAL_VERSION.load(deps.storage, proposal_id)?;
    let mut proposal_version = String::from(&proposal_id.to_string());
    proposal_version.push_str(".");
    proposal_version.push_str(&version.to_string());
    let votes = BALLOTS
        .sub_prefix(proposal_version)
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .map(|item| {
            let ((voter, pool), ballot) = item?;
            Ok(VoteInfo {
                voter,
                power: ballot.power,
                pool,
                percentage: ballot.vote.percentage,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&VoteListResponse { votes })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&info)
}

pub fn query_proposal_history(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<u128>,
    limit: Option<u64>,
    reverse: Option<bool>,
) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    let limit = min(limit.unwrap_or(DEFAULT_LIMIT), config.max_limit);
    let min = start_after.map(Bound::<u128>::exclusive);
    let sorting = if reverse.is_some() && reverse.unwrap() == true {
        cosmwasm_std::Order::Descending
    } else {
        cosmwasm_std::Order::Ascending
    };
    let proposal_histories = PROPOSAL_HISTORY
        .prefix(proposal_id)
        .range(deps.storage, min, None, sorting)
        .take(limit as usize)
        .map(|item| {
            let (version, proposal_history) = item?;
            Ok(ProposalHistoryInfo {
                version,
                info: proposal_history,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&ProposalHistoryResponse { history: proposal_histories })
}

pub fn query_proposal_version(
    deps: Deps,
    proposal_id: u64,
) -> StdResult<Binary> {
    let proposal_version = PROPOSAL_VERSION.load(deps.storage, proposal_id)?;
    to_binary(&ProposalVersionResponse { version: proposal_version })
}

pub fn query_proposal_executions(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<u64>,
    limit: Option<u64>
) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    let limit = min(limit.unwrap_or(DEFAULT_LIMIT), config.max_limit);
    let min = start_after.map(Bound::<u64>::exclusive);
    let proposal_executioins = PROPOSAL_EXECUTIONS
        .prefix(proposal_id)
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .map(|item| {
            let (time, execution) = item?;
            Ok(ProposalExecutionsInfo {
                time,
                version: execution.version,
                status: execution.status
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&ProposalExecusionsResponse { executions: proposal_executioins })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
