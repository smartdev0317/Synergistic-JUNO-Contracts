use cosmwasm_std::{
    QuerierWrapper, Addr
};

use crate::msg::{QueryMsg, GaugeResponse};

pub fn query_wynd_gauge_by_id (
    querier: &QuerierWrapper,
    contract: Addr,
    wynd_gauge_id: u64,
) -> GaugeResponse {
    querier.query_wasm_smart(contract, &QueryMsg::Gauge { id: wynd_gauge_id }).unwrap()
}
