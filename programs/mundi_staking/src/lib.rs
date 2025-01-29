use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
// use std::str::FromStr;

// On chain address "mundi...98Qq" owns this program
// This below program Vanity address Nicholai found on his cluster

// BEGIN FREEZE: DO NOT CHANGE until the `// END FREEZE:` comment

// Program ID
declare_id!("mundi2P4tJmSUTg9DMA93NCX25RcKmDmrFU86z9xnV2");

// END FREEZE:

// Market cap unlock declaration
const UNLOCK_MARKET_CAP: u64 = 450_000_000; // $450M in USD

// Timestamp of ending of staking Georgian EASTER Apr 20 2025 GMT
// .. because we want to give people as much time until the end of "easter"
const STAKING_END_TIME: i64 = 1_745_197_200;

// Possible first date of unstaking... same time but maybe we extend to the future
const UNSTAKING_START_TIME: i64 = 1_745_197_200; // Unstaking start time, Easter Apr 20 2025 GMT

// Random testing notes....
#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
 
#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Flip Mundi",
    project_url: "https://flipmundi.com",
    contacts: "email:nickmitchko@gmail.com",
    policy: "https://github.com/nickmitchko/mundi_staking/security.txt",
 
    // Optional Fields
    preferred_languages: "en",
    source_code: "https://github.com/nickmitchko/mundi_staking",
    acknowledgements: "Thank you to our bug bounty hunters!"

    
}

#[program]
pub mod token_staking {
    // use std::f32::MIN;

    use super::*;

