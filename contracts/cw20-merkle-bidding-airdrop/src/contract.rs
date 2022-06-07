#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Timestamp, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ExecuteMsg;
use sha2::Digest;
use std::convert::TryInto;

use crate::error::ContractError;
use crate::msg::{
    BidResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, MerkleRootResponse, MigrateMsg,
    QueryMsg, StagesInfoResponse,
};
use crate::state::{
    Config, BIDS, CLAIMED_AIRDROP_AMOUNT, CLAIM_AIRDROP, CONFIG, MERKLE_ROOT, STAGE_BID,
    STAGE_CLAIM_AIRDROP, STAGE_CLAIM_PRIZE, TICKET_PRICE, TOTAL_AIRDROP_AMOUNT,
};

// Version info, for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-merkle-airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // If owner not in message, set it as sender.
    let owner = msg
        .owner
        .map_or(Ok(info.sender), |o| deps.api.addr_validate(&o))?;

    let config = Config {
        owner: Some(owner),
        cw20_token_address: deps.api.addr_validate(&msg.cw20_token_address)?,
    };

    let stage_bid_end = (msg.stage_bid.start + msg.stage_bid.duration)?;
    let stage_claim_airdrop_end =
        (msg.stage_claim_airdrop.start + msg.stage_claim_airdrop.duration)?;

    // Bid stage have to start after contract instantiation.
    if msg.stage_bid.start.is_triggered(&env.block) {
        return Err(ContractError::BidStartPassed {});
    }

    if stage_bid_end > msg.stage_claim_airdrop.start {
        let first = String::from("bid");
        let second = String::from("Claim airdrop");
        return Err(ContractError::StagesOverlap { first, second });
    }

    if stage_claim_airdrop_end > msg.stage_claim_prize.start {
        let first = String::from("claim aidrop");
        let second = String::from("Claim prize");
        return Err(ContractError::StagesOverlap { first, second });
    }

    // Save contract's state after validity check avoid useless computation.
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
            total_amount,
        } => execute_register_merkle_root(deps, env, info, merkle_root, total_amount),
        ExecuteMsg::ClaimAirdrop { amount, proof } => {
            execute_claim_airdrop(deps, env, info, amount, proof)
        }
        ExecuteMsg::ClaimPrize { amount, proof } => todo!(),
        ExecuteMsg::WithdrawAirdrop { address } => {
            execute_withdraw_airdrop(deps, env, info, &address)
        }
        ExecuteMsg::WithdrawPrize { address } => todo!(),
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
    // If owner not present the config cannot be updated
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    // Just the owner can update the config
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
    let stage_bid = STAGE_BID.load(deps.storage)?;

    // Bid not allowed if bid phase didn't start yet.
    if !stage_bid.start.is_triggered(&_env.block) {
        return Err(ContractError::BidStageNotBegun {});
    }

    // Bid not allowed if bid phase is ended.
    let stage_bid_end = (stage_bid.start + stage_bid.duration)?;
    if stage_bid_end.is_triggered(&_env.block) {
        return Err(ContractError::BidStageExpired {});
    }

    let bid = BIDS.load(deps.storage, &info.sender)?;

    //bid can't be less than 0
    if bid < Uint128::zero() {
        return Err(ContractError::IncorrectBidValue {});
    }

    let ticket_price = TICKET_PRICE.load(deps.storage)?;

    // TODO: Controllare se bid già esistente

    // TODO: Bid minori di zero non vanno bene

    // If ticket price not paid, bid is not allowed.
    let fund_sent = get_amount_for_denom(&info.funds, "ujuno");
    if fund_sent.amount < ticket_price {
        return Err(ContractError::TicketPriceNotPaid {});
    }

    // If sender sent funds higher than ticket price, return change.
    let mut transfer_msg: Vec<CosmosMsg> = vec![];
    if fund_sent.amount > ticket_price {
        transfer_msg.push(get_bank_transfer_to_msg(
            &info.sender,
            &fund_sent.denom,
            fund_sent.amount - ticket_price,
        ))
    }

    // Save address and bid
    BIDS.save(deps.storage, &info.sender, &allocation)?;

    let res = Response::new()
        .add_messages(transfer_msg)
        .add_attribute("action", "bid")
        .add_attribute("player", info.sender)
        .add_attribute("allocation", allocation);
    Ok(res)
}

