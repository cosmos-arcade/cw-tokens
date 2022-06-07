#![cfg(test)]

use std::borrow::BorrowMut;

use cosmwasm_std::{coins, Addr, Coin, Empty, Uint128};

use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};
use cw_utils::{Scheduled, Duration};

use crate::contract::{execute, instantiate, query};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, StagesInfoResponse};
use crate::state::Stage;

fn mock_app() -> App {
    App::default()
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
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

pub fn create_cosmos_arcade(router: &mut App, owner: &Addr, ticket_price: Uint128) -> Addr {
    let cosmos_arcade_id = router.store_code(contract_cosmos_arcade());

    let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

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
        .unwrap()
}

fn get_info(router: &App, contract_addr: &Addr) -> StagesInfoResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::StagesInfo {})
        .unwrap()
}

#[test]
// receive cw20 tokens and release upon approval 
fn test_instantiate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "ujuno";

    let owner = Addr::unchecked("owner");
    let ticket_price = Uint128::new(10);
    let funds = coins(1_000_000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cosmos_arcade_addr = create_cosmos_arcade(&mut router, &owner, ticket_price);

    println!("{}", cosmos_arcade_addr);

    let info = get_info(&router, &cosmos_arcade_addr);

    println!("{:?}", info);

    assert_ne!(0, 0);
}
