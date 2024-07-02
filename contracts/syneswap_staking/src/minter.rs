#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20Coin, EmbeddedLogo, Logo, LogoInfo, MarketingInfoResponse, MinterResponse,
    TokenInfoResponse,
};

use crate::msg::{Cw20QueryMsg, TokenInstantiateMsg};
use cw20_base::enumerable::query_all_accounts;

use crate::state::{
    BalanceInfo, Config, MinterData, TokenInfo, UserInfo, BALANCES, CONFIG, LOGO, MARKETING_INFO,
    MINT_TIME, TOKEN_INFO, TOTAL_BALANCES,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO_SIZE_CAP: usize = 5 * 1024;
const REWARD_CALC_UNIT: Uint128 = Uint128::new(1000000000000u128);
/// Checks if data starts with XML preamble
fn verify_xml_preamble(data: &[u8]) -> StdResult<()> {
    // The easiest way to perform this check would be just match on regex, however regex
    // compilation is heavy and probably not worth it.

    let preamble = data
        .split_inclusive(|c| *c == b'>')
        .next()
        .ok_or(StdError::generic_err("Invalid xml preamble for SVG"))?;

    const PREFIX: &[u8] = b"<?xml ";
    const POSTFIX: &[u8] = b"?>";

    if !(preamble.starts_with(PREFIX) && preamble.ends_with(POSTFIX)) {
        Err(StdError::generic_err("Invalid xml preamble for SVG"))
    } else {
        Ok(())
    }

    // Additionally attributes format could be validated as they are well defined, as well as
    // comments presence inside of preable, but it is probably not worth it.
}

/// Validates XML logo
fn verify_xml_logo(logo: &[u8]) -> StdResult<()> {
    verify_xml_preamble(logo)?;

    if logo.len() > LOGO_SIZE_CAP {
        Err(StdError::generic_err("Logo binary data exceeds 5KB limit"))
    } else {
        Ok(())
    }
}

/// Validates png logo
fn verify_png_logo(logo: &[u8]) -> StdResult<()> {
    // PNG header format:
    // 0x89 - magic byte, out of ASCII table to fail on 7-bit systems
    // "PNG" ascii representation
    // [0x0d, 0x0a] - dos style line ending
    // 0x1a - dos control character, stop displaying rest of the file
    // 0x0a - unix style line ending
    const HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if logo.len() > LOGO_SIZE_CAP {
        Err(StdError::generic_err("Logo binary data exceeds 5KB limit"))
    } else if !logo.starts_with(&HEADER) {
        Err(StdError::generic_err("Invalid png header"))
    } else {
        Ok(())
    }
}

/// Checks if passed logo is correct, and if not, returns an error
fn verify_logo(logo: &Logo) -> StdResult<()> {
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => verify_xml_logo(logo),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => verify_png_logo(logo),
        Logo::Url(_) => Ok(()), // Any reasonable url validation would be regex based, probably not worth it
    }
}

