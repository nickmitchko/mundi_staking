use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("5tGMK2Hx4CgqkjWbT5sJFyZw39VX6zzU5ajscu8acoRn");

const UNLOCK_MARKET_CAP: u64 = 450_000_000; // $450M in USD

// $ spl-token create-token
// Creating token FeEucpirJWULUQwgwoiDJxzwoN7tMWFLTXEaupgSt8F3

// Address:  FeEucpirJWULUQwgwoiDJxzwoN7tMWFLTXEaupgSt8F3
// Decimals:  9

// Signature: 52dnNwh3JnV7WSpKGCB5pAFowEfL5MY7hJp8FY56HDGsU2hWXSdDEotgZhvy3urkyXRyZFqDDNj79SGxhXTEfEjs

#[program]
pub mod token_staking {
    use super::*;

    pub fn initialize_stake(ctx: Context<InitializeStake>, lock_duration: i64) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        stake_account.owner = ctx.accounts.owner.key();
        stake_account.token_account = ctx.accounts.stake_token_account.key();
        stake_account.lock_duration = lock_duration;
        stake_account.locked_at = Clock::get()?.unix_timestamp;
        stake_account.unlocked = false;
        stake_account.staked_amount = 0; // Initialize staked amount
        Ok(())
    }

    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;

        // Verify the stake token account matches the one in stake_account
        require!(
            ctx.accounts.stake_token_account.key() == stake_account.token_account,
            StakingError::InvalidTokenAccount
        );

        // Transfer tokens to the stake account
        let cpi_accounts = Transfer {
            from: ctx.accounts.from_token_account.to_account_info(),
            to: ctx.accounts.stake_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        stake_account.staked_amount = stake_account
            .staked_amount
            .checked_add(amount)
            .ok_or(StakingError::NumericOverflow)?;
        Ok(())
    }

    pub fn check_unlock_conditions(ctx: Context<CheckUnlock>) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        let oracle_account = &ctx.accounts.oracle_account;
        let token_mint = ctx.accounts.token_mint.to_account_info();

        // Safely deserialize the mint account
        let mint = Mint::try_deserialize(&mut &token_mint.data.borrow()[..])?;

        // Calculate market cap with overflow protection
        let market_cap = (oracle_account.price as u128)
            .checked_mul(mint.supply as u128)
            .ok_or(StakingError::NumericOverflow)?;
        let market_cap = market_cap
            .checked_div(1_000_000_000)
            .ok_or(StakingError::NumericOverflow)? as u64;

        // Check if market cap has reached unlock threshold
        if market_cap >= UNLOCK_MARKET_CAP {
            stake_account.unlocked = true;
        }

        Ok(())
    }

    pub fn unstake_tokens(ctx: Context<UnstakeTokens>) -> Result<()> {
        let stake_account = &ctx.accounts.stake_account;
        let current_time = Clock::get()?.unix_timestamp;

        // Check both time lock and market cap conditions
        require!(
            current_time >= stake_account.locked_at + stake_account.lock_duration,
            StakingError::TokensStillLocked
        );

        require!(stake_account.unlocked, StakingError::MarketCapNotReached);

        // Verify the stake token account
        require!(
            ctx.accounts.stake_token_account.key() == stake_account.token_account,
            StakingError::InvalidTokenAccount
        );

        // Calculate fee with overflow protection
        let fee_amount = stake_account
            .staked_amount
            .checked_mul(5)
            .ok_or(StakingError::NumericOverflow)?
            .checked_div(10000)
            .ok_or(StakingError::NumericOverflow)?;

        // Get stake account PDA signer
        let (pda, bump) = Pubkey::find_program_address(
            &[b"stak", ctx.accounts.owner.key().as_ref()],
            ctx.program_id,
        );

        // let seeds = &[b"stake", ctx.accounts.owner.key().as_ref(), &[bump]];

        // Transfer the fee
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stake_token_account.to_account_info(),
                    to: ctx.accounts.fee_collector.to_account_info(),
                    authority: ctx.accounts.stake_account.to_account_info(),
                },
                &[
                    &[b"stak".as_slice()],
                    &[ctx.accounts.owner.key().as_ref()],
                    &[&[bump]],
                ],
            ),
            fee_amount,
        )?;

        // Transfer remaining tokens back to owner
        let remaining_amount = stake_account
            .staked_amount
            .checked_sub(fee_amount)
            .ok_or(StakingError::NumericOverflow)?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stake_token_account.to_account_info(),
                    to: ctx.accounts.to_token_account.to_account_info(),
                    authority: ctx.accounts.stake_account.to_account_info(),
                },
                &[
                    &[b"stak".as_slice()],
                    &[ctx.accounts.owner.key().as_ref()],
                    &[&[bump]],
                ],
            ),
            remaining_amount,
        )?;

        Ok(())
    }

    pub fn update_price(ctx: Context<UpdatePrice>, price: u64) -> Result<()> {
        let oracle_account = &mut ctx.accounts.oracle_account;
        oracle_account.price = price;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeStake<'info> {
    // #[account(mut)]
    // pub owner: Signer<'info>,

    // #[account(
    //     init,
    //     payer = owner,
    //     space = 8 + StakeAccount::LEN,
    //     seeds = [b"stak", owner.key().as_ref()],
    //     bump
    // )]
    // pub stake_account: Account<'info, StakeAccount>,

    // #[account(
    //     constraint = stake_token_account.owner == owner.key()
    // )]
    // pub stake_token_account: Account<'info, TokenAccount>,

    // pub token_program: Program<'info, Token>,
    // pub system_program: Program<'info, System>,
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + StakeAccount::LEN,
        seeds = [b"stake", owner.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    pub mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = stake_account
    )]
    pub stake_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckUnlock<'info> {
    #[account(mut)]
    pub stake_account: Account<'info, StakeAccount>,
    pub oracle_account: Account<'info, CustomOracleAccount>,
    pub token_mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"stak", owner.key().as_ref()],
        bump,
        constraint = stake_account.owner == owner.key()
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        constraint = from_token_account.owner == owner.key()
    )]
    pub from_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = stake_token_account.owner == crate::ID
    )]
    pub stake_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"stak", owner.key().as_ref()],
        bump,
        constraint = stake_account.owner == owner.key()
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        constraint = stake_token_account.owner == token_program.key()
    )]
    pub stake_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = to_token_account.owner == token_program.key()
    )]
    pub to_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = fee_collector.owner == token_program.key()
    )]
    pub fee_collector: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct CustomOracleAccount {
    pub price: u64,
}

