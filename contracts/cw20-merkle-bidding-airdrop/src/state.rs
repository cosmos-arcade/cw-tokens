use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::{Duration, Scheduled};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Struct to manage the contract configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Owner If None set, contract is frozen.
    pub owner: Option<Addr>,
    pub cw20_token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
/// Struct to manage start and end of static stages.
pub struct Stage {
    /// Starting event for the stage.
    pub start: Scheduled,
    /// Ending event for the stage.
    pub duration: Duration,
}

/// Storage to manage contract configuration.
pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

/// Storage for the bid stage info.
pub const STAGE_BID_KEY: &str = "stage_bid";
pub const STAGE_BID: Item<Stage> = Item::new(STAGE_BID_KEY);

/// Storage for the airdrop stage info.
pub const STAGE_CLAIM_AIRDROP_KEY: &str = "stage_claim_airdrop";
pub const STAGE_CLAIM_AIRDROP: Item<Stage> = Item::new(STAGE_CLAIM_AIRDROP_KEY);

/// Storage for the claiming prize stage info.
pub const STAGE_CLAIM_PRIZE_KEY: &str = "stage_claim_prize";
pub const STAGE_CLAIM_PRIZE: Item<Stage> = Item::new(STAGE_CLAIM_PRIZE_KEY);

/// Storage to save the first game ticket prize.
pub const TICKET_PRICE_KEY: &str = "ticket_price";
pub const TICKET_PRICE: Item<Uint128> = Item::new(TICKET_PRICE_KEY);

pub const MERKLE_ROOT_PREFIX: &str = "merkle_root";
pub const MERKLE_ROOT: Item<String> = Item::new(MERKLE_ROOT_PREFIX);

/// Storage to save if an addres has claimed the airdrop or not.
pub const CLAIM_AIRDROP_PREFIX: &str = "claim_airdrop";
pub const CLAIM_AIRDROP: Map<&Addr, bool> = Map::new(CLAIM_AIRDROP_PREFIX);

/// Storage to save if a winning addres has claimed the prize or not.
pub const CLAIM_PRIZE_PREFIX: &str = "claim_prize";
pub const CLAIM_PRIZE: Map<&Addr, bool> = Map::new(CLAIM_PRIZE_PREFIX);

/// Storage to manage the bid of each addres.
pub const BIDS_PREFIX: &str = "bids";
pub const BIDS: Map<&Addr, Uint128> = Map::new("bids");

pub const CLAIMED_AIRDROP_AMOUNT_PREFIX: &str = "claimed_amount";
pub const CLAIMED_AIRDROP_AMOUNT: Item<Uint128> = Item::new(CLAIMED_AIRDROP_AMOUNT_PREFIX);

pub const TOTAL_AIRDROP_AMOUNT_PREFIX: &str = "total_amount";
pub const TOTAL_AIRDROP_AMOUNT: Item<Uint128> = Item::new(TOTAL_AIRDROP_AMOUNT_PREFIX);
