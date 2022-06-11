#![cfg(test)]

use std::borrow::BorrowMut;

use cosmwasm_std::{coins, from_slice, Addr, BlockInfo, Coin, CustomQuery, Empty, Event, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg, Denom};

use cw20_base::contract::execute_send;
use cw20_base::ContractError as Cw20Error;

use anyhow::Result as AnyResult;

use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_utils::{Duration, Scheduled};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::contract::{execute, instantiate, query};
use crate::ContractError;

use crate::msg::{
    AmountResponse, BidResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, MerkleRootsResponse,
    QueryMsg, StagesResponse,
};
use crate::state::Stage;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MyCustomQuery {
    Ping {},
    Capitalized { text: String },
}

impl CustomQuery for MyCustomQuery {}

fn mock_app() -> App {
    let mut app = App::default();
    let current_block = app.block_info();
    app.set_block(BlockInfo {
        height: 199_999,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });
    return app;
}

fn valid_stages() -> (Stage, Stage, Stage) {
    let stage_bid = Stage {
        start: Scheduled::AtHeight(200_000),
        duration: Duration::Height(2),
    };

    let stage_claim_airdrop = Stage {
        start: Scheduled::AtHeight(201_000),
        duration: Duration::Height(2),
    };

    let stage_claim_prize = Stage {
        start: Scheduled::AtHeight(202_000),
        duration: Duration::Height(2),
    };

    return (stage_bid, stage_claim_airdrop, stage_claim_prize);
}

pub fn contract_game() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query);
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn create_game(
    router: &mut App,
    owner: &Addr,
    ticket_price: Uint128,
    bins: u8,
    stage_bid: Stage,
    stage_claim_airdrop: Stage,
    stage_claim_prize: Stage,
    cw20_token: Option<String>,
) -> AnyResult<Addr> {
    let game_id = router.store_code(contract_game());

    let msg = InstantiateMsg {
        owner: Some("owner0000".to_string()),
        cw20_token_address: cw20_token.unwrap_or("random0000".to_string()),
        ticket_price,
        bins,
        stage_bid,
        stage_claim_airdrop,
        stage_claim_prize,
    };
    router.instantiate_contract(game_id, owner.clone(), &msg, &[], "game", None)
}

fn create_cw20(
    router: &mut App,
    owner: &Addr,
    name: String,
    symbol: String,
    balance: Uint128,
) -> Cw20Contract {
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name,
        symbol,
        decimals: 2,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: balance,
        }],
        mint: None,
        marketing: None,
    };
    let addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH", None)
        .unwrap();
    Cw20Contract(addr)
}

fn get_stages(router: &App, contract_addr: &Addr) -> StagesResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Stages {})
        .unwrap()
}

fn get_bid(router: &App, contract_addr: &Addr, address: String) -> BidResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Bid { address })
        .unwrap()
}

fn get_config(router: &App, contract_addr: &Addr) -> ConfigResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Config {})
        .unwrap()
}

fn get_merkle_roots(router: &App, contract_addr: &Addr) -> MerkleRootsResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::MerkleRoot {})
        .unwrap()
}

fn get_claimed_amount_airdrop(router: &App, contract_addr: &Addr) -> AmountResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::AirdropClaimedAmount {})
        .unwrap()
}

fn bank_balance(router: &mut App, addr: &Addr, denom: String) -> Coin {
    router
        .wrap()
        .query_balance(addr.to_string(), denom)
        .unwrap()
}

#[test]
fn test_instantiate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = &valid_stages();

    // Valid instantiation
    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    let info = get_stages(&router, &game_addr);
    assert_eq!(info.stage_bid.start, Scheduled::AtHeight(200_000));

    // Trigger error StageOverlap
    let mut stage_claim_airdrop_err = stage_claim_airdrop.clone();
    stage_claim_airdrop_err.start = Scheduled::AtHeight(100_000);
    let first = String::from("bid");
    let second = String::from("Claim airdrop");
    let err = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop_err,
        stage_claim_prize.clone(),
        None,
    )
    .unwrap_err();
    assert_eq!(
        ContractError::StagesOverlap { first, second },
        err.downcast().unwrap()
    );

    // Trigger error BidStartPassed
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 300_000,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let err = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap_err();
    assert_eq!(ContractError::BidStartPassed {}, err.downcast().unwrap());
}

