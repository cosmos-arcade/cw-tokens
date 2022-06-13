#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ExecuteMsg;
use sha2::Digest;
use std::convert::TryInto;

use crate::error::ContractError;
use crate::msg::{
    AmountResponse, BidResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, MerkleRootsResponse,
    MigrateMsg, QueryMsg, StagesResponse,
};
use crate::state::{
    Config, Stage, BIDS, CLAIMED_AIRDROP_AMOUNT, CLAIM_AIRDROP, CONFIG, STAGE_BID,
    STAGE_CLAIM_AIRDROP, STAGE_CLAIM_PRIZE, TICKET_PRICE, TOTAL_AIRDROP_AMOUNT, BINS, MERKLE_ROOT_AIRDROP, MERKLE_ROOT_GAME, CLAIM_PRIZE, WINNERS,
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
    // ======================================================================================
    // Contract configuration
    // ======================================================================================
    // If owner not in message, set it as sender.
    let owner = msg
        .owner
        .map_or(Ok(info.sender), |o| deps.api.addr_validate(&o))?;

    let config = Config {
        owner: Some(owner),
        cw20_token_address: deps.api.addr_validate(&msg.cw20_token_address)?,
    };

    // ======================================================================================
    // Stages validity checks
    // ======================================================================================
    let stage_bid_end = (msg.stage_bid.start + msg.stage_bid.duration)?;
    let stage_claim_airdrop_end =
        (msg.stage_claim_airdrop.start + msg.stage_claim_airdrop.duration)?;

    // Bid stage haa to start after contract instantiation.
    if msg.stage_bid.start.is_triggered(&env.block) {
        return Err(ContractError::BidStartPassed {});
    }

    // Airdrop claim stage has to start after bidding stage end.
    if stage_bid_end > msg.stage_claim_airdrop.start {
        let first = String::from("bid");
        let second = String::from("Claim airdrop");
        return Err(ContractError::StagesOverlap { first, second });
    }

    // Game prize claim has to start after airdrop claim stage end.
    if stage_claim_airdrop_end > msg.stage_claim_prize.start {
        let first = String::from("claim aidrop");
        let second = String::from("Claim prize");
        return Err(ContractError::StagesOverlap { first, second });
    }

    // ======================================================================================
    // Contract initial state
    // ======================================================================================
    // Saving contract's state after validity checks avoid useless computation.
    CONFIG.save(deps.storage, &config)?;
    STAGE_BID.save(deps.storage, &msg.stage_bid)?;
    STAGE_CLAIM_AIRDROP.save(deps.storage, &msg.stage_claim_airdrop)?;
    STAGE_CLAIM_PRIZE.save(deps.storage, &msg.stage_claim_prize)?;
    TICKET_PRICE.save(deps.storage, &msg.ticket_price)?;
    BINS.save(deps.storage, &msg.bins)?;
    WINNERS.save(deps.storage, &Uint128::new(0))?;

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
        ExecuteMsg::UpdateConfig {
            new_owner
        } => execute_update_config(deps, env, info, new_owner),
        ExecuteMsg::Bid {
            bin 
        } => execute_bid(deps, env, info, bin),
        ExecuteMsg::ChangeBid {
            bin
        } => execute_change_bid(deps, env, info, bin),
        ExecuteMsg::RemoveBid {} => execute_remove_bid(deps, env, info),
        ExecuteMsg::RegisterMerkleRoots {
            merkle_root_airdrop,
            total_amount,
            merkle_root_game
        } => execute_register_merkle_roots(deps, env, info, merkle_root_airdrop, total_amount, merkle_root_game),
        ExecuteMsg::ClaimAirdrop {
            amount,
            proof_airdrop,
            proof_game
        } => execute_claim_airdrop(deps, env, info, amount, proof_airdrop, proof_game),
        ExecuteMsg::ClaimPrize { amount, proof } => todo!(),
        ExecuteMsg::WithdrawAirdrop {
            address 
        } => execute_withdraw_airdrop(deps, env, info, &address),
        ExecuteMsg::WithdrawPrize { address } => todo!(),
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    // If owner not set the config cannot be updated.
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    // Just the owner can update the config.
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

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

pub fn check_if_valid_stage(
    env: Env,
    stage: Stage,
    stage_name: String,
) -> Result<(), ContractError> {
    // The stage has not started.
    if !stage.start.is_triggered(&env.block) {
        return Err(ContractError::StageNotStarted { stage_name });
    }

    // The stage has ended.
    let stage_end = (stage.start + stage.duration)?;
    if stage_end.is_triggered(&env.block) {
        return Err(ContractError::StageEnded { stage_name });
    }

    Ok(())
}

pub fn execute_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bin: u8,
) -> Result<Response, ContractError> {
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_name = String::from("bid");
    check_if_valid_stage(env, stage_bid, stage_name)?;

    let ticket_price = TICKET_PRICE.load(deps.storage)?;

    // If a bid is already present for the sender, no other bids can be placed.
    if BIDS.has(deps.storage, &info.sender) {
        return Err(ContractError::CannotBidMoreThanOnce {});
    };

    // If ticket price not paid, bid is not allowed.
    let fund_sent = get_amount_for_denom(&info.funds, "ujuno");
    if fund_sent.amount < ticket_price {
        return Err(ContractError::TicketPriceNotPaid {});
    }

    // If selected bin not permitted, bid not allowed.
    let bins = BINS.load(deps.storage)?;
    if bin > bins {
        return Err(ContractError::BinNotExists { bins });
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

    BIDS.save(deps.storage, &info.sender, &bin)?;

    let res = Response::new()
        .add_messages(transfer_msg)
        .add_attribute("action", "bid")
        .add_attribute("player", info.sender)
        .add_attribute("bin", bin.to_string());
    Ok(res)
}

pub fn execute_change_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bin: u8,
) -> Result<Response, ContractError> {
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_name = String::from("bid");
    check_if_valid_stage(env, stage_bid, stage_name)?;

    // If a previous bid doesn't exists for the sender, nothing can be changed.
    if !BIDS.has(deps.storage, &info.sender) {
        return Err(ContractError::BidNotPresent {});
    };

    BIDS.update(
        deps.storage,
        &info.sender,
        |_bin: Option<u8>| -> StdResult<u8> { Ok(bin) },
    )?;

    let res = Response::new()
        .add_attribute("action", "change_bid")
        .add_attribute("player", info.sender)
        .add_attribute("new_bin", bin.to_string());
    Ok(res)
}

