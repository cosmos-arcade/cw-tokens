use cosmwasm_std::{Addr, Uint128, Coin};
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

/// Storage to save the first game ticket price.
pub const TICKET_PRICE_KEY: &str = "ticket_price";
pub const TICKET_PRICE: Item<Coin> = Item::new(TICKET_PRICE_KEY);

/// Storage to save the number of allowed bins for the game.
pub const BINS_PREFIX: &str = "bins";
pub const BINS: Item<u8> = Item::new(BIDS_PREFIX);

/// Storage to manage the bid of each address.
pub const BIDS_PREFIX: &str = "bids";
pub const BIDS: Map<&Addr, u8> = Map::new("bids");

/// Storage for the Merkle root of the airdrop.
pub const MERKLE_ROOT_AIRDROP_PREFIX: &str = "merkle_root_airdrop";
pub const MERKLE_ROOT_AIRDROP: Item<String> = Item::new(MERKLE_ROOT_AIRDROP_PREFIX);

/// Storage for the Merkle root of the game.
pub const MERKLE_ROOT_GAME_PREFIX: &str = "merkle_root_game";
pub const MERKLE_ROOT_GAME: Item<String> = Item::new(MERKLE_ROOT_GAME_PREFIX);

/// Storage for the amount of airdropped tokens claimed.
/// This variable will consider:
/// - Amount from simple airdrop.
/// - Amount airdropped to winners of the first game.
pub const CLAIMED_AIRDROP_AMOUNT_PREFIX: &str = "claimed_amount";
pub const CLAIMED_AIRDROP_AMOUNT: Item<Uint128> = Item::new(CLAIMED_AIRDROP_AMOUNT_PREFIX);

/// Storage for the amount of the prize coming from the tickets claimed.
pub const CLAIMED_PRIZE_AMOUNT_PREFIX: &str = "claimed_prize";
pub const CLAIMED_PRIZE_AMOUNT: Item<Uint128> = Item::new(CLAIMED_PRIZE_AMOUNT_PREFIX);

/// Storage to save the number of winning addresses.
pub const WINNERS_PREFIX: &str = "winners";
pub const WINNERS: Item<Uint128> = Item::new(WINNERS_PREFIX);

/// Storage to keep track of the total prize from game tickets.
pub const TOTAL_TICKET_PRIZE_KEY: &str = "total_ticket_prize";
pub const TOTAL_TICKET_PRIZE: Item<Uint128> = Item::new(TOTAL_TICKET_PRIZE_KEY);

/// Total amount of tokens for the plain airdrop.
pub const TOTAL_AIRDROP_AMOUNT_PREFIX: &str = "total_amount_airdrop";
pub const TOTAL_AIRDROP_AMOUNT: Item<Uint128> = Item::new(TOTAL_AIRDROP_AMOUNT_PREFIX);

/// Total amount of tokens for the airdrop of the game winners.
pub const TOTAL_AIRDROP_GAME_AMOUNT_PREFIX: &str = "total_amount_game";
pub const TOTAL_AIRDROP_GAME_AMOUNT: Item<Uint128> = Item::new(TOTAL_AIRDROP_GAME_AMOUNT_PREFIX);

/// Storage to save if an address has claimed the airdrop or not.
pub const CLAIM_AIRDROP_PREFIX: &str = "claim_airdrop";
pub const CLAIM_AIRDROP: Map<&Addr, bool> = Map::new(CLAIM_AIRDROP_PREFIX);

/// Storage to save if a winning address has claimed the prize or not.
pub const CLAIM_PRIZE_PREFIX: &str = "claim_prize";
pub const CLAIM_PRIZE: Map<&Addr, bool> = Map::new(CLAIM_PRIZE_PREFIX);