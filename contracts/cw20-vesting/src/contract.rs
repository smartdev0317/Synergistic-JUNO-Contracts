#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20ReceiveMsg, DownloadLogoResponse, EmbeddedLogo, Logo, LogoInfo,
    MarketingInfoResponse, TokenInfoResponse,
};

use cw_utils::ensure_from_older_version;
use syne_curve_utils::Curve;

use crate::allowances::{
    execute_burn_from, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, query_allowance,
};
use crate::enumerable::{query_all_accounts, query_all_allowances};
use crate::error::ContractError;
use crate::msg::{
    assert_schedule_vests_amount, fully_vested, DelegatedResponse, InitBalance,
    InstantiateMsg, MaxVestingComplexityResponse, MigrateMsg, MinterResponse,
    StakingAddressResponse, VestingAllowListResponse, VestingResponse,
};
use crate::state::{
    deduct_coins, MinterData, TokenInfo, ALLOWLIST, BALANCES, DELEGATED, LOGO, MARKETING_INFO,
    MAX_VESTING_COMPLEXITY, STAKING, TOKEN_INFO, VESTING,
};

use synedao::{
    cw20_vesting::{
        Cw20ReceiveDelegationMsg, ExecuteMsg, QueryMsg, 
    }
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-vesting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO_SIZE_CAP: usize = 5 * 1024;

/// Checks if data starts with XML preamble
fn verify_xml_preamble(data: &[u8]) -> Result<(), ContractError> {
    // The easiest way to perform this check would be just match on regex, however regex
    // compilation is heavy and probably not worth it.

    let preamble = data
        .split_inclusive(|c| *c == b'>')
        .next()
        .ok_or(ContractError::InvalidXmlPreamble {})?;

    const PREFIX: &[u8] = b"<?xml ";
    const POSTFIX: &[u8] = b"?>";

    if !(preamble.starts_with(PREFIX) && preamble.ends_with(POSTFIX)) {
        Err(ContractError::InvalidXmlPreamble {})
    } else {
        Ok(())
    }

    // Additionally attributes format could be validated as they are well defined, as well as
    // comments presence inside of preable, but it is probably not worth it.
}

/// Validates XML logo
fn verify_xml_logo(logo: &[u8]) -> Result<(), ContractError> {
    verify_xml_preamble(logo)?;

    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else {
        Ok(())
    }
}

/// Validates png logo
fn verify_png_logo(logo: &[u8]) -> Result<(), ContractError> {
    // PNG header format:
    // 0x89 - magic byte, out of ASCII table to fail on 7-bit systems
    // "PNG" ascii representation
    // [0x0d, 0x0a] - dos style line ending
    // 0x1a - dos control character, stop displaying rest of the file
    // 0x0a - unix style line ending
    const HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else if !logo.starts_with(&HEADER) {
        Err(ContractError::InvalidPngHeader {})
    } else {
        Ok(())
    }
}

/// Checks if passed logo is correct, and if not, returns an error
fn verify_logo(logo: &Logo) -> Result<(), ContractError> {
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => verify_xml_logo(logo),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => verify_png_logo(logo),
        Logo::Url(_) => Ok(()), // Any reasonable url validation would be regex based, probably not worth it
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;
    let cap = msg.get_cap(&env.block.time);

    // set maximum vesting complexity
    MAX_VESTING_COMPLEXITY.save(deps.storage, &msg.max_curve_complexity)?;

    // create initial accounts
    let total_supply = create_accounts(&mut deps, &env, msg.initial_balances)?;

    if let Some(limit) = cap {
        if total_supply > limit {
            return Err(StdError::generic_err("Initial supply greater than cap").into());
        }
    }

    let mint = match msg.mint {
        Some(m) => Some(MinterData {
            minter: deps.api.addr_validate(&m.minter)?,
            cap: m.cap,
        }),
        None => None,
    };

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply,
        mint,
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    if let Some(marketing) = msg.marketing {
        let logo = if let Some(logo) = marketing.logo {
            verify_logo(&logo)?;
            LOGO.save(deps.storage, &logo)?;

            match logo {
                Logo::Url(url) => Some(LogoInfo::Url(url)),
                Logo::Embedded(_) => Some(LogoInfo::Embedded),
            }
        } else {
            None
        };

        let data = MarketingInfoResponse {
            project: marketing.project,
            description: marketing.description,
            marketing: marketing
                .marketing
                .map(|addr| deps.api.addr_validate(&addr))
                .transpose()?,
            logo,
        };
        MARKETING_INFO.save(deps.storage, &data)?;
    }

    // We initially add by default info.sender to the list
    let address_list = match msg.allowed_vesters {
        Some(addrs) => addrs
            .into_iter()
            .map(|a| deps.api.addr_validate(&a))
            .collect::<StdResult<_>>()?,
        None => vec![info.sender],
    };
    ALLOWLIST.save(deps.storage, &address_list)?;

    Ok(Response::default())
}

pub fn create_accounts(
    deps: &mut DepsMut,
    env: &Env,
    accounts: Vec<InitBalance>,
) -> Result<Uint128, ContractError> {
    validate_accounts(&accounts)?;

    let mut total_supply = Uint128::zero();
    for row in accounts.into_iter() {
        // ensure vesting schedule is valid
        let vesting = match &row.vesting {
            Some(s) => {
                assert_schedule_vests_amount(s, row.amount)?;
                if fully_vested(s, &env.block) {
                    None
                } else {
                    Some(s)
                }
            }
            None => None,
        };

        let address = deps.api.addr_validate(&row.address)?;
        if let Some(vest) = vesting {
            let max_complexity = MAX_VESTING_COMPLEXITY.load(deps.storage)?;
            vest.validate_complexity(max_complexity as usize)?;
            VESTING.save(deps.storage, &address, vest)?;
        }
        BALANCES.save(deps.storage, &address, &row.amount)?;
        total_supply += row.amount;
    }

    Ok(total_supply)
}

pub fn validate_accounts(accounts: &[InitBalance]) -> Result<(), ContractError> {
    let mut addresses = accounts.iter().map(|c| &c.address).collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();

    if addresses.len() != accounts.len() {
        Err(ContractError::DuplicateInitialBalanceAddresses {})
    } else {
        Ok(())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::TransferVesting {
            recipient,
            amount,
            schedule,
        } => execute_transfer_vesting(deps, env, info, recipient, amount, schedule),
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::UpdateMinter { minter } => execute_update_minter(deps, env, info, minter),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(deps, env, info, owner, contract, amount, msg),
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => execute_upload_logo(deps, env, info, logo),
        ExecuteMsg::AllowVester { address } => execute_add_address(deps, info, address),
        ExecuteMsg::DenyVester { address } => execute_remove_address(deps, info, address),
        ExecuteMsg::UpdateStakingAddress { address } => {
            execute_update_staking_address(deps, info, address)
        }
        ExecuteMsg::Delegate { amount, msg } => execute_delegate(deps, info, amount, msg),
        ExecuteMsg::Undelegate { recipient, amount } => {
            execute_undelegate(deps, env, info, recipient, amount)
        }
    }
}

pub fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&recipient)?;

    // this will handle vesting checks as well
    deduct_coins(deps.storage, &env, &info.sender, amount)?;

    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_transfer_vesting(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
    schedule: Curve,
) -> Result<Response, ContractError> {
    // info.sender must be at least on the allow_list to allow execute trasnfer vesting
    let allow_list = ALLOWLIST.load(deps.storage)?;
    if !allow_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // ensure vesting schedule is valid
    assert_schedule_vests_amount(&schedule, amount)?;

    let rcpt_addr = deps.api.addr_validate(&recipient)?;

    // if it is not already fully vested, we store this
    if !fully_vested(&schedule, &env.block) {
        let max_complexity = MAX_VESTING_COMPLEXITY.load(deps.storage)?;
        VESTING.update(
            deps.storage,
            &rcpt_addr,
            |old| -> Result<_, ContractError> {
                let schedule = old.map(|old| old.combine(&schedule)).unwrap_or(schedule);
                // make sure the vesting curve does not get too complex, rendering the account useless
                schedule.validate_complexity(max_complexity as usize)?;
                Ok(schedule)
            },
        )?;
    }

    // this will handle vesting checks as well
    deduct_coins(deps.storage, &env, &info.sender, amount)?;

    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        // use same action as we want explorers to show this as a transfer
        .add_attribute("action", "transfer")
        .add_attribute("type", "vesting")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // lower balance
    // this will handle vesting checks as well
    deduct_coins(deps.storage, &env, &info.sender, amount)?;
    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut info| -> StdResult<_> {
        info.total_supply = info.total_supply.checked_sub(amount)?;
        Ok(info)
    })?;

    let res = Response::new()
        .add_attribute("action", "burn")
        .add_attribute("from", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // update supply and enforce cap
    config.total_supply += amount;
    if let Some(limit) = config.get_cap(&env.block.time) {
        if config.total_supply > limit {
            return Err(ContractError::CannotExceedCap {});
        }
    }
    TOKEN_INFO.save(deps.storage, &config)?;

    // add amount to recipient balance
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "mint")
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_update_minter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    minter: String,
) -> Result<Response, ContractError> {
    let mut config = TOKEN_INFO.load(deps.storage)?;
    let mint_addr = deps.api.addr_validate(&minter)?;

    match config.mint.as_mut() {
        Some(mut old) => {
            if old.minter != info.sender {
                return Err(ContractError::Unauthorized {});
            }
            old.minter = mint_addr;
        }
        None => return Err(ContractError::Unauthorized {}),
    };

    TOKEN_INFO.save(deps.storage, &config)?;

    let res = Response::new()
        .add_attribute("action", "update_minter")
        .add_attribute("minter", minter);
    Ok(res)
}