    pub fn initialize_stake(ctx: Context<InitializeStake>, lock_duration: i64) -> Result<()> {
        // msg!("Starting initialize_stake");
        // msg!("Owner: {}", ctx.accounts.owner.key());
        // msg!("Stake Account: {}", ctx.accounts.stake_account.key());
        // msg!("Stake Token Account: {}", ctx.accounts.token_account.key());
        // msg!("Mint: {}", ctx.accounts.mint.key());
        let stake_account = &mut ctx.accounts.stake_account;
        // msg!("This is a log message");
        stake_account.owner = ctx.accounts.owner.key();
        stake_account.token_account = ctx.accounts.token_account.key();
        stake_account.lock_duration = lock_duration;
        stake_account.locked_at = Clock::get()?.unix_timestamp;
        stake_account.unlocked = false;
        stake_account.staked_amount = 0u64; // Initialize staked amount
        stake_account.last_check = 0u64;
        // msg!("This is a log message");
        Ok(())
    }

    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            current_time < STAKING_END_TIME,
            StakingError::TooLateToStake
        );
        // msg!("Starting stake_tokens instruction");
        // msg!("Owner: {}", ctx.accounts.owner.key());
        // msg!("Stake Account: {}", ctx.accounts.stake_account.key());
        // msg!(
        //     "From Token Account: {}",
        //     ctx.accounts.from_token_account.key()
        // );
        // msg!(
        //     "Stake Token Account: {}",
        //     ctx.accounts.stake_token_account.key()
        // );
        // msg!("Mint: {}", ctx.accounts.mint.key());
        // msg!("Entering stake_tokens function");
        // msg!("Stake amount provided: {}", amount);

        let stake_account = &mut ctx.accounts.stake_account;
        let rewards_account = &mut ctx.accounts.rewards_account;
        // msg!("Stake account address: {}", stake_account.key());

        // Verify the stake token account matches the one in stake_account
        // msg!("Checking if stake token account matches the one in stake_account");
        require!(
            ctx.accounts.from_token_account.key() == stake_account.token_account,
            StakingError::InvalidTokenAccount
        );
        // msg!("Stake token account matches the one in stake_account");
        // require!(
        //     amount >= MIN_STAKE_AMOUNT,
        //     StakingError::MinimumStakeAmount
        // );
        // msg!("Stake token account matches the one in stake_account");

        // Transfer tokens to the stake account
        // msg!("Preparing to transfer tokens to the stake account");
        let cpi_accounts = Transfer {
            from: ctx.accounts.from_token_account.to_account_info(),
            to: ctx.accounts.stake_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        // msg!("From token account: {}", cpi_accounts.from.key());
        // msg!("To stake token account: {}", cpi_accounts.to.key());
        // msg!("Authority account: {}", cpi_accounts.authority.key());

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        // msg!("Executing token transfer CPI");
        token::transfer(cpi_ctx, amount)?;
        // msg!("Token transfer successful");

        // Update the staked amount in the stake account
        // msg!("Updating staked amount in the stake account");
        stake_account.staked_amount = stake_account
            .staked_amount
            .checked_add(amount)
            .ok_or(StakingError::NumericOverflow)?;
        // msg!("Staked amount updated to: {}", stake_account.staked_amount);
        rewards_account.total_staked = rewards_account
            .total_staked
            .checked_add(amount)
            .ok_or(StakingError::NumericOverflow)?;

        // msg!("Exiting stake_tokens function successfully");
        Ok(())
    }

    pub fn check_unlock_conditions(ctx: Context<CheckUnlock>) -> Result<bool> {
        let stake_account = &mut ctx.accounts.stake_account;
        let oracle_account = &ctx.accounts.oracle_account;
        let token_mint = ctx.accounts.token_mint.to_account_info();
        ctx.program_id;

        // Safely deserialize the mint account
        let mint = Mint::try_deserialize(&mut &token_mint.data.borrow()[..])?;

        // msg!("Oracle price: {}", oracle_account.price);
        // msg!("Mint supply: {}", mint.supply);
        // Calculate market cap with overflow protection
        let market_cap = (oracle_account.price as f64) * (mint.supply as f64);
        // msg!(
        // "Calculated raw market cap (before division): {}",
        // market_cap
        // );
        // .checked_mul(mint.supply as f64)
        // .ok_or(StakingError::NumericOverflow)?;

        // Do a 1M div so that it's actually in the correct units
        // let divisor:u64 = 1_000_000_000 pow? mint.decimals
        // let market_cap = (market_cap / 1_000_000_000f64) as u64;
        let divisor = 10_u64.pow(mint.decimals as u32);
        let market_cap = (market_cap / (divisor as f64)) as u64;
        msg!("Market cap (after division): {}", market_cap);
        stake_account.last_check = market_cap;

        // .div(1_000_000_000)
        // .ok_or(StakingError::NumericOverflow)? as u64;

        // Check if market cap has reached unlock threshold

        let time_passed =
            Clock::get()?.unix_timestamp - stake_account.locked_at - stake_account.lock_duration;
        // msg!("Time passed since lock: {}", time_passed);
        if market_cap >= UNLOCK_MARKET_CAP && time_passed > 0 {
            // msg!("Unlock conditions met. Unlocking stake account.");
            stake_account.unlocked = true;
        } else {
            // msg!("Unlock conditions not met. Stake account remains locked.");
            stake_account.unlocked = false;
        }

        Ok(stake_account.unlocked)
    }

    pub fn unstake_tokens(ctx: Context<UnstakeTokens>) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        let current_time = Clock::get()?.unix_timestamp;
        let _market_cap = stake_account.last_check;
        let _rewards_account = &mut ctx.accounts.rewards_account;
        let _rewards_token_account = &mut ctx.accounts.rewards_token_account;

        // Check both time lock and market cap conditions
        require!(
            current_time >= stake_account.locked_at + stake_account.lock_duration,
            StakingError::TokensStillLocked
        );

        require!(
            current_time >= UNSTAKING_START_TIME,
            StakingError::TooEarlyToUnStake
        );

        require!(stake_account.unlocked, StakingError::MarketCapNotReached);

        // Verify the stake token account
        require!(
            ctx.accounts.to_token_account.key() == stake_account.token_account,
            StakingError::InvalidTokenAccount
        );


        // // Get stake account PDA signer
        let (_pda, bump) = Pubkey::find_program_address(
            &[b"stak", ctx.accounts.owner.key().as_ref()],
            ctx.program_id,
        );

        let binding = ctx.accounts.mint.key();
        let _mintkey = binding.as_ref();

        let (_pda, _reward_bump) = Pubkey::find_program_address(
            &[b"reward", ctx.accounts.mint.key().as_ref()],
            ctx.program_id,
        );

        // let stake_account_bump = ctx.bumps["stake_account"];
        // PDA seeds for signer
        let seeds = &[b"stak", ctx.accounts.owner.key.as_ref(), &[bump]];
        let signer_seeds = &[&seeds[..]];

        let mut remaining_amount: u64 = ctx.accounts.stake_token_account.amount; // User's remaining amount
        let staked_amount = ctx.accounts.stake_token_account.amount;


        // Check for the existence of the fee account, if it exists, then send the fee (tip)
        if let Some(fee_account) = &ctx.accounts.fee_collector {
            // Calculate fee with overflow protection
            let fee_amount = staked_amount
                .checked_mul(5)
                .ok_or(StakingError::NumericOverflow)?
                .checked_div(1000)
                .ok_or(StakingError::NumericOverflow)?;

            // msg!("Fee Amount: {}", fee_amount);

            // Transfer remaining tokens back to owner
            remaining_amount = staked_amount
                .checked_sub(fee_amount)
                .ok_or(StakingError::NumericOverflow)?;

            require!(
                fee_amount.checked_add(remaining_amount).unwrap() == staked_amount,
                StakingError::NumericOverflow
            );

            // msg!("Remaining Amount: {}", remaining_amount);

            let cpi_accounts = Transfer {
                from: ctx.accounts.stake_token_account.to_account_info(),
                to: fee_account.to_account_info(),
                authority: stake_account.to_account_info(),
            };

            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
            // msg!("Executing token transfer CPI");
            token::transfer(cpi_ctx, fee_amount)?;
            // msg!("Token transfer successful");
            // remaining_amount f;
        }

        let cpi_accounts = Transfer {
            from: ctx.accounts.stake_token_account.to_account_info(),
            to: ctx.accounts.to_token_account.to_account_info(),
            authority: stake_account.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        // msg!("Executing token transfer CPI");
        token::transfer(cpi_ctx, remaining_amount)?;

        stake_account.unlocked = false;
        stake_account.staked_amount = 0;

        Ok(())
    }

    pub fn update_price(ctx: Context<UpdatePrice>, price: f64) -> Result<()> {
        let oracle_account = &mut ctx.accounts.oracle_account;
        oracle_account.price = price;
        Ok(())
    }

    pub fn initialize_oracle(ctx: Context<InitializeOracle>) -> Result<()> {
        ctx.accounts.oracle_account.authority = ctx.accounts.authority.key();
        Ok(())
    }

    pub fn donate_to_rewards(ctx: Context<DonateToRewards>, amount: u64) -> Result<()> {
        let rewards_account = &mut ctx.accounts.rewards_account;
        let user_token_account = &ctx.accounts.user_token_account;
        let rewards_token_account = &mut ctx.accounts.rewards_token_account;
        let mint = &ctx.accounts.mint;
        let payer = &mut ctx.accounts.payer;
        let token_program = &ctx.accounts.token_program;
        // let associated_token_program = &ctx.accounts.associated_token_program;
        // let system_program = &ctx.accounts.system_program;

        // Ensure the user_token_account is associated with the correct mint
        require!(
            user_token_account.mint == mint.key(),
            StakingError::InvalidMint
        );

        // Ensure the rewards_token_account is associated with the correct mint
        require!(
            rewards_token_account.mint == mint.key(),
            StakingError::InvalidMint
        );

        // Transfer tokens from user's token account to rewards token account
        token::transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: user_token_account.to_account_info(),
                    to: rewards_token_account.to_account_info(),
                    authority: payer.to_account_info(),
                },
            ),
            amount,
        )?;

        // Update rewards_account total_rewards
        rewards_account.total_rewards += amount;
        rewards_account.mint = mint.key();
        rewards_account.total_donations += amount;
        rewards_account.is_initialized = true;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct DonateToRewards<'info> {
    #[account(
        init_if_needed,
        seeds = [b"reward", mint.key().as_ref()],
        bump,
        space = 8 + RewardsAccount::LEN, // discriminator (8) + struct size
        payer = payer
    )]
    pub rewards_account: Account<'info, RewardsAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint, 
        associated_token::authority = rewards_account,
    )]
    pub rewards_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,         
        associated_token::mint = mint, 
        associated_token::authority = payer
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeOracle<'info> {
    #[account(
        init, 
        payer = authority, 
        seeds = [b"oracle", mint.key().as_ref()],
        bump,
        space = 8 + 8 + 32
    )]
    pub oracle_account: Account<'info, CustomOracleAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeStake<'info> {
    // Account Owner and Signer account for this transaction
    #[account(mut)]
    pub owner: Signer<'info>,
    // Token ATA for the account owner (ATA), ensure that the signer can owns the token account
    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner.key()
    )]
    pub token_account: Account<'info, TokenAccount>,
    // an account we are going to initialize
    pub mint: Account<'info, Mint>,
    //
    #[account(
        init,
        payer = owner,
        space = 8 + StakeAccount::LEN,
        seeds = [b"stak", owner.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CheckUnlock<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"stak", owner.key().as_ref()],
        bump)]
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
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        seeds = [b"reward", mint.key().as_ref()],
        bump,
    )]
    pub rewards_account: Account<'info, RewardsAccount>,

    #[account(
        mut,
        constraint = from_token_account.owner == owner.key(),
        constraint = from_token_account.mint == mint.key()
    )]
    pub from_token_account: Account<'info, TokenAccount>,

    // Derived using spl-token address --verbose --token FeEucpirJWULUQwgwoiDJxzwoN7tMWFLTXEaupgSt8F3 --owner E9qirjcmLihw1J5ZRYeGyjdBvSVhqqwHyzXezYhfaZWc
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint, 
        associated_token::authority = stake_account,
    )]
    pub stake_token_account: Account<'info, TokenAccount>,
    // an account we are going to initialize
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"stak", owner.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        seeds = [b"reward", mint.key().as_ref()],
        bump,
    )]
    pub rewards_account: Account<'info, RewardsAccount>,

    // Added so that we can distribute rewards from the unstake action
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = rewards_account,
    )]
    pub rewards_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = to_token_account.owner == owner.key(),
        constraint = to_token_account.mint == mint.key()
    )]
    pub to_token_account: Account<'info, TokenAccount>,

    // Derived using spl-token address --verbose --token FeEucpirJWULUQwgwoiDJxzwoN7tMWFLTXEaupgSt8F3 --owner E9qirjcmLihw1J5ZRYeGyjdBvSVhqqwHyzXezYhfaZWc
    #[account(
        mut,
        constraint = stake_token_account.owner == stake_account.key(),
        constraint = stake_token_account.mint == mint.key()
    )]
    pub stake_token_account: Account<'info, TokenAccount>,
    // an account we are going to initialize
    pub mint: Account<'info, Mint>,
    // Fee is optional
    #[account(mut)]
    pub fee_collector: Option<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[account]
