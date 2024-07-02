use cosmwasm_std::{
    QuerierWrapper
};

use crate::{msg::QueryMsg, query::ProposalResponse, proposal::MultipleChoiceProposal};

pub fn query_loop_gauge_proposal_by_id (
    querier: &QuerierWrapper,
    contract_addr: String,
    proposal_id: u64,
) -> MultipleChoiceProposal {
    let res: ProposalResponse = querier.query_wasm_smart(contract_addr, &QueryMsg::Proposal { proposal_id }).unwrap();
    res.proposal
}