pub fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&contract)?;

    // move the tokens to the contract
    // this will handle vesting checks as well
    deduct_coins(deps.storage, &env, &info.sender, amount)?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "send")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &contract)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.into(),
                amount,
                msg,
            }
            .into_cosmos_msg(contract)?,
        );
    Ok(res)
}

pub fn execute_update_marketing(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    project: Option<String>,
    description: Option<String>,
    marketing: Option<String>,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    match project {
        Some(empty) if empty.trim().is_empty() => marketing_info.project = None,
        Some(project) => marketing_info.project = Some(project),
        None => (),
    }

    match description {
        Some(empty) if empty.trim().is_empty() => marketing_info.description = None,
        Some(description) => marketing_info.description = Some(description),
        None => (),
    }

    match marketing {
        Some(empty) if empty.trim().is_empty() => marketing_info.marketing = None,
        Some(marketing) => marketing_info.marketing = Some(deps.api.addr_validate(&marketing)?),
        None => (),
    }

    if marketing_info.project.is_none()
        && marketing_info.description.is_none()
        && marketing_info.marketing.is_none()
        && marketing_info.logo.is_none()
    {
        MARKETING_INFO.remove(deps.storage);
    } else {
        MARKETING_INFO.save(deps.storage, &marketing_info)?;
    }

    let res = Response::new().add_attribute("action", "update_marketing");
    Ok(res)
}

