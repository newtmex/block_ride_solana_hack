use solana_program::{pubkey, pubkey::Pubkey};
use std::str::FromStr;
use {
    anchor_lang::prelude::*,
    anchor_spl::{associated_token, mint, token},
    mpl_token_metadata::{
        instructions::{
            CreateMetadataAccountV3Cpi, CreateMetadataAccountV3CpiAccounts,
            CreateMetadataAccountV3InstructionArgs,
        },
        types::DataV2,
        ID as MPL_TOKEN_METADATA_ID,
    },
};

declare_id!("HU1W9jfeRjjMYK922unNRWUo2rAha1WFMuoxhTfN5aad");
pub const CUSTOM_USDC_MINT: Pubkey = pubkey!("CY8wKkNH5UwTLpY4gg4eJGYF8fHdyzjMLtn1MopDoN5A");

#[program]
pub mod octo_program {
    use super::*;

    pub fn initialize_program(
        ctx: Context<InitializeProgram>,
        grand_authority: Pubkey,
    ) -> Result<()> {
        let program_data: &mut Box<Account<'_, ProjectData>> = &mut ctx.accounts.project_pda;
        program_data.grand_authority = grand_authority;
        Ok(())
    }

    pub fn update_program_grand_authority(
        ctx: Context<UpdateProgramGrandAuthority>,
        new_grand_authority: Pubkey,
    ) -> Result<()> {
        let program_data: &mut Box<Account<'_, ProjectData>> = &mut ctx.accounts.project_pda;
        program_data.grand_authority = new_grand_authority;
        Ok(())
    }

    pub fn add_pool_creator(
        ctx: Context<AddCreator>,
        creator: Pubkey,
        can_create: bool,
    ) -> Result<()> {
        let pool_creator: &mut Box<Account<'_, PoolCreatorData>> =
            &mut ctx.accounts.pool_creator_data;
        pool_creator.creator = creator;
        pool_creator.can_create = can_create;
        Ok(())
    }

    pub fn update_pool_creator(ctx: Context<UpdatePoolCreator>, can_create: bool) -> Result<()> {
        let pool_creator: &mut Box<Account<'_, PoolCreatorData>> =
            &mut ctx.accounts.pool_creator_data;
        pool_creator.can_create = can_create;
        Ok(())
    }

