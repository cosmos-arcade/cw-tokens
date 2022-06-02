use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Stage;
use cosmwasm_std::Uint128;
use cw_utils::{Expiration, Scheduled};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Owner if none set to info.sender.
    pub owner: Option<String>,
    pub cw20_token_address: String,
    /// price to play
    pub ticket_price: Uint128,
    /// stage struct for bid phase
    pub stage_bid: Stage,
    /// stage struct for claim airdrop phase
    pub stage_claim_airdrop: Stage,
    /// stage struct for claim prize phase
    pub stage_claim_prize: Stage,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        /// NewOwner if non sent, contract gets locked. Recipients can receive airdrops
        /// but owner cannot register new stages.
        new_owner: Option<String>,
    },
    Bid {
        /// bidding allocation value
        allocation: Uint128,
    },
    ChangeBid {
        /// input a value to change a previous bid
        allocation: Uint128,
    },
    RemoveBid {},
    RegisterMerkleRoot {
        /// MerkleRoot is hex-encoded merkle root.
        merkle_root: String,
    },
    // Claim does not check if contract has enough funds, owner must ensure it.
    ClaimAirdrop {
        amount: Uint128,
        /// Proof is hex-encoded merkle proof.
        proof: Vec<String>,
    },
    ClaimPrize {},
    // Withdraw the remaining tokens after expire time (only owner)
    /*Withdraw {
        address: String,
    },*/
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    MerkleRoot {},
    Bid {},
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub owner: Option<String>,
    pub cw20_token_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MerkleRootResponse {
    /// MerkleRoot is hex-encoded merkle root.
    pub merkle_root: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidResponse {
    bid: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