pub fn execute_upload_logo(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    logo: Logo,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    verify_logo(&logo)?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    LOGO.save(deps.storage, &logo)?;

    let logo_info = match logo {
        Logo::Url(url) => LogoInfo::Url(url),
        Logo::Embedded(_) => LogoInfo::Embedded,
    };

    marketing_info.logo = Some(logo_info);
    MARKETING_INFO.save(deps.storage, &marketing_info)?;

    let res = Response::new().add_attribute("action", "upload_logo");
    Ok(res)
}

pub fn execute_add_address(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    // info.sender must be at least on the allow_list to add address to the list
    let mut allow_list = ALLOWLIST.load(deps.storage)?;
    if !allow_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // validate address and ensure unique
    let addr = deps.api.addr_validate(&address)?;
    if allow_list.contains(&addr) {
        return Err(ContractError::AddressAlreadyExist {});
    }

    // Add the new address to the allow list
    allow_list.push(addr);
    ALLOWLIST.save(deps.storage, &allow_list)?;

    let res = Response::new().add_attribute("action", "add address");
    Ok(res)
}

pub fn execute_remove_address(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    // info.sender must be at least on the allow_list to remove address to the list
    let allow_list = ALLOWLIST.load(deps.storage)?;
    if !allow_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // validate address and remove
    let addr = deps.api.addr_validate(&address)?;
    let prev_len = allow_list.len();
    let allow_list: Vec<Addr> = allow_list
        .into_iter()
        .filter(|item| *item != addr)
        .collect();

    // ensure it was found and left something
    if prev_len == allow_list.len() {
        return Err(ContractError::AddressNotFound {});
    }
    if allow_list.is_empty() {
        return Err(ContractError::AtLeastOneAddressMustExist {});
    }

    ALLOWLIST.save(deps.storage, &allow_list)?;
    let res = Response::new().add_attribute("action", "remove address");
    Ok(res)
}