    pub fn create_pool(
        ctx: Context<CreatePool>,
        reference: Pubkey,
        authority: Pubkey,
        // seed: u64,
        shares: u64,
        deposit: u64,
        name: String,
        symbol: String,
        uri: String,
        start_date: u64,
        maturity_date: u64,
        apy: u8,
    ) -> Result<()> {
        // Get the pool account
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;

        // Initialize the pool account
        pool.init(
            [
                *ctx.accounts.creator.key,
                authority,
                reference,
                // *ctx.accounts.reference.key,
                ctx.accounts.mint.key(),
            ],
            [shares, start_date, maturity_date],
            ctx.bumps.pool,
            apy,
        )?;

        // Get the shares minted from the deposit
        let minted: u64 = pool.get_shares_from_deposit(deposit);
        // let clock = Clock::get()?;
        // Validate the input
        // require_gt!(
        //     start_date,
        //     clock.unix_timestamp,
        //     ErrorCode::StartDatePassed
        // );
        // require_gt!(
        //     maturity_date,
        //     clock.unix_timestamp,
        //     ErrorCode::MaturityDatePassed
        // );
        // require_gt!(maturity_date, start_date, ErrorCode::StartDatePassed);
        // if !ctx.accounts.pool_creator_data.can_create || ctx.accounts.pool_creator_data.creator != ctx.accounts.creator.key() {
        //  return Err(ErrorCode::CreatorNotAuthorized.into());
        // }
        require_gte!(shares, 1_u64, ErrorCode::MinimumShares);
        require_gte!(deposit, Pool::MIN_DEPOSIT, ErrorCode::MinimumDeposit);
        // require_eq!(seed % shares, 0, ErrorCode::InvalidSeedSharesRatio);
        require!(pool.is_valid_deposit(deposit), ErrorCode::MinimumDeposit);
        require_gte!(
            pool.shares,
            pool.minted + minted,
            ErrorCode::ExceedsAvailableShares
        );

        // Signer seeds
        let pool_seeds: &[&[u8]; 3] = &[b"pool", pool.reference.as_ref(), &[pool.bump]];
        let signers_seeds: &[&[&[u8]]; 1] = &[&pool_seeds[..]];

        // Send the USDC to the pool account
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.creator_usdc_account.to_account_info(),
                    to: ctx.accounts.pool_usdc_account.to_account_info(),
                    authority: ctx.accounts.creator.to_account_info(),
                },
            ),
            deposit,
        )?;

        // Create the metadata account
        let metadata_program: &AccountInfo<'_> = &ctx.accounts.metadata_program.to_account_info();
        let metadata: &AccountInfo<'_> = &ctx.accounts.metadata.to_account_info();
        let mint: &AccountInfo<'_> = &ctx.accounts.mint.to_account_info();
        let mint_authority: &AccountInfo<'_> = &pool.to_account_info();
        let payer: &AccountInfo<'_> = &ctx.accounts.creator.to_account_info();
        let system_program: &AccountInfo<'_> = &ctx.accounts.system_program.to_account_info();
        let rent: &AccountInfo<'_> = &ctx.accounts.rent.to_account_info();
        let update_authority: &AccountInfo<'_> = &pool.to_account_info();

        let metadata_v3_cpi: CreateMetadataAccountV3Cpi<'_, '_> = CreateMetadataAccountV3Cpi::new(
            metadata_program,
            CreateMetadataAccountV3CpiAccounts {
                metadata,
                mint,
                mint_authority,
                payer,
                system_program,
                rent: Some(rent),
                update_authority: (update_authority, true),
            },
            CreateMetadataAccountV3InstructionArgs {
                collection_details: None,
                data: DataV2 {
                    name: name.to_string(),
                    symbol: symbol.to_string(),
                    uri: uri.to_string(),
                    seller_fee_basis_points: 0,
                    creators: None,
                    collection: None,
                    uses: None,
                },
                is_mutable: true,
            },
        );
        metadata_v3_cpi.invoke_signed(signers_seeds)?;

        // Mint tokens to the creator token account
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    authority: pool.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.creator_mint_account.to_account_info(),
                },
                signers_seeds,
            ),
            minted,
        )?;

        // Update the pool account
        pool.add_minted(minted);

        Ok(())
    }

    pub fn buy_shares(ctx: Context<BuyShares>, shares: u64) -> Result<()> {
        // Get the pool account
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;

        // Get deposit from shares
        let deposit: u64 = pool.get_deposit_from_shares(shares);

        // Validate the input
        // require!(!pool.investment_period_ended(), ErrorCode::StartDatePassed);
        require!(pool.is_valid_deposit(deposit), ErrorCode::MinimumDeposit);
        require_gte!(
            pool.shares,
            pool.minted + shares,
            ErrorCode::ExceedsAvailableShares
        );

        // Signer seeds
        let pool_seeds: &[&[u8]; 3] = &[b"pool", pool.reference.as_ref(), &[pool.bump]];
        let signers_seeds: &[&[&[u8]]; 1] = &[&pool_seeds[..]];

        // Send the USDC to the pool account
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.buyer_usdc_account.to_account_info(),
                    to: ctx.accounts.pool_usdc_account.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            deposit,
        )?;

        // Mint tokens to the creator token account
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    authority: pool.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.buyer_mint_account.to_account_info(),
                },
                signers_seeds,
            ),
            shares,
        )?;

        // Update the pool account
        pool.add_minted(shares);

        Ok(())
    }

    pub fn distribute(ctx: Context<Distribute>, amount: u64) -> Result<()> {
        // Get accounts
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;
        let distribution: &mut Account<'_, Distribution> = &mut ctx.accounts.distribution;

        // Initialize the distribution account
        // distribution.set(
        //     pool.key(),
        //     ctx.accounts.distribution_authority.key(),
        //     ctx.bumps.distribution,
        // )?;

        // Check if the distribution has enough USDC
        // require!(
        //     pool.investment_period_ended(),
        //     ErrorCode::StartDateNotPassed
        // );
        // require_gte!(
        //     ctx.accounts.distribution_usdc_account.amount,
        //     (distribution.rewards - distribution.claimed) + amount,
        //     ErrorCode::InsufficientDistributionUSDCBalance
        // );

        // Send the USDC to the distribution USDC account
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.signer_usdc_account.to_account_info(),
                    to: ctx.accounts.distribution_usdc_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            amount,
        )?;

        // Add rewards to the distribution
        distribution.add_rewards(amount);

        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>, rewards: u64) -> Result<()> {
        // Get accounts
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;
        let distribution: &mut Account<'_, Distribution> = &mut ctx.accounts.distribution;

        // Check if the distribution has enough USDC

        require_gte!(
            ctx.accounts.distribution_usdc_account.amount,
            distribution.rewards - distribution.claimed,
            ErrorCode::InsufficientDistributionUSDCBalance
        );
        require_gte!(
            ctx.accounts.distribution_usdc_account.amount,
            rewards,
            ErrorCode::InsufficientDistributionUSDCBalance
        );
        // require!(
        //     pool.investment_period_ended(),
        //     ErrorCode::StartDateNotPassed
        // );
        // require!(!pool.maturity_date_passed(), ErrorCode::MaturityDatePassed);
        // Signer seeds
        let distribution_seeds: &[&[u8]; 3] = &[
            b"distribution",
            distribution.pool.as_ref(),
            &[distribution.bump],
        ];
        let signers_seeds: &[&[&[u8]]; 1] = &[&distribution_seeds[..]];

        // Transfer the USDC to the holder account
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.distribution_usdc_account.to_account_info(),
                    to: ctx.accounts.holder_usdc_account.to_account_info(),
                    authority: distribution.to_account_info(),
                },
                signers_seeds,
            ),
            rewards,
        )?;

        // Update the distribution account
        distribution.add_claimed(rewards);

        Ok(())
    }

    pub fn withdraw_from_pool(ctx: Context<WithdrawFromPool>, shares: u64) -> Result<()> {
        // Get accounts
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;

        let pool_usdc_balance: u64 = ctx.accounts.pool_usdc_account.amount;
        let amount: u64 = pool.get_deposit_from_shares(shares);

        // Validations
        // require_eq!(pool.shares, pool.minted, ErrorCode::SeedRoundsNotCompleted);
        // require!(
        //     pool.investment_period_ended(),
        //     ErrorCode::StartDateNotPassed
        // );
        require_gte!(
            pool_usdc_balance,
            amount,
            ErrorCode::InsufficientPoolUSDCBalance
        );

        // Signer seeds
        let pool_seeds: &[&[u8]; 3] = &[b"pool", pool.reference.as_ref(), &[pool.bump]];
        let signers_seeds: &[&[&[u8]]; 1] = &[&pool_seeds[..]];

        // Transfer the USDC
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.pool_usdc_account.to_account_info(),
                    to: ctx.accounts.to_usdc_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                signers_seeds,
            ),
            amount,
        )?;

        Ok(())
    }

    pub fn close_pool(ctx: Context<ClosePool>) -> Result<()> {
        let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;
        let distribution: &mut Account<'_, Distribution> = &mut ctx.accounts.distribution;

        // Initialize the distribution account
        distribution.set(
            pool.key(),
            ctx.accounts.distribution_authority.key(),
            ctx.bumps.distribution,
        )?;
        pool.closed = true;

        Ok(())
    }

    // pub fn claim_deposit(ctx: Context<ClaimDeposit>) -> Result<()> {
    //     // Get the pool account
    //     let pool: &mut Account<'_, Pool> = &mut ctx.accounts.pool;

    //     // Signer seeds
    //     let pool_seeds: &[&[u8]; 3] = &[b"pool", pool.reference.as_ref(), &[pool.bump]];
    //     let signers_seeds: &[&[&[u8]]; 1] = &[&pool_seeds[..]];

    //     // Transfer the USDC to the user account
    //     token::transfer(
    //         CpiContext::new_with_signer(
    //             ctx.accounts.token_program.to_account_info(),
    //             token::Transfer {
    //                 from: ctx.accounts.pool_usdc_account.to_account_info(),
    //                 to: ctx.accounts.holder_usdc_account.to_account_info(),
    //                 authority: pool.to_account_info(),
    //             },
    //             signers_seeds,
    //         ),
    //         pool.get_deposit_from_shares(ctx.accounts.holder_mint_account.amount),
    //     )?;

    //     // Burn tokens from the holder token account
    //     token::burn(
    //         CpiContext::new(
    //             ctx.accounts.token_program.to_account_info(),
    //             token::Burn {
    //                 authority: ctx.accounts.holder.to_account_info(),
    //                 mint: ctx.accounts.mint.to_account_info(),
    //                 from: ctx.accounts.holder_mint_account.to_account_info(),
    //             },
    //         ),
    //         ctx.accounts.holder_mint_account.amount,
    //     )?;

    //     Ok(())
    // }

    pub fn close_pool_accounts(_ctx: Context<ClosePoolAccounts>) -> Result<()> {
        Ok(())
    }
}

