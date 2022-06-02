use cosmwasm_std::StdError;
use cw_utils::{Expiration, Scheduled};
use hex::FromHexError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Hex(#[from] FromHexError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid input")]
    InvalidInput {},

    #[error("Already claimed")]
    Claimed {},

    #[error("Wrong length")]
    WrongLength {},

    #[error("Verification failed")]
    VerificationFailed {},

    #[error("Cannot migrate from different contract type: {previous_contract}")]
    CannotMigrate { previous_contract: String },

    #[error("Claim Airdrop stage has expired")]
    ClaimAirdropStageExpired {},

    #[error("Claim Prize stage has expired")]
    ClaimPrizeStageExpired {},

    #[error("Bid stage hasn't begun")]
    BidStageNotBegun {},

    #[error("Claim Airdrop stage hasn't begun")]
    ClaimAirdropStageNotBegun {},

    #[error("Claim Prize stage hasn't begun")]
    ClaimPrizeStageNotBegun {},

    #[error("Bid stage has expired")]
    BidStageExpired {},

    #[error("You must pay ticket price to bid")]
    TicketPriceNotPaid {},

    #[error("You didn't bid anything yet")]
    NonExistentBid {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("Incorrect native denom: provided: {provided}, required: {required}")]
    IncorrectNativeDenom { provided: String, required: String },
}