pub fn execute_update_staking_address(
    deps: DepsMut,
    info: MessageInfo,
    staking: String,
) -> Result<Response, ContractError> {
    // Staking address can be updated only once
    // If load is failing, it means it wasn't set before
    match STAKING.load(deps.storage) {
        Ok(_) => Err(ContractError::StakingAddressAlreadyUpdated {}),
        Err(_) => {
            if let Some(mint) = TOKEN_INFO.load(deps.storage)?.mint {
                if info.sender == mint.minter {
                    let staking_address = deps.api.addr_validate(&staking)?;
                    STAKING.save(deps.storage, &staking_address)?;
                    Ok(Response::new().add_attribute("update staking address", staking))
                } else {
                    Err(ContractError::UnauthorizedUpdateStakingAddress {})
                }
            } else {
                Err(ContractError::MinterAddressNotSet {})
            }
        }
    }
}

pub fn execute_delegate(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let token_address = match STAKING.load(deps.storage) {
        Ok(address) => address,
        Err(_) => return Err(ContractError::StakingAddressNotSet {}),
    };

    // this allows to delegate also vested tokens, because vested is included in balance anyway
    BALANCES.update(deps.storage, &info.sender, |balance| {
        let balance = balance.unwrap_or_default();
        balance
            .checked_sub(amount)
            .map_err(|_| ContractError::NotEnoughToDelegate)
    })?;
    // make sure we add it to the other side
    BALANCES.update(deps.storage, &token_address, |balance| -> StdResult<_> {
        let balance = balance.unwrap_or_default() + amount;
        Ok(balance)
    })?;

    DELEGATED.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "delegate")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &token_address)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveDelegationMsg {
                sender: info.sender.into(),
                amount,
                msg,
            }
            .into_cosmos_msg(token_address)?,
        );
    Ok(res)
}

pub fn execute_undelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    match STAKING.load(deps.storage) {
        Ok(staking) => {
            if staking != info.sender {
                return Err(ContractError::UnauthorizedUndelegate {});
            }
        }
        Err(_) => return Err(ContractError::StakingAddressNotSet {}),
    };

    let recipient_address = deps.api.addr_validate(&recipient)?;

    if !DELEGATED.has(deps.storage, &recipient_address) {
        return Err(ContractError::NoTokensDelegated {});
    }
    DELEGATED.update(
        deps.storage,
        &recipient_address,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    deduct_coins(deps.storage, &env, &info.sender, amount)?;
    BALANCES.update(
        deps.storage,
        &recipient_address,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "undelegate")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &recipient_address)
        .add_attribute("amount", amount);
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Vesting { address } => to_binary(&query_vesting(deps, env, address)?),
        QueryMsg::Delegated { address } => to_binary(&query_delegated(deps, address)?),
        QueryMsg::VestingAllowList {} => to_binary(&query_allow_list(deps)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::MaxVestingComplexity {} => to_binary(&query_max_complexity(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps, env)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
        QueryMsg::StakingAddress {} => to_binary(&query_staking_address(deps)?),
    }
}

