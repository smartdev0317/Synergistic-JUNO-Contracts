use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Deps, Env, Order, StdResult, Storage, Uint128};
use cw_storage_plus::{Bound, Index, IndexList, IndexedMap, Item, Map, MultiIndex};
use cw_utils::maybe_addr;

use crate::msg::{VoteInfo, GaugeResponse};

/// Type alias for u64 to make the map types a bit more self-explanatory
pub type GaugeId = u64;
pub type Epoch = u128;

pub const CONFIG: Item<Config> = Item::new("config");
pub const GAUGES: Map<GaugeId, Gauge> = Map::new("gauges");
const LAST_ID: Item<GaugeId> = Item::new("last_id");
pub const LAST_EPOCH: Map<GaugeId, Epoch> = Map::new("last_epoch");
pub const VOTE_HISTORY: Map<(GaugeId, Epoch), VoteHistory> = Map::new("vote_history");

/// Get ID for gauge registration and increment value in storage
pub fn fetch_last_id(storage: &mut dyn Storage) -> StdResult<u64> {
    let last_id = LAST_ID.load(storage).unwrap_or_default();
    LAST_ID.save(storage, &(last_id + 1u64))?;
    Ok(last_id)
}

/// This lets us find and update any vote given both voter and gauge.
/// It also lets us iterate over all votes by a given voter on all gauges
/// or by a given gauge id. This is needed when a voter weight changes
/// in order to update the guage.
pub fn votes() -> Votes<'static> {
    Votes::new("votes", "votes__gaugeid")
}

// settings for pagination
const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 30;

#[cw_serde]
pub struct Config {
    /// Address of contract to that contains all voting powers (where we query and listen to hooks)
    pub voting_powers: Addr,
    /// Address that can add new gauges or stop them
    pub owner: Addr,
    /// Address of DAO core module resposible for instantiation and execution of messages
    pub dao_core: Addr,
    /// WYND DAO Gauge contract
    pub wynd_gauge: Addr,

    pub wynd_staker: Addr,
}

#[cw_serde]
pub struct VoteHistory {
    pub epoch: u64,
    pub next_epoch: u64,
    pub votes: Vec<Vote>,
    pub executed: bool,
}


#[cw_serde]
pub struct Gauge {
    /// Descriptory label of gauge
    pub title: String,
    /// Address of contract to serve gauge-specific info (AdapterQueryMsg)
    pub adapter: Addr,
    /// Frequency (in seconds) the gauge executes messages, typically something like 7*86400
    pub epoch: u64,
    /// Minimum percentage of votes needed by a given option to be in the selected set
    pub min_percent_selected: Option<Decimal>,
    /// Maximum number of Options to make the selected set. Needed even with
    /// `min_percent_selected` to provide some guarantees on gas usage of this query.
    pub max_options_selected: u32,
    // Any votes above that percentage will be discarded
    pub max_available_percentage: Option<Decimal>,
    /// True if the gauge is stopped
    pub is_stopped: bool,
    /// UNIX time (seconds) when next epoch can be executed. If < env.block.time then Execute can be called
    pub next_epoch: u64,
    /// The last set of options selected by the gauge, `None` before the first execution
    pub last_executed_set: Option<Vec<(String, Uint128)>>,
    /// Set this in migration if the gauge should be periodically reset
    pub reset: Option<Reset>,
    /// pending time to execute gauge
    pub epoch_pending_size: u64,
    /// wynd gauge id
    pub wynd_gauge_id: u64,
}

#[cw_serde]
pub struct Reset {
    /// until the first reset, this is None - needed for 0-cost migration from current state
    pub last: Option<u64>,
    /// seconds between reset
    pub reset_each: u64,
    /// next time we can reset
    pub next: u64,
}

impl Gauge {
    /// Returns `true` if the gauge is currently being reset
    pub fn is_resetting(&self) -> bool {
        self.reset
            .as_ref()
            .map(|r| r.last == Some(r.next))
            .unwrap_or_default()
    }

    /// Returns `true` if the gauge is currently pending
    pub fn is_pending(&self, timestamp: u64, wynd_gauge: &GaugeResponse) -> bool {
        // currently, wynd_gauge is not ended but syne gauge is ended
        if timestamp > self.next_epoch - self.epoch_pending_size && timestamp <= self.next_epoch {
            return true;
        }
        // currently, wynd gauge is ended but still not executed
        else if timestamp > self.next_epoch && timestamp > wynd_gauge.next_epoch {
            return true;
        }
        // currently, syne_gauge is not ended or in the next epoch
        else { return false; }
    }
}