#[constant]
pub const PROJECT_PDA: &[u8] = b"BLOCKRIDE_SYSTEM";
#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(init,
     payer = creator,
     seeds=[PROJECT_PDA],bump,
      space = 8 + std::mem::size_of::<ProjectData>())]
    pub project_pda: Box<Account<'info, ProjectData>>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct ProjectData {
    pub grand_authority: Pubkey,
}

#[derive(Accounts)]
pub struct UpdateProgramGrandAuthority<'info> {
    #[account(mut,address=project_pda.grand_authority @ ErrorCode::SignerNotAuthorized)]
    pub grand_authority: Signer<'info>,

    #[account(mut,
     seeds=[PROJECT_PDA],bump)]
    pub project_pda: Box<Account<'info, ProjectData>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(creator: Pubkey)]
pub struct AddCreator<'info> {
    #[account(mut,address = project_pda.grand_authority @ ErrorCode::SignerNotAuthorized)]
    pub grand_authority: Signer<'info>,

    #[account(mut,
     seeds=[PROJECT_PDA],bump)]
    pub project_pda: Box<Account<'info, ProjectData>>,

    #[account(init,
     payer = grand_authority,
     seeds=[PROJECT_PDA,creator.as_ref()],bump,
      space = 8 + std::mem::size_of::<PoolCreatorData>())]
    pub pool_creator_data: Box<Account<'info, PoolCreatorData>>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct PoolCreatorData {
    pub creator: Pubkey,
    pub can_create: bool,
}