#[account]
pub struct StakeAccount {
    pub owner: Pubkey,
    pub token_account: Pubkey,
    pub staked_amount: u64,
    pub locked_at: i64,
    pub lock_duration: i64,
    pub unlocked: bool,
}

#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    #[account(mut)]
    pub oracle_account: Account<'info, CustomOracleAccount>,
    pub authority: Signer<'info>,
}

impl StakeAccount {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 1;
}

#[error_code]
pub enum StakingError {
    #[msg("Tokens are still locked")]
    TokensStillLocked,
    #[msg("Market cap has not reached unlock threshold $450M")]
    MarketCapNotReached,
    #[msg("You are not authorized to update the marketcap")]
    Unauthorized,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Numeric overflow occurred")]
    NumericOverflow,
}

// use anchor_lang::prelude::Pubkey;
// use anchor_lang::prelude::*;
// use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
// // use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

// declare_id!("5tGMK2Hx4CgqkjWbT5sJFyZw39VX6zzU5ajscu8acoRn");

// const UNLOCK_MARKET_CAP: u64 = 450_000_000; // $450M in USD

// #[program]
// pub mod token_staking {
//     use std::str::FromStr;
//     // use std::borrow::Borrow;

//     // use pyth_sdk_solana::state::SolanaPriceAccount;
//     // use solana_account_info::AccountInfo;