#[cw_serde]
pub struct WeightedVotes {
    /// The gauge these votes are for
    pub gauge_id: GaugeId,
    /// The voting power behind the vote.
    pub power: Uint128,
    /// the user's votes for this gauge
    pub votes: Vec<Vote>,
    /// Timestamp when vote was cast.
    /// Allow `None` for 0-cost migration from current data
    pub cast: Option<u64>,
}

impl WeightedVotes {
    /// Returns `true` if the vote is
    pub fn is_expired(&self, gauge: &Gauge) -> bool {
        // check if the vote is older than the last reset
        match &gauge.reset {
            Some(Reset {
                last: Some(expired),
                ..
            }) => {
                // votes with no timestamp are always considered too old once a reset happened
                // (they are legacy votes pre-first reset)
                self.cast.unwrap_or_default() < *expired
            }
            // everything is valid before the first reset (last = `None`) or if the gauge is not resettable
            _ => false,
        }
    }
}

impl Default for WeightedVotes {
    fn default() -> Self {
        WeightedVotes {
            gauge_id: 0,
            power: Uint128::zero(),
            votes: vec![],
            cast: None,
        }
    }
}

#[cw_serde]
pub struct Vote {
    /// Option voted for.
    pub option: String,
    /// The weight of the power given to this vote
    pub weight: Decimal,
}

struct VoteIndexes<'a> {
    // Last type param defines the pk deserialization type
    pub vote: MultiIndex<'a, GaugeId, WeightedVotes, (&'a Addr, GaugeId)>,
}

impl<'a> IndexList<WeightedVotes> for VoteIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<WeightedVotes>> + '_> {
        Box::new(std::iter::once(&self.vote as &dyn Index<WeightedVotes>))
    }
}

pub struct Votes<'a> {
    // Votes are indexed by `(addr, gauge_id, weight)` triplet
    votes: IndexedMap<'a, (&'a Addr, GaugeId), WeightedVotes, VoteIndexes<'a>>,
}

impl<'a> Votes<'a> {
    pub fn new(storage_key: &'a str, vote_subkey: &'a str) -> Self {
        let indexes = VoteIndexes {
            vote: MultiIndex::new(|_, vote| vote.gauge_id, storage_key, vote_subkey),
        };
        let votes = IndexedMap::new(storage_key, indexes);
        Self { votes }
    }

    pub fn save(
        &self,
        storage: &mut dyn Storage,
        voter: &'a Addr,
        gauge_id: GaugeId,
        vote: &WeightedVotes,
    ) -> StdResult<()> {
        self.votes.save(storage, (voter, gauge_id), vote)
    }

    pub fn set_votes(
        &self,
        storage: &mut dyn Storage,
        env: &Env,
        voter: &'a Addr,
        gauge_id: GaugeId,
        votes: Vec<Vote>,
        power: impl Into<Uint128>,
    ) -> StdResult<()> {
        let power = power.into();
        self.votes.save(
            storage,
            (voter, gauge_id),
            &WeightedVotes {
                gauge_id,
                power,
                votes,
                cast: Some(env.block.time.seconds()),
            },
        )
    }

    pub fn remove_votes(
        &self,
        storage: &mut dyn Storage,
        voter: &'a Addr,
        gauge_id: GaugeId,
    ) -> StdResult<()> {
        self.votes.remove(storage, (voter, gauge_id))
    }

    pub fn load(
        &self,
        storage: &dyn Storage,
        voter: &'a Addr,
        gauge_id: GaugeId,
    ) -> StdResult<WeightedVotes> {
        self.votes.load(storage, (voter, gauge_id))
    }

    pub fn may_load(
        &self,
        storage: &dyn Storage,
        voter: &'a Addr,
        gauge_id: GaugeId,
    ) -> StdResult<Option<WeightedVotes>> {
        self.votes.may_load(storage, (voter, gauge_id))
    }

    pub fn query_votes_by_voter(
        &self,
        deps: Deps,
        voter_addr: &'a Addr,
        start_after: Option<GaugeId>,
        limit: Option<u32>,
    ) -> StdResult<Vec<WeightedVotes>> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start = start_after.map(Bound::exclusive);