#[derive(Accounts)]
// #[instruction(poolcreator: Pubkey)]
pub struct UpdatePoolCreator<'info> {
    #[account(mut,
     seeds=[PROJECT_PDA,pool_creator_data.creator.as_ref()],bump)]
    pub pool_creator_data: Box<Account<'info, PoolCreatorData>>,

    #[account(mut, address = project_pda.grand_authority @ ErrorCode::SignerNotAuthorized)]
    pub creator: Signer<'info>,
    #[account(mut,
     seeds=[PROJECT_PDA],bump)]
    pub project_pda: Box<Account<'info, ProjectData>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(reference:Pubkey)]
pub struct CreatePool<'info> {
    // #[account(mut,
    //  seeds=[PROJECT_PDA,pool_creator_data.creator.as_ref()],bump)]
    // pub pool_creator_data: Account<'info, PoolCreatorData>,
    #[account(mut, 
    //     address = pool_creator_data.creator @ ErrorCode::CreatorNotAuthorized,
    // constraint = pool_creator_data.can_create == true @ ErrorCode::CreatorNotAuthorized
 )]
    pub creator: Signer<'info>,

    // An ephemeral account that is used as a seed for the Pool PDA.
    // Must be a signer to prevent front-running attack by someone else but the original creator.
    // pub reference: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = 8 + std::mem::size_of::<Pool>(),
        // seeds = [b"pool", reference.as_ref()],
        seeds = [b"pool", reference.as_ref()],
        bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        init,
        payer = creator,
        mint::decimals = 0,
        mint::authority = pool,
        mint::freeze_authority = pool,
        token::token_program = token_program,
        seeds = [b"mint", pool.key().as_ref()],
        bump,
    )]
    pub mint: Account<'info, token::Mint>,

    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = creator,
        associated_token::token_program = token_program,
    )]
    pub creator_mint_account: Account<'info, token::TokenAccount>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = creator,
    )]
    pub creator_usdc_account: Account<'info, token::TokenAccount>,

    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = usdc_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program,
    )]
    pub pool_usdc_account: Account<'info, token::TokenAccount>,

    /// CHECKS: The metadata account.
    #[account(
        mut,
        seeds = [b"metadata", metadata_program.key().as_ref(), mint.key().as_ref()],
        bump,
        seeds::program = metadata_program,
    )]
    pub metadata: UncheckedAccount<'info>,

    /// CHECKS: The metadata program.
    #[account(address = MPL_TOKEN_METADATA_ID)]
    pub metadata_program: UncheckedAccount<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct BuyShares<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        constraint = !pool.closed @ ErrorCode::PoolClosed,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        token::token_program = token_program,
        seeds = [b"mint", pool.key().as_ref()],
        bump,
    )]
    pub mint: Account<'info, token::Mint>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = mint,
        associated_token::authority = buyer,
        associated_token::token_program = token_program,
    )]
    pub buyer_mint_account: Account<'info, token::TokenAccount>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = buyer,
        associated_token::token_program = token_program,
    )]
    pub buyer_usdc_account: Account<'info, token::TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program,
    )]
    pub pool_usdc_account: Account<'info, token::TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct Distribute<'info> {
    // #[account(
    //     mut,
    //     address = pool.authority,
    // )]
    // authority: Signer<'info>,
    #[account(mut,)]
    signer: Signer<'info>,

    #[account(address = distribution.authority)]
    distribution_authority: Signer<'info>,

    #[account(
        mut,
        constraint = pool.closed @ ErrorCode::PoolClosed,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = signer,
    )]
    pub signer_usdc_account: Account<'info, token::TokenAccount>,
    #[account(
        mut,
        seeds = [b"distribution", pool.key().as_ref()],
        bump,
    )]
    pub distribution: Account<'info, Distribution>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = distribution,
        associated_token::token_program = token_program,
    )]
    pub distribution_usdc_account: Account<'info, token::TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    holder: Signer<'info>,

    #[account(address = distribution.authority)]
    authority: Signer<'info>,

    #[account(
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        seeds = [b"distribution", pool.key().as_ref()],
        bump = distribution.bump,
    )]
    pub distribution: Account<'info, Distribution>,

    #[account(
        token::token_program = token_program,
        seeds = [b"mint", pool.key().as_ref()],
        bump,
    )]
    pub mint: Account<'info, token::Mint>,

    #[account(
        associated_token::mint = mint,
        associated_token::authority = holder,
        associated_token::token_program = token_program,
    )]
    pub holder_mint_account: Account<'info, token::TokenAccount>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = distribution,
        associated_token::token_program = token_program,
    )]
    pub distribution_usdc_account: Account<'info, token::TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = holder,
        associated_token::token_program = token_program,
    )]
    pub holder_usdc_account: Account<'info, token::TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct WithdrawFromPool<'info> {
    #[account(
        mut,
        address = pool.authority,
    )]
    authority: Signer<'info>,

    #[account(
        mut,
        constraint = !pool.closed @ ErrorCode::PoolClosed,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program,
    )]
    pub pool_usdc_account: Account<'info, token::TokenAccount>,

    #[account(mut)]
    pub to_usdc_account: Account<'info, token::TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct ClosePool<'info> {
    #[account(
        mut,
        address = pool.authority,
    )]
    pub authority: Signer<'info>,

    #[account(
        mut,
        constraint = !pool.closed @ ErrorCode::PoolClosed,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + std::mem::size_of::<Distribution>(),
        seeds = [b"distribution", pool.key().as_ref()],
        bump,
    )]
    pub distribution: Account<'info, Distribution>,

    distribution_authority: Signer<'info>,
    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = usdc_mint,
        associated_token::authority = distribution,
        associated_token::token_program = token_program,
    )]
    pub distribution_usdc_account: Account<'info, token::TokenAccount>,

    pub token_program: Program<'info, token::Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
}

