#![cfg(test)]

use std::borrow::BorrowMut;

use cosmwasm_std::{coins, Addr, Coin, Empty, Uint128, BlockInfo, Event };

use anyhow::Result as AnyResult;

use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_utils::{Scheduled, Duration};

use crate::ContractError;
use crate::contract::{execute, instantiate, query};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, StagesInfoResponse, BidResponse};
use crate::state::Stage;

fn mock_app() -> App {
    let mut app = App::default();
    let current_block = app.block_info();
    app.set_block(BlockInfo {
        height: 199_999,
        time: current_block.time,
        chain_id: current_block.chain_id
    });
    return app
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

pub fn contract_cosmos_arcade() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        execute,
        instantiate,
        query,
    );
    Box::new(contract)
}

pub fn create_cosmos_arcade(
    router: &mut App,
    owner: &Addr,
    ticket_price: Uint128,
    stage_bid: Stage,
    stage_claim_airdrop: Stage,
    stage_claim_prize: Stage
) -> AnyResult<Addr> {
    let cosmos_arcade_id = router.store_code(contract_cosmos_arcade());

    let msg = InstantiateMsg {
        owner: Some("owner0000".to_string()),
        cw20_token_address: "random0000".to_string(),
        ticket_price,
        stage_bid,
        stage_claim_airdrop,
        stage_claim_prize,
    };
    router
        .instantiate_contract(
            cosmos_arcade_id,
            owner.clone(),
            &msg,
            &[],
            "cosmos_arcade",
            None,
        )
}

fn get_stages_info(router: &App, contract_addr: &Addr) -> StagesInfoResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::StagesInfo {})
        .unwrap()
}

fn get_bid(router: &App, contract_addr: &Addr, address: String) -> BidResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Bid { address })
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

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = &valid_stages();

    // Valid instantiation
    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    let info = get_stages_info(&router, &cosmos_arcade_addr);
    assert_eq!(info.stage_bid.start, Scheduled::AtHeight(200_000));

    // Trigger error StageOverlap
    let mut stage_claim_airdrop_err = stage_claim_airdrop.clone();
    stage_claim_airdrop_err.start = Scheduled::AtHeight(100_000);
    let first = String::from("bid");
    let second = String::from("Claim airdrop");
    let err = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop_err,
        stage_claim_prize.clone()
    ).unwrap_err();
    assert_eq!(
        ContractError::StagesOverlap {first, second},
        err.downcast().unwrap());

    // Trigger error BidStartPassed
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 300_000,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let err = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap_err();
    assert_eq!(
        ContractError::BidStartPassed {},
        err.downcast().unwrap());

}


#[test]
fn valid_bid_no_change() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let bid_msg = ExecuteMsg::Bid {
        allocation: Uint128::new(1_000)
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10)
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap();

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(999_990), balance.amount);

    let err = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap_err();

    assert_eq!(ContractError::CannotBidMoreThanOnce {}, err.downcast().unwrap());
}

#[test]
fn valid_bid_with_change() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let bid_msg = ExecuteMsg::Bid {
        allocation: Uint128::new(1_000)
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(20)
    };

    let res = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change.clone()],
        )
        .unwrap();

    let event_transfer = Event::new("transfer")
        .add_attributes(vec![("recipient", "owner" ), ("sender", "contract0"), ("amount", "10ujuno")]);

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

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let bid_msg = ExecuteMsg::Bid {
        allocation: Uint128::new(1_000)
    };

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(1)
    };

    let err = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change],
        )
        .unwrap_err();

    assert_eq!(ContractError::TicketPriceNotPaid {}, err.downcast().unwrap());
}

#[test]
fn change_bid() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let change_bid_msg = ExecuteMsg::ChangeBid {allocation: Uint128::new(2_000)};
    
    let err = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &change_bid_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::BidNotPresent {}, err.downcast().unwrap());

    let bid_msg = ExecuteMsg::Bid {allocation: Uint128::new(1_000)};

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10)
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &bid_msg,
            &[valid_bid_no_change],
        )
        .unwrap();

    let info = get_bid(&router, &cosmos_arcade_addr, owner.to_string());
    assert_eq!(
        BidResponse {bid: Some(Uint128::new(1_000))},
        info
    );

    let change_bid_msg = ExecuteMsg::ChangeBid {allocation: Uint128::new(2_000)};
    
    let _res = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &change_bid_msg,
            &[],
        )
        .unwrap();

    let info = get_bid(&router, &cosmos_arcade_addr, owner.to_string());

    assert_eq!(BidResponse {bid: Some(Uint128::new(2_000))}, info);
}

#[test]
fn remove_bid() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger bidding start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 200_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let remove_bid_msg = ExecuteMsg::RemoveBid {};
    
    let err = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &remove_bid_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::BidNotPresent {}, err.downcast().unwrap());

    let bid_msg = ExecuteMsg::Bid {allocation: Uint128::new(1_000)};

    let valid_bid_no_change = Coin {
        denom: NATIVE_TOKEN_DENOM.into(),
        amount: Uint128::new(10)
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
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
            cosmos_arcade_addr.clone(),
            &remove_bid_msg,
            &[],
        )
        .unwrap();

    let info = get_bid(&router, &cosmos_arcade_addr, owner.to_string());

    assert_eq!(BidResponse {bid: None}, info);

    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(Uint128::new(1_000_000), balance.amount);
}

#[test]
fn register_merkle_root() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";
    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);

    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

    let cosmos_arcade_addr = create_cosmos_arcade(
        &mut router,
        &owner,
        ticket_price,
        stage_bid.clone(),
        stage_claim_airdrop.clone(),
        stage_claim_prize.clone()
    ).unwrap();

    // Trigger airdrop claim start
    let current_block = router.block_info();
    router.set_block(BlockInfo {
        height: 201_001,
        time: current_block.time,
        chain_id: current_block.chain_id
    });

    let remove_bid_msg = ExecuteMsg::RemoveBid {};
    
    let err = router
        .execute_contract(
            owner.clone(),
            cosmos_arcade_addr.clone(),
            &remove_bid_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(ContractError::BidNotPresent {}, err.downcast().unwrap());

}