pub fn execute_change_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    allocation: Uint128,
) -> Result<Response, ContractError> {
    let stage_bid = STAGE_BID.load(deps.storage)?;

    // Change bid not allowed if bid phase didn't start yet.
    if !stage_bid.start.is_triggered(&_env.block) {
        return Err(ContractError::BidStageNotBegun {});
    }

    // Change bid not allowed if bid phase is ended.
    let stage_bid_end = (stage_bid.start + stage_bid.duration)?;
    if stage_bid_end.is_triggered(&_env.block) {
        return Err(ContractError::BidStageExpired {});
    }

    // It will rise an error if info.sender dosn't have an active bid.
    BIDS.load(deps.storage, &info.sender)?;

    BIDS.update(
        deps.storage,
        &info.sender,
        |allocation: Option<Uint128>| -> StdResult<_> { Ok(allocation.unwrap()) },
    )?;

    let res = Response::new()
        .add_attribute("action", "change_bid")
        .add_attribute("player", info.sender)
        .add_attribute("new_allocation", allocation);
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
    let stage_bid_end = (stage_bid.start + stage_bid.duration)?;
    if stage_bid_end.is_triggered(&_env.block) {
        return Err(ContractError::BidStageExpired {});
    }

    BIDS.remove(deps.storage, &info.sender);

    bank_transfer_to_msg(&info.sender, ticket_price, "ujuno");

    let res = Response::new()
        .add_attribute("action", "remove_bid")
        .add_attribute("player", info.sender)
        .add_attribute("ticket_price_payback", ticket_price);
    Ok(res)
}

pub fn execute_register_merkle_root(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    merkle_root: String,
    total_amount: Option<Uint128>,
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

    MERKLE_ROOT.save(deps.storage, &merkle_root)?;

    // save total airdropped amount
    let amount = total_amount.unwrap_or_else(Uint128::zero);
    TOTAL_AIRDROP_AMOUNT.save(deps.storage, &amount)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_merkle_root"),
        attr("merkle_root", merkle_root),
        attr("total_amount", amount),
    ]))
}