//     use super::*;

//     pub fn initialize_stake(ctx: Context<InitializeStake>, lock_duration: i64) -> Result<()> {
//         let stake_account = &mut ctx.accounts.stake_account;
//         stake_account.owner = ctx.accounts.owner.key();
//         stake_account.token_account = ctx.accounts.stake_token_account.key();
//         stake_account.lock_duration = lock_duration;
//         stake_account.locked_at = Clock::get()?.unix_timestamp;
//         stake_account.unlocked = false;
//         Ok(())
//     }

//     pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
//         let stake_account = &mut ctx.accounts.stake_account;

//         // Transfer tokens to the stake account
//         let cpi_accounts = Transfer {
//             from: ctx.accounts.from_token_account.to_account_info(),
//             to: ctx.accounts.stake_token_account.to_account_info(),
//             authority: ctx.accounts.owner.to_account_info(),
//         };

//         let cpi_program = ctx.accounts.token_program.to_account_info();
//         let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
//         token::transfer(cpi_ctx, amount)?;

//         stake_account.staked_amount = amount;
//         Ok(())
//     }

//     pub fn check_unlock_conditions(ctx: Context<CheckUnlock>) -> Result<()> {
//         let stake_account = &mut ctx.accounts.stake_account;
//         let oracle_account = &ctx.accounts.oracle_account;

//         // Get the price from the custom oracle account
//         let current_price = oracle_account.price;

//         // Get total supply from token mint
//         // let supply = ctx.accounts.token_mint.supply;
//         let token_mint = &ctx.accounts.token_mint;
//         let mint = Mint::try_deserialize_unchecked(&mut token_mint.data.borrow().as_ref())
//             .map_err(|_| anchor_lang::error::ErrorCode::ConstraintAssociatedTokenTokenProgram)?;

//         let supply = mint.supply;

//         // Calculate market cap (price * supply)
//         let market_cap = (current_price as u128 * supply as u128 / 1_000_000_000) as u64; // Convert to millions

//         // Check if market cap has reached unlock threshold
//         if market_cap >= UNLOCK_MARKET_CAP {
//             stake_account.unlocked = true;
//         }

//         Ok(())
//     }

//     pub fn unstake_tokens(ctx: Context<UnstakeTokens>) -> Result<()> {
//         let stake_account = &ctx.accounts.stake_account;
//         let current_time = Clock::get()?.unix_timestamp;

//         // Check both time lock and market cap conditions
//         require!(
//             current_time >= stake_account.locked_at + stake_account.lock_duration,
//             StakingError::TokensStillLocked
//         );

//         require!(stake_account.unlocked, StakingError::MarketCapNotReached);

//         // Fee of .05% for the staking currency
//         let fee_amount = stake_account.staked_amount * 5 / 100 / 100;

//         // Transfer the fee to the fee collector account
//         let cpi_accounts = Transfer {
//             from: ctx.accounts.stake_token_account.to_account_info(),
//             to: ctx.accounts.fee_collector.to_account_info(),
//             authority: ctx.accounts.stake_account.to_account_info(),
//         };

//         let cpi_program = ctx.accounts.token_program.to_account_info();
//         let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
//         token::transfer(cpi_ctx, fee_amount)?;

//         // Transfer the remaining tokens back to the owner
//         let cpi_accounts = Transfer {
//             from: ctx.accounts.stake_token_account.to_account_info(),
//             to: ctx.accounts.to_token_account.to_account_info(),
//             authority: ctx.accounts.stake_account.to_account_info(),
//         };

//         let cpi_program = ctx.accounts.token_program.to_account_info();
//         let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
//         token::transfer(cpi_ctx, stake_account.staked_amount - fee_amount)?;

//         Ok(())
//     }