pub fn execute_remove_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_name = String::from("bid");
    check_if_valid_stage(env, stage_bid, stage_name)?;

    let ticket_price = TICKET_PRICE.load(deps.storage)?;

    // Vector for a possible refund message.
    let mut transfer_msg: Vec<CosmosMsg> = vec![];

    // IF: check if a bid for the sender is not present.
    // ELSE: if the bid is present, remove it and send back the ticket price to the sender.
    if !BIDS.has(deps.storage, &info.sender) {
        return Err(ContractError::BidNotPresent {});
    } else {
        BIDS.remove(deps.storage, &info.sender);
        transfer_msg.push(get_bank_transfer_to_msg(
            &info.sender,
            "ujuno",
            ticket_price,
        ));
    }

    let res = Response::new()
        .add_messages(transfer_msg)
        .add_attribute("action", "remove_bid")
        .add_attribute("player", info.sender)
        .add_attribute("ticket_price_payback", ticket_price);
    Ok(res)
}

pub fn execute_register_merkle_roots(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    merkle_root_airdrop: String,
    total_amount: Option<Uint128>,
    merkle_root_game: String,
) -> Result<Response, ContractError> {
    // Just the contract owner can load the Merkle root.
    let cfg = CONFIG.load(deps.storage)?;
    let owner = cfg.owner.ok_or(ContractError::Unauthorized {})?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    // TODO: check sul periodo in cui poter depositare la merkle root. 
    // FIssiamo che è possibile solo fino alll'inizio del claim?

    // Check merkle root airdrop length.
    let mut root_buf: [u8; 32] = [0; 32];
    hex::decode_to_slice(&merkle_root_airdrop, &mut root_buf)?;

    // Check merkle root game length.
    let mut root_buf: [u8; 32] = [0; 32];
    hex::decode_to_slice(&merkle_root_game, &mut root_buf)?;

    // Save total airdropped amount.
    let amount = total_amount.unwrap_or_else(Uint128::zero);

    MERKLE_ROOT_AIRDROP.save(deps.storage, &merkle_root_airdrop)?;
    MERKLE_ROOT_GAME.save(deps.storage, &merkle_root_game)?;
    TOTAL_AIRDROP_AMOUNT.save(deps.storage, &amount)?;
    CLAIMED_AIRDROP_AMOUNT.save(deps.storage, &Uint128::zero())?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_merkle_roots"),
        attr("merkle_root_airdrop", merkle_root_airdrop),
        attr("total_amount", amount),
        attr("merkle_root_game", merkle_root_game),
    ]))
}