// ======================================================================================
// Tests bid
// ======================================================================================
#[test]
fn valid_bid_no_change() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let bid_msg = ExecuteMsg::Bid {
        bin: 1,
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10),
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap();

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(999_990), balance.amount);

    let err = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::CannotBidMoreThanOnce {},
        err.downcast().unwrap()
    );
}

#[test]
fn valid_bid_with_change() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let bid_msg = ExecuteMsg::Bid {
        bin: 1,
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(20),
    };

    let res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap();

    let event_transfer = Event::new("transfer").add_attributes(vec![
        ("recipient", "owner"),
        ("sender", "contract0"),
        ("amount", "10ujuno"),
    ]);

    let check_event_transfer = res.has_event(&event_transfer);
    assert_eq!(1, check_event_transfer as i32);

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(999_990), balance.amount);
}

#[test]
fn invalid_bid() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let bid_msg = ExecuteMsg::Bid {
        bin: 1,
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(1),
    };

    let err = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::TicketPriceNotPaid {},
        err.downcast().unwrap()
    );
}

#[test]
fn change_bid() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let change_bid_msg = ExecuteMsg::ChangeBid {
        bin: 2,
    };

    let err = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &change_bid_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::BidNotPresent {}, err.downcast().unwrap());

    let bid_msg = ExecuteMsg::Bid {
        bin: 1,
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10),
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change],
        )
        .unwrap();

    let info = get_bid(&router, &game_addr, owner.to_string());
    assert_eq!(
        BidResponse {
            bid: Some(1)
        },
        info
    );

    let change_bid_msg = ExecuteMsg::ChangeBid {
        bin: 2,
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &change_bid_msg,
            &[],
        )
        .unwrap();

    let info = get_bid(&router, &game_addr, owner.to_string());

    assert_eq!(
        BidResponse {
            bid: Some(2)
        },
        info
    );
}

#[test]
fn remove_bid() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    let remove_bid_msg = ExecuteMsg::RemoveBid {};

    let err = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &remove_bid_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::BidNotPresent {}, err.downcast().unwrap());

    let bid_msg = ExecuteMsg::Bid {
        bin: 1,
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10),
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change],
        )
        .unwrap();

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(999_990), balance.amount);

    let remove_bid_msg = ExecuteMsg::RemoveBid {};

    let _res = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &remove_bid_msg,
            &[],
        )
        .unwrap();

    let info = get_bid(&router, &game_addr, owner.to_string());

    assert_eq!(BidResponse { bid: None }, info);

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(1_000_000), balance.amount);
}

// ======================================================================================
// Tests Merkle root
// ======================================================================================
#[test]
fn register_merkle_root() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        None,
    )
    .unwrap();
    
    // Check Merkle roots properly saved
    let register_merkle_root_msg = ExecuteMsg::RegisterMerkleRoots {
        merkle_root_airdrop: "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37".to_string(),
        total_amount: None,
        merkle_root_game: "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d38".to_string()
    };

    let _res = router
        .execute_contract(
            Addr::unchecked("owner0000"),
            game_addr.clone(),
            &register_merkle_root_msg,
            &[],
        )
        .unwrap();

    let info = get_merkle_roots(&router, &game_addr);
    assert_eq!(
        info.merkle_root_airdrop,
        "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37".to_string()
    );
    assert_eq!(
        info.merkle_root_game,
        "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d38".to_string()
    );

    // Only the game owner can register the roots
    let err = router
        .execute_contract(
            owner.clone(),
            game_addr.clone(),
            &register_merkle_root_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());
}

const TEST_DATA_AIRDROP: &[u8] = include_bytes!("../testdata/airdrop_stage_1_test_data.json");
const TEST_DATA_GAME: &[u8] = include_bytes!("../testdata/airdrop_game_1_test_data.json");

#[derive(Deserialize, Debug)]
struct Encoded {
    account: String,
    amount: Uint128,
    root: String,
    proofs: Vec<String>,
}