//     pub fn update_price(ctx: Context<UpdatePrice>, price: u64) -> Result<()> {
//         let oracle_account = &mut ctx.accounts.oracle_account;
//         let authority = &ctx.accounts.authority;

//         oracle_account.price = price;

//         Ok(())
//     }
// }

// #[derive(Accounts)]
// pub struct InitializeStake<'info> {
//     #[account(mut)]
//     pub owner: Signer<'info>,

//     #[account(
//         init,
//         payer = owner,
//         space = 8 + StakeAccount::LEN,
//         seeds = [b"stake", owner.key().as_ref()],
//         bump
//     )]
//     pub stake_account: Account<'info, StakeAccount>,

//     // #[account(address = stake_token_account.key)]
//     /// CHECK: This is not dangerous because we programmatically define this
//     pub stake_token_account: AccountInfo<'info>,
//     pub system_program: Program<'info, System>,
// }

// // Replace the Account type with the account_info type
// #[derive(Accounts)]
// pub struct CheckUnlock<'info> {
//     #[account(mut)]
//     pub stake_account: Account<'info, StakeAccount>,
//     /// CHECK: Pyth price account
//     pub oracle_account: Account<'info, CustomOracleAccount>, // Custom oracle account
//     /// CHECK: This is not dangerous because it's static
//     pub token_mint: AccountInfo<'info>,
// }

// // Replace the Account type with the account_info type
// #[derive(Accounts)]
// pub struct StakeTokens<'info> {
//     #[account(mut)]
//     pub owner: Signer<'info>,

//     #[account(
//         mut,
//         seeds = [b"stake", owner.key().as_ref()],
//         bump,
//         constraint = stake_account.owner == owner.key()
//     )]
//     pub stake_account: Account<'info, StakeAccount>,

//     /// CHECK: This is not dangerous because we don't read or write from this account
//     #[account(mut)]
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     pub from_token_account: AccountInfo<'info>,
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     #[account(mut)]
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     pub stake_token_account: AccountInfo<'info>,
//     pub token_program: Program<'info, Token>,
// }

// // Replace the Account type with the account_info type
// #[derive(Accounts)]
// pub struct UnstakeTokens<'info> {
//     #[account(mut)]
//     pub owner: Signer<'info>,

//     #[account(
//         mut,
//         seeds = [b"stake", owner.key().as_ref()],
//         bump,
//         constraint = stake_account.owner == owner.key()
//     )]
//     pub stake_account: Account<'info, StakeAccount>,

//     /// CHECK: This is not dangerous because we don't read or write from this account
//     #[account(mut)]
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     pub stake_token_account: AccountInfo<'info>,
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     #[account(mut)]
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     pub to_token_account: AccountInfo<'info>,
//     #[account(mut)]
//     /// CHECK: This is not dangerous because we don't read or write from this account
//     pub fee_collector: AccountInfo<'info>,
//     pub token_program: Program<'info, Token>,
// }

// #[account]
// pub struct CustomOracleAccount {
//     pub price: u64, // Price in USD, for example
// }

// #[account]
// pub struct StakeAccount {
//     pub owner: Pubkey,
//     pub token_account: Pubkey,
//     pub staked_amount: u64,
//     pub locked_at: i64,
//     pub lock_duration: i64,
//     pub unlocked: bool,
// }

// #[derive(Accounts)]
// pub struct UpdatePrice<'info> {
//     #[account(mut)]
//     pub oracle_account: Account<'info, CustomOracleAccount>,
//     pub authority: Signer<'info>, // Authority to update the price
// }

// impl StakeAccount {
//     pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 1;
// }

// #[error_code]
// pub enum StakingError {
//     #[msg("Tokens are still locked")]
//     TokensStillLocked,
//     #[msg("Market cap has not reached unlock threshold $450M")]
//     MarketCapNotReached,
//     #[msg("You are not authorized to update the marketcap.")]
//     Unauthorized,
// }
