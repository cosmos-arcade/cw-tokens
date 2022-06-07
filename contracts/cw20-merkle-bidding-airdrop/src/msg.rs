use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Stage;
use cosmwasm_std::{Addr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Owner if none set to info.sender.
    pub owner: Option<String>,
    /// Address of the token.
    pub cw20_token_address: String,
    /// Price of the ticket to bid.
    pub ticket_price: Uint128,
    /// Info related to the bidding stage.
    pub stage_bid: Stage,
    /// Info related to the airdrop claiming stage.
    pub stage_claim_airdrop: Stage,
    /// Info related to the prize claiming stage.
    pub stage_claim_prize: Stage,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Update current contract configuration.
    UpdateConfig {
        /// NewOwner if non sent, contract gets locked. Recipients can receive airdrops
        /// but owner cannot register new stages.
        new_owner: Option<String>,
    },
    /// Place a bid.
    Bid {
        /// bidding allocation value
        allocation: Uint128,
    },
    /// Change the value of a previously placed bid.
    ChangeBid {
        /// input a value to change a previous bid
        allocation: Uint128,
    },
    /// Remove a previously placed bid.
    RemoveBid {},
    /// Register Merkle root in the contract.
    RegisterMerkleRoot {
        /// MerkleRoot is hex-encoded merkle root.
        merkle_root: String,
        total_amount: Option<Uint128>,
    },
    // Claim does not check if contract has enough funds, owner must ensure it.
    /// Claim airdrop allocation.
    ClaimAirdrop {
        amount: Uint128,
        /// Proof is hex-encoded merkle proof.
        proof: Vec<String>,
    },
    ClaimPrize {
        amount: Uint128,
        proof: Vec<String>,
    },
    // Withdraw the remaining Airdrop tokens after expire time (only owner)
    WithdrawAirdrop {
        address: Addr,
    },
    // Withdraw the remaining Prize tokens after expire time (only owner)
    WithdrawPrize {
        address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    StagesInfo {},
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StagesInfoResponse {
    pub stage_bid: Stage,
    pub stage_claim_airdrop: Stage,
    pub stage_claim_prize: Stage,
}