#[derive(Accounts)]
pub struct ClaimDeposit<'info> {
    #[account(mut)]
    holder: Signer<'info>,

    #[account(
        constraint = pool.closed @ ErrorCode::PoolNotClosed,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        token::token_program = token_program,
        seeds = [b"mint", pool.key().as_ref()],
        bump,
    )]
    pub mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = holder,
        associated_token::token_program = token_program,
    )]
    pub holder_mint_account: Account<'info, token::TokenAccount>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program,
    )]
    pub pool_usdc_account: Account<'info, token::TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = holder,
        associated_token::token_program = token_program,
    )]
    pub holder_usdc_account: Account<'info, token::TokenAccount>,

    pub token_program: Program<'info, token::Token>,
}

#[derive(Accounts)]
pub struct ClosePoolAccounts<'info> {
    #[account(
        mut,
        address = pool.authority,
    )]
    pub authority: Signer<'info>,

    #[account(
        mut,
        constraint = pool.closed @ ErrorCode::PoolNotClosed,
        close = authority,
        seeds = [b"pool", pool.reference.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        constraint = distribution.rewards == distribution.claimed @ ErrorCode::UnclaimedDistributionRewards,
        close = authority,
        seeds = [b"distribution", pool.key().as_ref()],
        bump = distribution.bump
    )]
    pub distribution: Account<'info, Distribution>,

    #[account(
        mut,
        token::token_program = token_program,
        seeds = [b"mint", pool.key().as_ref()],
        bump,
    )]
    pub mint: Account<'info, token::Mint>,

    /// CHECKS: The USDC mint.
    #[account(address = CUSTOM_USDC_MINT)]
    pub usdc_mint: Account<'info, token::Mint>,

    #[account(
        mut,
        constraint = pool_usdc_account.amount == 0 @ ErrorCode::NonZeroPoolUSDCBalance,
        associated_token::mint = usdc_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program,
    )]
    pub pool_usdc_account: Account<'info, token::TokenAccount>,

    #[account(
        mut,
        constraint = distribution_usdc_account.amount == 0 @ ErrorCode::NonZeroDistributionUSDCBalance,
        associated_token::mint = usdc_mint,
        associated_token::authority = distribution,
        associated_token::token_program = token_program,
    )]
    pub distribution_usdc_account: Account<'info, token::TokenAccount>,

    pub token_program: Program<'info, token::Token>,
}

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub creator: Pubkey,
    pub authority: Pubkey,
    pub reference: Pubkey,
    pub mint: Pubkey,
    // pub seed: u64,
    pub shares: u64,
    pub minted: u64,
    pub closed: bool,
    pub bump: u8,
    pub start_date: u64,
    pub maturity_date: u64,
    pub apy: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Distribution {
    pub pool: Pubkey,
    pub authority: Pubkey,
    pub rewards: u64,
    pub claimed: u64,
    pub bump: u8,
}