pub fn instantiate_token(
    deps: DepsMut,
    _env: &Env,
    _info: &MessageInfo,
    msg: TokenInstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;
    // create initial accounts

    validate_accounts(&msg.initial_balances)?;

    let mut total_supply = Uint128::zero();
    for row in &msg.initial_balances {
        let address = deps.api.addr_validate(&row.address)?;
        BALANCES.save(deps.storage, &address, &row.amount)?;
        total_supply += row.amount;
    }

    if let Some(limit) = msg.get_cap() {
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

    Ok(Response::default())
}
// Update Token Info: token name and token symbol
pub fn update_token_info(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    name: String,
    symbol: String,
) -> StdResult<Response> {
    // permission check
    let config: Config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    let mut token_info = TOKEN_INFO.load(deps.storage)?;

    token_info.name = name;
    token_info.symbol = symbol;

    TOKEN_INFO.save(deps.storage, &token_info)?;
    Ok(Response::new().add_attribute("action", "update_token_info"))
}

pub fn validate_accounts(accounts: &[Cw20Coin]) -> StdResult<()> {
    let mut addresses = accounts.iter().map(|c| &c.address).collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();

    if addresses.len() != accounts.len() {
        Err(StdError::generic_err("Duplicate initial balance addresses"))
    } else {
        Ok(())
    }
}

pub fn execute_mint(
    deps: DepsMut,
    _env: Env,
    //info: MessageInfo,
    recipient: String,
    amount: Uint128,
    duration: u64,
) -> StdResult<Response> {
    let staking_config = CONFIG.load(deps.storage)?;
    if amount == Uint128::zero() {
        return Err(StdError::generic_err("Invalid zero amount"));
    }

    let mut config = TOKEN_INFO.load(deps.storage)?;
    let amount = Uint128::from(amount.u128() * duration as u128);
    // update supply and enforce cap
    config.total_supply += amount;
    if let Some(limit) = config.get_cap() {
        if config.total_supply > limit {
            return Err(StdError::generic_err("Minting cannot exceed the cap"));
        }
    }
    TOKEN_INFO.save(deps.storage, &config)?;

    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    // total balances of all minting
    let mut total_balance: Uint128 = Uint128::from(0u128);
    let prev_total_balances = TOTAL_BALANCES
        .may_load(deps.storage, duration)?
        .unwrap_or_default();

    let mut prev_balances: Uint128 = Uint128::from(0u128);
    let user_info = MINT_TIME
        .may_load(deps.storage, (&rcpt_addr, duration))?
        .unwrap_or_default();
    println!("user_info.balance: {} ", user_info.balance);
    let prev_amount = user_info.actual_balance;
    // adding and calculating user total balance of one duration
    if user_info.mint_time > 0u64 {
        let month_seconds = staking_config.lock_time_frame * duration;
        let time_diff = _env.block.time.seconds() - user_info.mint_time;
        if time_diff < month_seconds {
            let power_time = month_seconds - time_diff;
            let power_time = REWARD_CALC_UNIT.multiply_ratio(power_time, 1u64);
            let power_time = power_time.multiply_ratio(1u64, month_seconds);
            if power_time > Uint128::from(0u128) {
                prev_balances = user_info.balance * power_time;
                prev_balances = prev_balances.multiply_ratio(1u64, REWARD_CALC_UNIT);
            } else {
                prev_balances = Uint128::from(0u128);
            }
        }
    }
    // println!()
    //adding and calculating total market cap balance of one duration
    if prev_total_balances.balance > Uint128::from(0u128) {
        let month_seconds = staking_config.lock_time_frame * duration;
        let time_diff = _env.block.time.seconds() - prev_total_balances.mint_time;
        if time_diff < month_seconds {
            let power_time = month_seconds - time_diff;
            let power_time = REWARD_CALC_UNIT.multiply_ratio(power_time, 1u64);
            let power_time = power_time.multiply_ratio(1u64, month_seconds);

            if power_time > Uint128::from(0u128) {
                total_balance = prev_total_balances.balance * power_time;
                total_balance = total_balance.multiply_ratio(1u64, REWARD_CALC_UNIT);
                if prev_balances > Uint128::from(0u128) {
                    println!(
                        "prev_balances {}, prev_balances {}",
                        total_balance, prev_balances
                    );
                    total_balance -= prev_balances;
                    println!(
                        "after total {}, prev_balances {}",
                        total_balance, prev_balances
                    );
                    total_balance += prev_amount;
                    println!(
                        "after total {}, prev_balances {}",
                        total_balance, prev_balances
                    );
                }
            } else {
                total_balance = Uint128::from(0u128);
            }
        }
    }
    println!("total_balance {}", total_balance);

    let balance_info = BalanceInfo {
        balance: total_balance + amount,
        mint_time: _env.block.time.seconds(),
    };

    TOTAL_BALANCES.save(deps.storage, duration, &balance_info)?;

    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |_balance: Option<Uint128>| -> StdResult<_> { Ok(prev_balances + amount) },
    )?;

    println!(
        "Mint_balances after: {:?}, prev_amount {}",
        prev_balances, prev_amount
    );
    let user_info = UserInfo {
        balance: amount + prev_amount,
        actual_balance: amount + prev_amount,
        last_claimed_time: _env.block.time.seconds(),
        mint_time: _env.block.time.seconds(),
    };
    MINT_TIME.save(deps.storage, (&rcpt_addr, duration), &user_info)?;
    let res = Response::new()
        .add_attribute("action", "mint")
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn query(deps: Deps, _env: Env, msg: Cw20QueryMsg) -> StdResult<Binary> {
    match msg {
        Cw20QueryMsg::Balance { address } => to_binary(&query_balance(deps, _env, address)?),

        Cw20QueryMsg::BalanceByDuration { address, duration } => {
            to_binary(&query_balance_by_duration(deps, _env, address, duration)?)
        }

        Cw20QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        Cw20QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        Cw20QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        Cw20QueryMsg::TotalBalance { duration } => to_binary(&query_total_balance(
            deps,
            _env.block.time.seconds(),
            duration,
        )?),
    }
}
//Query Balance of all durations for user
pub fn query_balance(deps: Deps, env: Env, address: String) -> StdResult<BalanceResponse> {
    let staking_config = CONFIG.load(deps.storage)?;
    let address = deps.api.addr_validate(&address)?;
    let duration_values = staking_config.duration_values_vector;
    // let mut total_balances = TOTAL_BALANCES
    //     .may_load(deps.storage, duration)?
    //     .unwrap_or_default();

    let mut user_total_balance = Uint128::from(0u128);
    // let balance = total_balances.balance;
    // println!("total_balance:{:?}", balance.clone());

    let mut balance: Uint128 = Uint128::from(0u128);

    for duration in duration_values {
        let user_info =
            if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
                user_info
            } else {
                UserInfo {
                    mint_time: 0u64,
                    last_claimed_time: 0u64,
                    balance: Uint128::from(0u128),
                    actual_balance: Uint128::zero(),
                }
            };
        if user_info.mint_time > 0u64 {
            let month_seconds: u64 = staking_config.lock_time_frame * duration;
            let time_diff: u64 = env.block.time.seconds() - user_info.mint_time;
            if time_diff < month_seconds {
                let power_time = month_seconds - time_diff;
                let power_time = REWARD_CALC_UNIT.multiply_ratio(power_time, 1u64);
                let power_time = power_time.multiply_ratio(1u64, month_seconds);
                if power_time > Uint128::from(0u128) {
                    balance = user_info.balance * power_time;
                    balance = balance.multiply_ratio(1u64, REWARD_CALC_UNIT);
                } else {
                    balance = Uint128::from(0u128);
                }
            }
            user_total_balance += balance;
        }
    }
    balance = user_total_balance;
    Ok(BalanceResponse { balance })
}

