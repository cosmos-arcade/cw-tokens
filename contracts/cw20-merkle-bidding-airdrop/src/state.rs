use std::ops::Add;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::{Expiration, Scheduled};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Owner If None set, contract is frozen.
    pub owner: Option<Addr>,
    pub cw20_token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
/// struct to manage start and end of static stages
pub struct Stage {
    /// start variable will be scheduled by time or block
    pub start: Scheduled,
    /// end variable will be scheduled by time or block
    /// or, if needed, never.
    pub end: Expiration,
}

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const STAGE_BID_KEY: &str = "stage_bid";
pub const STAGE_BID: Item<Stage> = Item::new(STAGE_BID_KEY);

pub const STAGE_CLAIM_AIRDROP_KEY: &str = "stage_claim_airdrop";
pub const STAGE_CLAIM_AIRDROP: Item<Stage> = Item::new(STAGE_CLAIM_AIRDROP_KEY);

pub const STAGE_CLAIM_PRIZE_KEY: &str = "stage_claim_prize";
pub const STAGE_CLAIM_PRIZE: Item<Stage> = Item::new(STAGE_CLAIM_PRIZE_KEY);

pub const WINNING_ADDRESSES_KEY: &str = "winning_addresses";
pub const WINNING_ADDRESSES: Map<&Addr, bool> = Map::new(WINNING_ADDRESSES_KEY);

pub const TICKET_PRICE_KEY: &str = "ticket_price";
pub const TICKET_PRICE: Item<Uint128> = Item::new(TICKET_PRICE_KEY);

pub const MERKLE_ROOT_PREFIX: &str = "merkle_root";
pub const MERKLE_ROOT: Item<String> = Item::new(MERKLE_ROOT_PREFIX);

pub const CLAIM_PREFIX: &str = "claim";
pub const CLAIM: Map<&Addr, bool> = Map::new(CLAIM_PREFIX);

pub const BIDS_PREFIX: &str = "bids";
pub const BIDS: Map<&Addr, Uint128> = Map::new("bids");