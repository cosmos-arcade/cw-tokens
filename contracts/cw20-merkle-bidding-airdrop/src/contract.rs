#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Binary, BlockInfo, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Timestamp, Uint128, CosmosMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ExecuteMsg;
use cw_utils::{Expiration, Scheduled};
use sha2::Digest;
use std::convert::TryInto;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MerkleRootResponse, MigrateMsg,
    QueryMsg, BidResponse,
};
use crate::state::{
    self, Config, Stage, BIDS, CLAIM, CONFIG, MERKLE_ROOT, STAGE_BID, STAGE_CLAIM_AIRDROP,
    STAGE_CLAIM_PRIZE, TICKET_PRICE,
};

// Version info, for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-merkle-airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = msg
        .owner
        .map_or(Ok(info.sender), |o| deps.api.addr_validate(&o))?;

    let config = Config {
        owner: Some(owner),
        cw20_token_address: deps.api.addr_validate(&msg.cw20_token_address)?,
    };
    CONFIG.save(deps.storage, &config)?;

    STAGE_BID.save(deps.storage, &msg.stage_bid)?;
    STAGE_CLAIM_AIRDROP.save(deps.storage, &msg.stage_claim_airdrop)?;
    STAGE_CLAIM_PRIZE.save(deps.storage, &msg.stage_claim_prize)?;
    TICKET_PRICE.save(deps.storage, &msg.ticket_price)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_owner } => execute_update_config(deps, env, info, new_owner),
        ExecuteMsg::Bid { allocation } => execute_bid(deps, env, info, allocation),
        ExecuteMsg::ChangeBid { allocation } => execute_change_bid(deps, env, info, allocation),
        ExecuteMsg::RemoveBid {} => execute_remove_bid(deps, env, info),
        ExecuteMsg::RegisterMerkleRoot {
            merkle_root,
        } => execute_register_merkle_root(deps, env, info, merkle_root),
        /*ExecuteMsg::ClaimAirdrop { amount, proof } => {
            execute_claim_airdrop(deps, env, info, amount, proof)
        }
        ExecuteMsg::ClaimPrize {} => execute_claim_prize(deps, env, info),
        ExecuteMsg::Withdraw { stage, address } => {
            execute_withdraw(deps, env, info, stage, address)
        }*/
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
) -> Result<Response, ContractError> {
    // authorize owner
    let cfg = CONFIG.load(deps.storage)?;
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    // if owner some validated to addr, otherwise set to none
    let mut tmp_owner = None;
    if let Some(addr) = new_owner {
        tmp_owner = Some(deps.api.addr_validate(&addr)?)
    }

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.owner = tmp_owner;
        Ok(exists)
    })?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    allocation: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // if owner set validate, otherwise unauthorized
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let ticket_price = TICKET_PRICE.load(deps.storage)?;
    let stage_bid = STAGE_BID.load(deps.storage)?;

    //you can't bid if Bid Phase didn't start yet
    if !stage_bid.start.is_triggered(&_env.block) {
        return Err(ContractError::BidStageNotBegun {});
    }

    //you can't bid if Bid Phase is ended
    if stage_bid.end.is_expired(&_env.block) {
        return Err(ContractError::BidStageEnded {});
    }

    //if ticket price not paid, you can't bid
    if get_amount_for_denom(&info.funds, "ujuno").amount < ticket_price {
        return Err(ContractError::TicketPriceNotPaid {});
    }

    BIDS.update(
        deps.storage,
        &info.sender,
        |allocation: Option<Uint128>| -> StdResult<_> { Ok(allocation.unwrap()) },
    )?;

    let res = Response::new()
        .add_attribute("action", "bid")
        .add_attribute("player", info.sender)
        .add_attribute("allocation", allocation);
    Ok(res)
}

pub fn execute_register_merkle_root(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    merkle_root: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // if owner set validate, otherwise unauthorized
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    // check merkle root length
    let mut root_buf: [u8; 32] = [0; 32];
    hex::decode_to_slice(&merkle_root, &mut root_buf)?;

    let stage_null: u8 = 0;

    MERKLE_ROOT.save(deps.storage, &merkle_root)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_merkle_root"),
        attr("merkle_root", merkle_root),
    ]))
}