pub struct CustomOracleAccount {
    pub price: f64,
    pub authority: Pubkey,
}

#[account]
pub struct StakeAccount {
    pub owner: Pubkey,
    pub token_account: Pubkey,
    pub staked_amount: u64,
    pub locked_at: i64,
    pub lock_duration: i64,
    pub unlocked: bool,
    pub last_check: u64, // Market cap last checked at
}

#[account]
pub struct RewardsAccount {
    pub mint: Pubkey,
    pub total_rewards: u64,
    pub distributed_rewards: u64, // Rewards distributed so far
    pub total_staked: u64,        // Total staked tokens across all users
    pub total_donations: u64,     // Total donations to the rewards pool
    pub is_initialized: bool,
}

impl RewardsAccount {
    pub const LEN: usize = 8 + // account discriminator
                     32 + // mint Pubkey
                     8 + // total_rewards u64
                     8 + // is_initialized bool
                     8 + 1; // padding to make it 8-byte aligned
}

#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    #[account(
        mut,
        constraint = oracle_account.authority == authority.key(),
        seeds = [b"oracle", mint.key().as_ref()],
        bump
    )]
    pub oracle_account: Account<'info, CustomOracleAccount>,
    #[account(
        constraint = authority.key() == oracle_account.authority @ StakingError::Unauthorized
    )]
    pub authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
}

impl StakeAccount {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 1 + 8;
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
    #[msg("Token Mint Does Not Exist")]
    InvalidMint,
    #[msg("The staking period has ended")]
    TooLateToStake,
    #[msg("The un-staking period has not begun yet (after orthodox easter 2025)")]
    TooEarlyToUnStake,
}