pub fn query_balance_by_duration(
    deps: Deps,
    env: Env,
    address: String,
    duration: u64,
) -> StdResult<Uint128> {
    let staking_config = CONFIG.load(deps.storage)?;
    let address = deps.api.addr_validate(&address)?;

    // let mut total_balances = TOTAL_BALANCES
    //     .may_load(deps.storage, duration)?
    //     .unwrap_or_default();

    // let balance = total_balances.balance;
    // println!("total_balance:{:?}", balance.clone());

    let mut balance: Uint128 = Uint128::from(0u128);
    let user_info =
        if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
            user_info
        } else {
            UserInfo {
                mint_time: 0u64,
                last_claimed_time: 0u64,
                balance: Uint128::from(0u128),
                actual_balance: Uint128::zero(),
            }
        };
    if user_info.mint_time > 0u64 {
        let month_seconds: u64 = staking_config.lock_time_frame * duration;
        let time_diff: u64 = env.block.time.seconds() - user_info.mint_time;
        if time_diff < month_seconds {
            let power_time = month_seconds - time_diff;
            let power_time = REWARD_CALC_UNIT.multiply_ratio(power_time, 1u64);
            let power_time = power_time.multiply_ratio(1u64, month_seconds);
            if power_time > Uint128::from(0u128) {
                balance = user_info.balance * power_time;
                balance = balance.multiply_ratio(1u64, REWARD_CALC_UNIT);
            } else {
                balance = Uint128::from(0u128);
            }
        }
    }
    Ok(balance)
}

pub fn query_total_balance(deps: Deps, mut time: u64, duration: u64) -> StdResult<BalanceResponse> {
    let staking_config = CONFIG.load(deps.storage)?;
    let total_balances =
        if let Some(total_balances) = TOTAL_BALANCES.may_load(deps.storage, duration)? {
            total_balances
        } else {
            BalanceInfo {
                mint_time: 0u64,
                balance: Uint128::from(0u128),
            }
        };
    let mut total_balance: Uint128 = Uint128::from(0u128);
    if total_balances.mint_time > 0u64 {
        let month_seconds: u64 = staking_config.lock_time_frame * duration;
        if time < total_balances.mint_time {
            time = total_balances.mint_time;
        }
        let time_diff: u64 = time - total_balances.mint_time;
        if time_diff < month_seconds {
            let power_time = month_seconds - time_diff;
            let power_time = REWARD_CALC_UNIT.multiply_ratio(power_time, 1u64);
            let power_time = power_time.multiply_ratio(1u64, month_seconds);
            if power_time > Uint128::from(0u128) {
                total_balance = total_balances.balance * power_time;
                total_balance = total_balance.multiply_ratio(1u64, REWARD_CALC_UNIT);
            } else {
                total_balance = Uint128::from(0u128);
            }
        }
    }

    let balance = total_balance;
    println!("total_balanceQuery:{:?}", total_balance.clone());
    Ok(BalanceResponse { balance })
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

pub fn query_minter(deps: Deps) -> StdResult<Option<MinterResponse>> {
    let meta = TOKEN_INFO.load(deps.storage)?;
    let minter = match meta.mint {
        Some(m) => Some(MinterResponse {
            minter: m.minter.into(),
            cap: m.cap,
        }),
        None => None,
    };
    Ok(minter)
}

pub fn query_marketing_info(deps: Deps) -> StdResult<MarketingInfoResponse> {
    Ok(MARKETING_INFO.may_load(deps.storage)?.unwrap_or_default())
}