pub fn execute_change_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    allocation: Uint128,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // if owner set validate, otherwise unauthorized
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let stage_bid = STAGE_BID.load(deps.storage)?;
    
    //you can't change bid if Bid Phase didn't start yet
    if !stage_bid.start.is_triggered(&_env.block) {
        return Err(ContractError::BidStageNotBegun {});
    }

    //you can't change bid if Bid Phase is ended
    if stage_bid.end.is_expired(&_env.block) {
        return Err(ContractError::BidStageEnded {});
    }

    let bid = BIDS.load(deps.storage, &info.sender)?;
    
    // you must have bid before, to change it
    if bid == Uint128::zero() {
        return Err(ContractError::NonExistentBid {});
    }

    BIDS.update(
        deps.storage,
        &info.sender,
        |allocation: Option<Uint128>| -> StdResult<_> { Ok(allocation.unwrap()) },
    )?;

    let res = Response::new()
        .add_attribute("action", "change_bid")
        .add_attribute("player", info.sender)
        .add_attribute("allocation", allocation);
    Ok(res)

}

pub fn execute_remove_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // if owner set validate, otherwise unauthorized
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let ticket_price = TICKET_PRICE.load(deps.storage)?;
    let stage_bid = STAGE_BID.load(deps.storage)?;
    
    //you can't remove bid if Bid Phase didn't start yet
    if !stage_bid.start.is_triggered(&_env.block) {
        return Err(ContractError::BidStageNotBegun {});
    }

    //you can't remove bid if Bid Phase is ended
    if stage_bid.end.is_expired(&_env.block) {
        return Err(ContractError::BidStageEnded {});
    }

    let bid = BIDS.load(deps.storage, &info.sender)?;
    
    // you must have bid before, to remove it
    if bid == Uint128::zero() {
        return Err(ContractError::NonExistentBid {});
    }

    BIDS.remove(deps.storage, &info.sender);

    
    send_back_bid(&info.sender, bid, "ujuno");

    let res = Response::new()
        .add_attribute("action", "remove_bid")
        .add_attribute("player", info.sender);
    Ok(res)

}


fn get_amount_for_denom(coins: &[Coin], denom: &str) -> Coin {
    let amount: Uint128 = coins
        .iter()
        .filter(|c| c.denom == denom)
        .map(|c| c.amount)
        .sum();
    Coin {
        amount,
        denom: denom.to_string(),
    }
}

fn send_back_bid(recipient: &Addr, bid: Uint128, denom: &str) -> CosmosMsg {

    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: bid,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg  
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::MerkleRoot {} => to_binary(&query_merkle_root(deps)?),
        QueryMsg::Bid {} => to_binary(&query_bid(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.map(|o| o.to_string()),
        cw20_token_address: cfg.cw20_token_address.to_string(),
    })
}

pub fn query_merkle_root(deps: Deps) -> StdResult<MerkleRootResponse> {
    let merkle_root = MERKLE_ROOT.load(deps.storage)?; 
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.may_load(deps.storage)?;
    let stage_claim_prize = STAGE_CLAIM_PRIZE.load(deps.storage)?;

    let resp = MerkleRootResponse {merkle_root};

    Ok(resp)
}

pub fn query_bid(deps: Deps) -> StdResult<BidResponse> {

}




