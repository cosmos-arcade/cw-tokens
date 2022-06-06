#![cfg(test)]

use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper};

use crate::contract::{execute, instantiate, query, reply};

fn mock_app() -> App {
    App::default()
}

pub fn contract() -> Box<dyn Contract<Empty>>{
    let contract = ContractWrapper::new(
        execute,
        instantiate,
        query,
    ).with_reply(reply);
    Box::new(contract)
}