pub fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
}

pub fn query_vesting(deps: Deps, env: Env, address: String) -> StdResult<VestingResponse> {
    let address = deps.api.addr_validate(&address)?;
    let schedule = VESTING.may_load(deps.storage, &address)?;
    let time = env.block.time.seconds();
    let locked = schedule.as_ref().map(|c| c.value(time)).unwrap_or_default();
    Ok(VestingResponse { schedule, locked })
}

pub fn query_delegated(deps: Deps, address: String) -> StdResult<DelegatedResponse> {
    let address = deps.api.addr_validate(&address)?;
    let delegated = DELEGATED
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(DelegatedResponse { delegated })
}

pub fn query_token_info(deps: Deps) -> StdResult<TokenInfoResponse> {
    let info = TOKEN_INFO.load(deps.storage)?;
    let res = TokenInfoResponse {
        name: info.name,
        symbol: info.symbol,
        decimals: info.decimals,
        total_supply: info.total_supply,
    };
    Ok(res)
}

pub fn query_max_complexity(deps: Deps) -> StdResult<MaxVestingComplexityResponse> {
    let complexity = MAX_VESTING_COMPLEXITY.load(deps.storage)?;
    Ok(MaxVestingComplexityResponse { complexity })
}

pub fn query_minter(deps: Deps, env: Env) -> StdResult<Option<MinterResponse>> {
    let meta = TOKEN_INFO.load(deps.storage)?;
    let minter = match meta.mint {
        Some(m) => {
            let current_cap = m.cap.as_ref().map(|v| v.value(env.block.time.seconds()));
            Some(MinterResponse {
                minter: m.minter.into(),
                cap: m.cap,
                current_cap,
            })
        }
        None => None,
    };
    Ok(minter)
}

pub fn query_marketing_info(deps: Deps) -> StdResult<MarketingInfoResponse> {
    Ok(MARKETING_INFO.may_load(deps.storage)?.unwrap_or_default())
}

pub fn query_allow_list(deps: Deps) -> StdResult<VestingAllowListResponse> {
    let allow_list = ALLOWLIST
        .load(deps.storage)?
        .into_iter()
        .map(|a| a.into())
        .collect();
    Ok(VestingAllowListResponse { allow_list })
}

pub fn query_download_logo(deps: Deps) -> StdResult<DownloadLogoResponse> {
    let logo = LOGO.load(deps.storage)?;
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/svg+xml".to_owned(),
            data: logo,
        }),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/png".to_owned(),
            data: logo,
        }),
        Logo::Url(_) => Err(StdError::not_found("logo")),
    }
}

pub fn query_staking_address(deps: Deps) -> StdResult<StakingAddressResponse> {
    let address = STAKING.may_load(deps.storage)?;
    Ok(StakingAddressResponse { address })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // make sure picewise linear curve is passed in the message
    match msg.picewise_linear_curve {
        Curve::PiecewiseLinear(_) => (),
        _ => {
            return Err(ContractError::MigrationIncorrectCurve {});
        }
    };

    TOKEN_INFO.update(deps.storage, |mut token_info| -> StdResult<_> {
        // We can unwrap because we know cap is set
        token_info.mint.as_mut().unwrap().cap = Some(msg.picewise_linear_curve);
        Ok(token_info)
    })?;

    Ok(Response::new())
}