pub fn execute_claim_airdrop(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    proof_airdrop: Vec<String>,
    proof_game: Vec<String>
) -> Result<Response, ContractError> {
    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.load(deps.storage)?;
    let stage_name = String::from("claim airdrop");
    check_if_valid_stage(env, stage_claim_airdrop, stage_name)?;

    // Verify that the user has not already made the claim.
    let claimed = CLAIM_AIRDROP.may_load(deps.storage, &info.sender)?;
    if claimed.is_some() {
        return Err(ContractError::AlreadyClaimed {});
    }

    let config = CONFIG.load(deps.storage)?;
    let merkle_root_airdrop = MERKLE_ROOT_AIRDROP.load(deps.storage)?;
    let merkle_root_game = MERKLE_ROOT_GAME.load(deps.storage)?;

    // Compare proofs: the proof sent by the user must be the same of the one
    // produced with info.sender address.
    let user_input = format!("{}{}", info.sender, amount);
    let hash = sha2::Sha256::digest(user_input.as_bytes())
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::WrongLength {})?;

    let hash = proof_airdrop.into_iter().try_fold(hash, |hash, p| {
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
    hex::decode_to_slice(merkle_root_airdrop, &mut root_buf)?;
    if root_buf != hash {
        return Err(ContractError::VerificationFailed {});
    }

    // verify not claimed
    let sender_bid = BIDS.may_load(deps.storage, &info.sender)?;
    if sender_bid.is_some() {
        let sender_bid = sender_bid.unwrap();

        let user_input = format!("{}{}", info.sender, sender_bid);
        let hash = sha2::Sha256::digest(user_input.as_bytes())
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::WrongLength {})?;

        let hash = proof_game.into_iter().try_fold(hash, |hash, p| {
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
        hex::decode_to_slice(merkle_root_game, &mut root_buf)?;
        if root_buf == hash {
            CLAIM_PRIZE.save(deps.storage, &info.sender, &false)?;
            WINNERS.update(deps.storage, |mut winners_number| -> StdResult<_> {
                winners_number += Uint128::new(1);
                Ok(winners_number)
            })?;
        }
    }
        
    // Update claim index.
    CLAIM_AIRDROP.save(deps.storage, &info.sender, &true)?;

    // Update claimed amount to reflect
    CLAIMED_AIRDROP_AMOUNT.update(deps.storage, |mut claimed_amount| -> StdResult<_> {
        claimed_amount += amount;
        Ok(claimed_amount)
    })?;

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
    let amount = total_amount - claimed_amount;

    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    transfer_msgs.push(get_cw20_transfer_to_msg(
        &address,
        &cfg.cw20_token_address,
        amount,
    )?);

    let res = Response::new()
        .add_messages(transfer_msgs)
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

// ======================================================================================
// Queries
// ======================================================================================
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Stages {} => to_binary(&query_stages(deps)?),
        QueryMsg::Bid { address } => to_binary(&query_bid(deps, address)?),
        QueryMsg::MerkleRoot {} => to_binary(&query_merkle_root(deps)?),
        QueryMsg::AirdropClaimedAmount {} => to_binary(&query_airdrop_claimed_amount(deps)?),
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
pub fn query_stages(deps: Deps) -> StdResult<StagesResponse> {
    let stage_bid = STAGE_BID.load(deps.storage)?;
    let stage_claim_airdrop = STAGE_CLAIM_AIRDROP.load(deps.storage)?;
    let stage_claim_prize = STAGE_CLAIM_PRIZE.load(deps.storage)?;
    Ok(StagesResponse {
        stage_bid,
        stage_claim_airdrop,
        stage_claim_prize,
    })
}

pub fn query_bid(deps: Deps, address: String) -> StdResult<BidResponse> {
    let bid = BIDS.may_load(deps.storage, &deps.api.addr_validate(&address)?)?;
    Ok(BidResponse { bid })
}

pub fn query_merkle_root(deps: Deps) -> StdResult<MerkleRootsResponse> {
    let merkle_root_airdrop = MERKLE_ROOT_AIRDROP.load(deps.storage)?;
    let total_amount = TOTAL_AIRDROP_AMOUNT.load(deps.storage)?;
    let merkle_root_game = MERKLE_ROOT_GAME.load(deps.storage)?;

    let resp = MerkleRootsResponse {
        merkle_root_airdrop,
        total_amount,
        merkle_root_game
    };

    Ok(resp)
}

pub fn query_airdrop_claimed_amount(deps: Deps) -> StdResult<AmountResponse> {
    let total_claimed = CLAIMED_AIRDROP_AMOUNT.load(deps.storage)?;

    let resp = AmountResponse { total_claimed };

    Ok(resp)
}

// ======================================================================================
// Utils
// ======================================================================================
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
    use crate::state::Stage;

    use super::*;
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw_utils::{Duration, Scheduled};

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
            bins: 10,
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

        let res = query(deps.as_ref(), env, QueryMsg::Stages {}).unwrap();
        let stages_info: StagesResponse = from_binary(&res).unwrap();
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
            bins: 10,
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
}