pub fn execute_claim_airdrop(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
    proof: Vec<String>,
) -> Result<Response, ContractError> {
    let stage_claim_aidrop = STAGE_CLAIM_AIRDROP.load(deps.storage)?;

    // throw an error if airdrop claim stage hasn't started
    if !stage_claim_aidrop.start.is_triggered(&_env.block) {
        return Err(ContractError::ClaimAirdropStageNotBegun {});
    }

    // throw an error if airdrop claim stage is expired
    let stage_claim_airdrop_end = (stage_claim_aidrop.start + stage_claim_aidrop.duration)?;
    if stage_claim_airdrop_end.is_triggered(&_env.block) {
        return Err(ContractError::ClaimAirdropStageExpired {});
    }

    // verify that user did not claimed already
    let claimed = CLAIM_AIRDROP.may_load(deps.storage, &info.sender)?;
    if claimed.is_some() {
        return Err(ContractError::AlreadyClaimed {});
    }

    let config = CONFIG.load(deps.storage)?;
    let merkle_root = MERKLE_ROOT.load(deps.storage)?;

    let user_input = format!("{}{}", info.sender, amount);
    let hash = sha2::Sha256::digest(user_input.as_bytes())
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::WrongLength {})?;

    let hash = proof.into_iter().try_fold(hash, |hash, p| {
        let mut proof_buf = [0; 32];
        hex::decode_to_slice(p, &mut proof_buf)?;
        let mut hashes = [hash, proof_buf];
        hashes.sort_unstable();
        sha2::Sha256::digest(&hashes.concat())
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::WrongLength {})
    })?;

    let mut root_buf: [u8; 32] = [0; 32];
    hex::decode_to_slice(merkle_root, &mut root_buf)?;
    if root_buf != hash {
        return Err(ContractError::VerificationFailed {});
    }

    // Update claim index
    CLAIM_AIRDROP.save(deps.storage, &info.sender, &true)?;

    // Update claimed amount to reflect
    let mut claimed_amount = CLAIMED_AIRDROP_AMOUNT.load(deps.storage)?;
    claimed_amount += amount;
    CLAIMED_AIRDROP_AMOUNT.save(deps.storage, &claimed_amount)?;

    let res = Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: config.cw20_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
        })
        .add_attribute("action", "claim_airdrop")
        .add_attribute("address", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_withdraw_airdrop(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: &Addr,
) -> Result<Response, ContractError> {
    // authorize owner
    let cfg = CONFIG.load(deps.storage)?;
    // If owner not present you can't withdraw
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    // Just the owner can withdraw
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.load(deps.storage)?;
    let stage_claim_airdrop_end = (stage_claim_airdrop.start + stage_claim_airdrop.duration)?;

    // if Stage Claim Airdrop is not over yet, can't withdraw
    if !stage_claim_airdrop_end.is_triggered(&_env.block) {
        return Err(ContractError::ClaimAirdropStageNotFinished {});
    }

    let total_amount = TOTAL_AIRDROP_AMOUNT.load(deps.storage)?;
    let claimed_amount = CLAIMED_AIRDROP_AMOUNT.load(deps.storage)?;
    let amount = (total_amount - claimed_amount);

    get_cw20_transfer_to_msg(&address, &cfg.cw20_token_address, amount)?;

    let res = Response::new()
        .add_attribute("action", "withdraw_airdrop")
        .add_attribute("address", address)
        .add_attribute("amount", amount);

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

fn bank_transfer_to_msg(recipient: &Addr, amount: Uint128, denom: &str) -> CosmosMsg {
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg
}

fn get_cw20_transfer_to_msg(
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        funds: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();
    Ok(cw20_transfer_cosmos_msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::StagesInfo {} => to_binary(&query_stages_info(deps)?),
        QueryMsg::MerkleRoot {} => todo!(),
        QueryMsg::Bid {} => todo!(),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner.map(|o| o.to_string()),
        cw20_token_address: cfg.cw20_token_address.to_string(),
    })
}

/// Returns stages's information.
pub fn query_stages_info(deps: Deps) -> StdResult<StagesInfoResponse> {
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.load(deps.storage)?;
    let stage_claim_prize = STAGE_CLAIM_PRIZE.load(deps.storage)?;
    Ok(StagesInfoResponse {
        stage_bid: stage_bid,
        stage_claim_airdrop: stage_claim_airdrop,
        stage_claim_prize: stage_claim_prize,
    })
}

/*
pub fn query_merkle_root(deps: Deps) -> StdResult<MerkleRootResponse> {
    let merkle_root = MERKLE_ROOT.load(deps.storage)?;
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.may_load(deps.storage)?;
    let stage_claim_prize = STAGE_CLAIM_PRIZE.load(deps.storage)?;

    let resp = MerkleRootResponse {merkle_root};

    Ok(resp)
}
*/

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

fn get_bank_transfer_to_msg(recipient: &Addr, denom: &str, native_amount: Uint128) -> CosmosMsg {
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: native_amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{from_binary, from_slice, CosmosMsg, SubMsg};
    use cw_utils::Duration;
    use serde::Deserialize;

    fn valid_stages() -> (Stage, Stage, Stage) {
        let stage_bid = Stage {
            start: Scheduled::AtHeight(200_000),
            duration: Duration::Height(2),
        };

        let stage_claim_airdrop = Stage {
            start: Scheduled::AtHeight(203_000),
            duration: Duration::Height(2),
        };

        let stage_claim_prize = Stage {
            start: Scheduled::AtHeight(206_000),
            duration: Duration::Height(2),
        };

        return (stage_bid, stage_claim_airdrop, stage_claim_prize);
    }
    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies();

        let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "random0000".to_string(),
            ticket_price: Uint128::new(10),
            stage_bid: stage_bid,
            stage_claim_airdrop: stage_claim_airdrop,
            stage_claim_prize: stage_claim_prize,
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // it worked, let's query the state
        let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0000", config.owner.unwrap().as_str());
        assert_eq!("random0000", config.cw20_token_address.as_str());

        let res = query(deps.as_ref(), env, QueryMsg::StagesInfo {}).unwrap();
        let stages_info: StagesInfoResponse = from_binary(&res).unwrap();
        assert_eq!(Scheduled::AtHeight(200_000), stages_info.stage_bid.start);
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies();

        let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "random0000".to_string(),
            ticket_price: Uint128::new(10),
            stage_bid: stage_bid,
            stage_claim_airdrop: stage_claim_airdrop,
            stage_claim_prize: stage_claim_prize,
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // Update owner
        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_owner: Some("owner0001".to_string()),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        // println!("{:?}", res);
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
    fn bid() {
        let mut deps = mock_dependencies();

        let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "random0000".to_string(),
            ticket_price: Uint128::new(10),
            stage_bid: stage_bid,
            stage_claim_airdrop: stage_claim_airdrop,
            stage_claim_prize: stage_claim_prize,
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // Place a valid bid without change
        let mut env = mock_env();
        env.block.height = 200_001;

        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(10),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Place a valid bid with change
        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(13),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Place unvalid bid
        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(1),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        assert_eq!(ContractError::TicketPriceNotPaid {}, res);
    }

    #[test]
    fn change_bid() {
        let mut deps = mock_dependencies();

        let (stage_bid, stage_claim_airdrop, stage_claim_prize) = valid_stages();

        let msg = InstantiateMsg {
            owner: Some("owner0000".to_string()),
            cw20_token_address: "random0000".to_string(),
            ticket_price: Uint128::new(10),
            stage_bid: stage_bid,
            stage_claim_airdrop: stage_claim_airdrop,
            stage_claim_prize: stage_claim_prize,
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        // Place a valid bid without change
        let mut env = mock_env();
        env.block.height = 200_001;

        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(10),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Place a valid bid with change
        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(13),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Place unvalid bid
        let info = mock_info(
            "owner0001",
            &[Coin {
                denom: String::from("ujuno"),
                amount: Uint128::new(1),
            }],
        );

        let msg = ExecuteMsg::Bid {
            allocation: Uint128::new(5_000_000),
        };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        assert_eq!(ContractError::TicketPriceNotPaid {}, res);
    }
}
