use cosmwasm_schema::{cw_serde};
use cosmwasm_std::{CosmosMsg, Empty};
use cw_utils::Expiration;
use cw20::Cw20ReceiveMsg;
use cw3::{Vote};
use cw4::MemberChangedHookMsg;



// TODO: add some T variants? Maybe good enough as fixed Empty for now
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConverter {
        address: String,
    },
    UpdateDuration {
        duration: u64,
    },
    Propose {
        title: String,
        description: String,
        msgs: Vec<CosmosMsg<Empty>>,
        // note: we ignore API-spec'd earliest if passed, always opens immediately
        latest: Option<Expiration>,
    },
    Vote {
        proposal_id: u64,
        vote: Vote,
    },
    Execute {
        proposal_id: u64,
    },
    Close {
        proposal_id: u64,
    },
    /// Handles update hook messages from the group contract
    MemberChangedHook(MemberChangedHookMsg),
    // Receive cw20 tokens
    Receive(Cw20ReceiveMsg),
    WithdrawRewards {},
}

#[cw_serde]
pub enum StakeMsg {
    Stake {},
}