#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, from_slice, CosmosMsg, SubMsg};
    use serde::Deserialize;

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "anchor0000".to_string(),
            ticket_price: Uint128::from(555555u128),
            stage_bid: Stage {
                start: Scheduled::AtHeight(555),
                end: Expiration::AtHeight(555),
            },
            stage_claim_airdrop: Stage {
                start: Scheduled::AtHeight(777),
                end: Expiration::AtHeight(777),
            },
            stage_claim_prize: Stage {
                start: Scheduled::AtHeight(999),
                end: Expiration::AtHeight(999),
            },
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // it worked, let's query the state
        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0000", config.owner.unwrap().as_str());
        assert_eq!("anchor0000", config.cw20_token_address.as_str());
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: None,
            cw20_token_address: "anchor0000".to_string(),
            ticket_price: Uint128::from(555555u128),
            stage_bid: Stage {
                start: Scheduled::AtHeight(555),
                end: Expiration::AtHeight(555),
            },
            stage_claim_airdrop: Stage {
                start: Scheduled::AtHeight(777),
                end: Expiration::AtHeight(777),
            },
            stage_claim_prize: Stage {
                start: Scheduled::AtHeight(999),
                end: Expiration::AtHeight(999),
            },
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // update owner
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: Some("owner0001".to_string()),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), env, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0001", config.owner.unwrap().as_str());

        // Unauthorized err
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig { new_owner: None };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});
    }

    #[test]
    fn register_merkle_root() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "anchor0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // register new merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37"
                .to_string(),
            expiration: None,
            start: None,
            total_amount: None,
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "register_merkle_root"),
                attr("stage", "1"),
                attr(
                    "merkle_root",
                    "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37"
                ),
                attr("total_amount", "0")
            ]
        );

        let res = query(deps.as_ref(), env.clone(), QueryMsg::LatestStage {}).unwrap();
        let latest_stage: LatestStageResponse = from_binary(&res).unwrap();
        assert_eq!(1u8, latest_stage.latest_stage);

        let res = query(
            deps.as_ref(),
            env,
            QueryMsg::MerkleRoot {
                stage: latest_stage.latest_stage,
            },
        )
        .unwrap();
        let merkle_root: MerkleRootResponse = from_binary(&res).unwrap();
        assert_eq!(
            "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37".to_string(),
            merkle_root.merkle_root
        );
    }

    const TEST_DATA_1: &[u8] = include_bytes!("../testdata/airdrop_stage_1_test_data.json");
    const TEST_DATA_2: &[u8] = include_bytes!("../testdata/airdrop_stage_2_test_data.json");

    #[derive(Deserialize, Debug)]
    struct Encoded {
        account: String,
        amount: Uint128,
        root: String,
        proofs: Vec<String>,
    }

    /*#[test]
    fn claim() {
        // Run test 1
        let mut deps = mock_dependencies();
        let test_data: Encoded = from_slice(TEST_DATA_1).unwrap();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: test_data.root,
            expiration: None,
            start: None,
            total_amount: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        let msg = ExecuteMsg::Claim {
            amount: test_data.amount,
            stage: 1u8,
            proof: test_data.proofs,
        };

        let env = mock_env();
        let info = mock_info(test_data.account.as_str(), &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: test_data.account.clone(),
                amount: test_data.amount,
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "claim"),
                attr("stage", "1"),
                attr("address", test_data.account.clone()),
                attr("amount", test_data.amount)
            ]
        );

        // Check total claimed on stage 1
        assert_eq!(
            from_binary::<TotalClaimedResponse>(
                &query(
                    deps.as_ref(),
                    env.clone(),
                    QueryMsg::TotalClaimed { stage: 1 }
                )
                .unwrap()
            )
            .unwrap()
            .total_claimed,
            test_data.amount
        );

        // Check address is claimed
        assert!(
            from_binary::<IsClaimedResponse>(
                &query(
                    deps.as_ref(),
                    env.clone(),
                    QueryMsg::IsClaimed {
                        stage: 1,
                        address: test_data.account
                    }
                )
                .unwrap()
            )
            .unwrap()
            .is_claimed
        );

        // check error on double claim
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Claimed {});

        // Second test
        let test_data: Encoded = from_slice(TEST_DATA_2).unwrap();

        // register new drop
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: test_data.root,
            expiration: None,
            start: None,
            total_amount: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // Claim next airdrop
        let msg = ExecuteMsg::Claim {
            amount: test_data.amount,
            stage: 2u8,
            proof: test_data.proofs,
        };

        let env = mock_env();
        let info = mock_info(test_data.account.as_str(), &[]);
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let expected: SubMsg<_> = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: test_data.account.clone(),
                amount: test_data.amount,
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "claim"),
                attr("stage", "2"),
                attr("address", test_data.account),
                attr("amount", test_data.amount)
            ]
        );

        // Check total claimed on stage 2
        assert_eq!(
            from_binary::<TotalClaimedResponse>(
                &query(deps.as_ref(), env, QueryMsg::TotalClaimed { stage: 2 }).unwrap()
            )
            .unwrap()
            .total_claimed,
            test_data.amount
        );
    }*/

    const TEST_DATA_1_MULTI: &[u8] =
        include_bytes!("../testdata/airdrop_stage_1_test_multi_data.json");

    #[derive(Deserialize, Debug)]
    struct Proof {
        account: String,
        amount: Uint128,
        proofs: Vec<String>,
    }

    #[derive(Deserialize, Debug)]
    struct MultipleData {
        total_claimed_amount: Uint128,
        root: String,
        accounts: Vec<Proof>,
    }

    #[test]
    fn multiple_claim() {
        // Run test 1
        let mut deps = mock_dependencies();
        let test_data: MultipleData = from_slice(TEST_DATA_1_MULTI).unwrap();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: test_data.root,
            expiration: None,
            start: None,
            total_amount: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // Loop accounts and claim
        for account in test_data.accounts.iter() {
            let msg = ExecuteMsg::Claim {
                amount: account.amount,
                stage: 1u8,
                proof: account.proofs.clone(),
            };

            let env = mock_env();
            let info = mock_info(account.account.as_str(), &[]);
            let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
            let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "token0000".to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: account.account.clone(),
                    amount: account.amount,
                })
                .unwrap(),
            }));
            assert_eq!(res.messages, vec![expected]);

            assert_eq!(
                res.attributes,
                vec![
                    attr("action", "claim"),
                    attr("stage", "1"),
                    attr("address", account.account.clone()),
                    attr("amount", account.amount)
                ]
            );
        }

        // Check total claimed on stage 1
        let env = mock_env();
        assert_eq!(
            from_binary::<TotalClaimedResponse>(
                &query(deps.as_ref(), env, QueryMsg::TotalClaimed { stage: 1 }).unwrap()
            )
            .unwrap()
            .total_claimed,
            test_data.total_claimed_amount
        );
    }

    // Check expiration. Chain height in tests is 12345
    #[test]
    fn stage_expires() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
            ticket_price: Uint128::from(555555u128),
            stage_bid: Stage {
                start: Scheduled::AtHeight(555),
                end: Expiration::AtHeight(555),
            },
            stage_claim_airdrop: Stage {
                start: Scheduled::AtHeight(777),
                end: Expiration::AtHeight(777),
            },
            stage_claim_prize: Stage {
                start: Scheduled::AtHeight(999),
                end: Expiration::AtHeight(999),
            },
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // can register merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc"
                .to_string(),
            total_amount: None,
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // can't claim expired
        /*let msg = ExecuteMsg::Claim {
            amount: Uint128::new(5),
            stage: 1u8,
            proof: vec![],
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::StageExpired {
                stage: 1,
                expiration: Expiration::AtHeight(100)
            }
        )*/
    #[test]
    fn cant_burn() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // can register merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc"
                .to_string(),
            expiration: Some(Expiration::AtHeight(12346)),
            start: None,
            total_amount: Some(Uint128::new(100000)),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // Can't burn not expired stage
        let msg = ExecuteMsg::Burn { stage: 1u8 };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::StageNotExpired {
                stage: 1,
                expiration: Expiration::AtHeight(12346)
            }
        )
    }

    #[test]
    fn can_burn() {
        let mut deps = mock_dependencies();
        let test_data: Encoded = from_slice(TEST_DATA_1).unwrap();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let mut env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: test_data.root,
            expiration: Some(Expiration::AtHeight(12500)),
            start: None,
            total_amount: Some(Uint128::new(10000)),
        };
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            amount: test_data.amount,
            stage: 1u8,
            proof: test_data.proofs,
        };

        let info = mock_info(test_data.account.as_str(), &[]);
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: test_data.account.clone(),
                amount: test_data.amount,
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "claim"),
                attr("stage", "1"),
                attr("address", test_data.account.clone()),
                attr("amount", test_data.amount)
            ]
        );

        // makes the stage expire
        env.block.height = 12501;

        // Can burn after expired stage
        let msg = ExecuteMsg::Burn { stage: 1u8 };

        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();

        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::new(9900),
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "burn"),
                attr("stage", "1"),
                attr("address", "owner0000"),
                attr("amount", Uint128::new(9900)),
            ]
        );
    }

    #[test]
    fn cant_withdraw() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // can register merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc"
                .to_string(),
            expiration: Some(Expiration::AtHeight(12346)),
            start: None,
            total_amount: Some(Uint128::new(100000)),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // Can't withdraw not expired stage
        let msg = ExecuteMsg::Withdraw {
            stage: 1u8,
            address: "addr0005".to_string(),
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::StageNotExpired {
                stage: 1,
                expiration: Expiration::AtHeight(12346)
            }
        )
    }

    #[test]
    fn can_withdraw() {
        let mut deps = mock_dependencies();
        let test_data: Encoded = from_slice(TEST_DATA_1).unwrap();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let mut env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: test_data.root,
            expiration: Some(Expiration::AtHeight(12500)),
            start: None,
            total_amount: Some(Uint128::new(10000)),
        };
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Claim some tokens
        let msg = ExecuteMsg::Claim {
            amount: test_data.amount,
            stage: 1u8,
            proof: test_data.proofs,
        };

        let info = mock_info(test_data.account.as_str(), &[]);
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: test_data.account.clone(),
                amount: test_data.amount,
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "claim"),
                attr("stage", "1"),
                attr("address", test_data.account.clone()),
                attr("amount", test_data.amount)
            ]
        );

        // makes the stage expire
        env.block.height = 12501;

        // Can burn after expired stage
        let msg = ExecuteMsg::Withdraw {
            stage: 1u8,
            address: "addr0005".to_string(),
        };

        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();

        let expected = SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "token0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(9900),
                recipient: "addr0005".to_string(),
            })
            .unwrap(),
        }));
        assert_eq!(res.messages, vec![expected]);

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "withdraw"),
                attr("stage", "1"),
                attr("address", "owner0000"),
                attr("amount", Uint128::new(9900)),
                attr("recipient", "addr0005")
            ]
        );
    }

    #[test]
    fn stage_starts() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // can register merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc"
                .to_string(),
            expiration: None,
            start: Some(Scheduled::AtHeight(200_000)),
            total_amount: None,
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // can't claim expired
        let msg = ExecuteMsg::Claim {
            amount: Uint128::new(5),
            stage: 1u8,
            proof: vec![],
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::StageNotBegun {
                stage: 1,
                start: Scheduled::AtHeight(200_000)
            }
        )
    }

    #[test]
    fn owner_freeze() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "token0000".to_string(),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // can register merkle root
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "5d4f48f147cb6cb742b376dce5626b2a036f69faec10cd73631c791780e150fc"
                .to_string(),
            expiration: None,
            start: None,
            total_amount: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // can update owner
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: Some("owner0001".to_string()),
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // freeze contract
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::UpdateConfig { new_owner: None };

        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // cannot register new drop
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "ebaa83c7eaf7467c378d2f37b5e46752d904d2d17acd380b24b02e3b398b3e5a"
                .to_string(),
            expiration: None,
            start: None,
            total_amount: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // cannot update config
        let env = mock_env();
        let info = mock_info("owner0001", &[]);
        let msg = ExecuteMsg::RegisterMerkleRoot {
            merkle_root: "ebaa83c7eaf7467c378d2f37b5e46752d904d2d17acd380b24b02e3b398b3e5a"
                .to_string(),
            expiration: None,
            start: None,
            total_amount: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});
    }
}