impl Pool {
    pub const MIN_DEPOSIT: u64 = 100_u64 * 1e6 as u64;

    pub fn init(
        &mut self,
        [creator, authority, reference, mint]: [Pubkey; 4],
        [shares, start_date, maturity_date]: [u64; 3],
        bump: u8,
        apy: u8,
    ) -> Result<()> {
        self.creator = creator;
        self.authority = authority;
        self.reference = reference;
        self.mint = mint;
        // self.seed = seed;
        self.shares = shares;
        self.minted = 0;
        self.closed = false;
        self.bump = bump;
        self.start_date = start_date;
        self.maturity_date = maturity_date;
        self.apy = apy;
        Ok(())
    }

    pub fn get_shares_from_deposit(&self, deposit: u64) -> u64 {
        (deposit * self.shares) / (self.shares * 1e6 as u64)
    }

    pub fn get_deposit_from_shares(&self, shares: u64) -> u64 {
        (shares * (self.shares * 1e6 as u64)) / self.shares
    }

    pub fn get_min_deposit(&self) -> u64 {
        self.get_deposit_from_shares(1)
    }

    pub fn is_valid_deposit(&self, deposit: u64) -> bool {
        deposit % self.get_min_deposit() == 0
    }

    pub fn add_minted(&mut self, minted: u64) {
        self.minted += minted;
    }