        self.votes
            .prefix(voter_addr)
            .range(deps.storage, start, None, Order::Ascending)
            .map(|index| {
                let (_, vote) = index?;
                Ok(vote)
            })
            .take(limit)
            .collect()
    }

    pub fn query_votes_by_gauge(
        &self,
        deps: Deps,
        gauge_id: GaugeId,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<Vec<VoteInfo>> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let addr = maybe_addr(deps.api, start_after)?;
        let start = addr.as_ref().map(|a| Bound::exclusive((a, gauge_id)));

        let gauge = GAUGES.load(deps.storage, gauge_id)?;

        self.votes
            .idx
            .vote
            .prefix(gauge_id)
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .filter(|r| match r {
                Ok((_, v)) => !v.is_expired(&gauge), // filter out expired votes
                Err(_) => true,                      // keep the error
            })
            .map(|r| {
                let ((voter, _gauge), votes) = r?;
                Ok(VoteInfo {
                    voter: voter.into_string(),
                    votes: votes.votes,
                    cast: votes.cast,
                })
            })
            // NIT: collect and into_iter is a bit inefficient... guess it was too complex/confusing otherwise, so fine
            .collect()
    }
}

/// Total amount of votes in all options, used to calculate min percentage.
pub const TOTAL_CAST: Map<GaugeId, u128> = Map::new("total_power");

/// Count how many points each option has per gauge
pub const TALLY: Map<(GaugeId, &str), u128> = Map::new("tally");
/// Sorted index of options by points, separated by gauge - data field is a placeholder
pub const OPTION_BY_POINTS: Map<(GaugeId, u128, &str), u8> = Map::new("tally_points");

/// Updates the tally for one option.
/// The first time a user votes, they get `{old_vote: 0, new_vote: power}`
/// If they change options, call old option with `{old_vote: power, new_vote: 0}` and new option with `{old_vote: 0, new_vote: power}`
/// If a user changes power (member update hook), call existing option with `{old_vote: old_power, new_vote: new_power}`
pub fn update_tally(
    storage: &mut dyn Storage,
    gauge: GaugeId,
    option: &str,
    old_vote: u128,
    new_vote: u128,
) -> StdResult<()> {
    update_tallies(storage, gauge, vec![(option, old_vote, new_vote)])
}

/// Completely removes the given option from the tally.
pub fn remove_tally(storage: &mut dyn Storage, gauge: GaugeId, option: &str) -> StdResult<()> {
    let old_vote = TALLY.may_load(storage, (gauge, option))?;

    // update main index
    TALLY.remove(storage, (gauge, option));

    if let Some(old_vote) = old_vote {
        let total_cast = TOTAL_CAST.may_load(storage, gauge)?.unwrap_or_default();
        // update total cast
        TOTAL_CAST.save(storage, gauge, &(total_cast - old_vote))?;

        // update sorted index
        OPTION_BY_POINTS.remove(storage, (gauge, old_vote, option));
    }

    Ok(())
}

/// Updates the tally for one option.
/// The first time a user votes, they get `{old_vote: 0, new_vote: power}`
/// If they change options, call old option with `{old_vote: power, new_vote: 0}` and new option with `{old_vote: 0, new_vote: power}`
/// If a user changes power (member update hook), call existing option with `{old_vote: old_power, new_vote: new_power}`
pub fn update_tallies(
    storage: &mut dyn Storage,
    gauge: GaugeId,
    // (option, old, new)
    updates: Vec<(&str, u128, u128)>,
) -> StdResult<()> {
    let mut old_votes = 0u128;
    let mut new_votes = 0u128;

    for (option, old_vote, new_vote) in updates {
        old_votes += old_vote;
        new_votes += new_vote;

        // get old and new values
        let old_count = TALLY.may_load(storage, (gauge, option))?;
        let count = old_count.unwrap_or_default() + new_vote - old_vote;

        // update main index
        TALLY.save(storage, (gauge, option), &count)?;

        // delete old secondary index (if any)
        if let Some(old) = old_count {
            OPTION_BY_POINTS.remove(storage, (gauge, old, option));
        }
        // add new secondary index
        OPTION_BY_POINTS.save(storage, (gauge, count, option), &1u8)?;
    }

    // update total count
    let total = TOTAL_CAST.may_load(storage, gauge)?.unwrap_or_default();
    let total = total + new_votes - old_votes;
    TOTAL_CAST.save(storage, gauge, &total)
}