#[test]
fn claim() {
    let mut router = mock_app();

    let test_data_airdrop: Encoded = from_slice(TEST_DATA_AIRDROP).unwrap();
    let test_data_game: Encoded = from_slice(TEST_DATA_GAME).unwrap();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    let bins: u8 = 10;

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    // Create the game token contract
    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(1_000_000),
    );

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cw20_token_address = Some(cw20_token.addr().to_string()).unwrap();
    let game_addr = create_game(
        &mut router,
        &owner,
        ticket_price,
        bins,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone(),
        Some(cw20_token_address.clone()),
    )
    .unwrap();

    // Check that the game has the correct cw20 token contract.
    let info = get_config(&router, &game_addr);
    assert_eq!(info.cw20_token_address, cw20_token_address);

    let owner_balance = cw20_token
        .balance::<App, Addr, MyCustomQuery>(&router, owner.clone())
        .unwrap();
    assert_eq!(owner_balance, Uint128::new(1_000_000));

    // Check that the correct Merkle roots are saved.
    let register_merkle_root_msg = ExecuteMsg::RegisterMerkleRoots {
        merkle_root_airdrop: test_data_airdrop.root,
        total_amount: Some(Uint128::new(1_000)),
        merkle_root_game: test_data_game.root
    };

    let _res = router
        .execute_contract(
            Addr::unchecked("owner0000"),
            game_addr.clone(),
            &register_merkle_root_msg,
            &[],
        )
        .unwrap();

    let info = get_merkle_roots(&router, &game_addr);
    assert_eq!(
        info.merkle_root_airdrop,
        "b45c1ea28b26adb13e412933c9e055b01fdf7585304b00cd8f1cb220aa6c5e88".to_string()
    );
    assert_eq!(info.total_amount, Uint128::new(1_000));

    let info = get_claimed_amount_airdrop(&router, &game_addr);
    assert_eq!(info.total_claimed, Uint128::new(0));

    // Transfer token to the game contract
    let send_token_msg = cw20::Cw20ExecuteMsg::Transfer {
        recipient: game_addr.clone().into(),
        amount: Uint128::new(110),
    };

    let _res = router
        .execute_contract(
            owner,
            Addr::unchecked(cw20_token_address),
            &send_token_msg,
            &[],
        )
        .unwrap();

    let game_balance = cw20_token
        .balance::<App, Addr, MyCustomQuery>(&router, game_addr.clone())
        .unwrap();
    assert_eq!(game_balance, Uint128::new(110));

    // Claim not allowed if claiming stage not active.
    let claim_airdrop_msg = ExecuteMsg::ClaimAirdrop {
        amount: test_data_airdrop.amount,
        proof: test_data_airdrop.proofs.clone(),
    };

    let err = router
        .execute_contract(
            Addr::unchecked(game_addr.to_string()),
            game_addr.clone(),
            &claim_airdrop_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::StageNotStarted {
            stage_name: String::from("claim airdrop")
        },
        err.downcast().unwrap()
    );

    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 201_001,
        time: current_block.time,
        chain_id: current_block.chain_id,
    });

    // Cannot be claimed a different amount than the one in the Merkle tree.
    let claim_airdrop_msg = ExecuteMsg::ClaimAirdrop {
        amount: Uint128::new(1_000),
        proof: test_data_airdrop.proofs.clone(),
    };

    let err = router
        .execute_contract(
            Addr::unchecked(test_data_airdrop.account.clone()),
            game_addr.clone(),
            &claim_airdrop_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::VerificationFailed {},
        err.downcast().unwrap()
    );

    let claim_airdrop_msg = ExecuteMsg::ClaimAirdrop {
        amount: test_data_airdrop.amount.clone(),
        proof: test_data_airdrop.proofs.clone(),
    };

    let _res = router
        .execute_contract(
            Addr::unchecked(test_data_airdrop.account.clone()),
            game_addr.clone(),
            &claim_airdrop_msg,
            &[],
        )
        .unwrap();

    let claimer_balance = cw20_token
        .balance::<App, Addr, MyCustomQuery>(&router, Addr::unchecked(test_data_airdrop.account.clone()))
        .unwrap();
    assert_eq!(claimer_balance, Uint128::new(100));

    let claim_airdrop_msg = ExecuteMsg::ClaimAirdrop {
        amount: test_data_airdrop.amount.clone(),
        proof: test_data_airdrop.proofs.clone(),
    };

    let err = router
        .execute_contract(
            Addr::unchecked(test_data_airdrop.account.clone()),
            game_addr.clone(),
            &claim_airdrop_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::AlreadyClaimed {}, err.downcast().unwrap());

    let game_balance = cw20_token
        .balance::<App, Addr, MyCustomQuery>(&router, game_addr.clone())
        .unwrap();
    assert_eq!(game_balance, Uint128::new(10));

    let info = get_claimed_amount_airdrop(&router, &game_addr);
    assert_eq!(info.total_claimed, Uint128::new(100));
}