    pub fn sub_minted(&mut self, minted: u64) {
        self.minted -= minted;
    }
    // pub fn investment_period_ended(&self) -> bool {
    //     let clock = Clock::get();
    //     match clock{
    //         Ok(T)=>{
    //             if T.unix_timestamp > self.start_date{
    //                 return true;
    //             }
    //             return false
    //         },
    //         Err(_E)=>{
    //             return false;
    //         }
    //     }
    // if clock.unix_timestamp > self.start_date{
    //     return true;
    // }
    // return false
    // }

    // pub fn maturity_date_passed(&self) -> bool {
    //     let clock = Clock::get();
    //     match clock{
    //         Ok(T)=>{
    //             if T.unix_timestamp > self.maturity_date{
    //                 return true;
    //             }
    //             return false
    //         },
    //         Err(_E)=>{
    //             return false;
    //         }
    //     }
    // if clock.unix_timestamp > self.maturity_date{
    //     return true;
    // }
    // return false
    // }
}

impl Distribution {
    pub fn set(&mut self, pool: Pubkey, authority: Pubkey, bump: u8) -> Result<()> {
        self.pool = pool;
        self.authority = authority;
        self.bump = bump;

        Ok(())
    }

    pub fn set_authority(&mut self, authority: Pubkey) {
        self.authority = authority;
    }

    pub fn add_rewards(&mut self, amount: u64) {
        self.rewards += amount;
    }

    pub fn sub_rewards(&mut self, amount: u64) {
        self.rewards -= amount;
    }

    pub fn add_claimed(&mut self, amount: u64) {
        self.claimed += amount;
    }

    pub fn sub_claimed(&mut self, amount: u64) {
        self.claimed -= amount;
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Investment period has not ended")]
    StartDateNotPassed,
    #[msg("Pool Start Date has passed")]
    StartDatePassed,
    #[msg("Pool maturity date has passed")]
    MaturityDatePassed,
    #[msg("Signer not authorized")]
    SignerNotAuthorized,
    #[msg("Creator not authorized")]
    CreatorNotAuthorized,
    #[msg("Pool not closed")]
    PoolNotClosed,
    #[msg("Pool closed")]
    PoolClosed,
    #[msg("Shares must be at least 1")]
    MinimumShares,
    #[msg("Deposit must be at least 3,000 USDC")]
    MinimumDeposit,
    #[msg("Seed must be divisible by shares")]
    InvalidSeedSharesRatio,
    #[msg("Exceeds available shares")]
    ExceedsAvailableShares,
    #[msg("Insufficient distribution USDC balance")]
    InsufficientDistributionUSDCBalance,
    #[msg("Seed rounds are not completed")]
    SeedRoundsNotCompleted,
    #[msg("Insufficient pool USDC balance")]
    InsufficientPoolUSDCBalance,
    #[msg("Unclaimed distribution rewards")]
    UnclaimedDistributionRewards,
    #[msg("Non-zero pool USDC balance")]
    NonZeroPoolUSDCBalance,
    #[msg("Non-zero distribution USDC balance")]
    NonZeroDistributionUSDCBalance,
}
