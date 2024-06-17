use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program_memory::sol_memcmp,
    pubkey::{Pubkey, PUBKEY_BYTES},
};
use anchor_spl::token::{CloseAccount, TransferChecked};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{close_account, transfer_checked, Mint, Token, TokenAccount},
};

declare_id!("A7Pc9wBYYY9ZhfMNopRha8KmWk8k46ZZfVrwoiYWFbV5");

pub fn cmp_pubkeys(a: &Pubkey, b: &Pubkey) -> bool {
    sol_memcmp(a.as_ref(), b.as_ref(), PUBKEY_BYTES) == 0
}

#[error_code]
pub enum EscrowError {
    #[msg("Account does not have correct owner Program")]
    IncorrectOwnerProgram,
    #[msg("Account is not initialized")]
    UninitializedAccount,
    #[msg("Account is already initialized")]
    InitializedAccount,
    #[msg("Auth account is not valid")]
    InvalidAuthority,
    #[msg("Auth is not signer")]
    MissingRequiredSignature,
    #[msg("Incorrect PDA Derived")]
    InvalidSeeds,
    #[msg("Amount not equal")]
    AmountNotEqual,
}

// process_instruction (program_id, accounts, instruction_data) 를 받아서 실행
// program_id, accounts (parsed) => Context 가 되고 | accounts 들은 Account Struct로 정의 되어 있는 타입에 따라 파싱된다.
// instruction_data 는 context 뒤에 따라오는 함수의 argument 로 파싱된다.

#[program]
pub mod token_escrow_anchor {

    use super::*;

    pub fn init_escrow(ctx: Context<InitEscrow>, sell_amount: u64, buy_amount: u64) -> Result<()> {
        if !ctx.accounts.authority.to_account_info().is_signer {
            //err! 는 앵커에서 지원하는 매크로 입니다.
            return err!(EscrowError::MissingRequiredSignature);
        }

        let (escrow_address, bump) = Pubkey::find_program_address(
            &[
                b"escrow",
                ctx.accounts.authority.key().as_ref(),
                ctx.accounts.sell_mint.key().as_ref(),
            ],
            &id(),
        );

        if !cmp_pubkeys(&escrow_address, &ctx.accounts.escrow.key()) {
            // 기존 native 하게 작성할 때 thiserror 크레이트를 사용해서 from 트레잇을 구현하여 ProgramError로 만든것
            return Err(EscrowError::InvalidSeeds.into());
        }

        if ctx.bumps.escrow != bump {
            return Err(EscrowError::InvalidSeeds.into());
        }

        ctx.accounts.escrow.is_initialized = true;
        ctx.accounts.escrow.authority = ctx.accounts.authority.key();
        ctx.accounts.escrow.sell_mint = ctx.accounts.sell_mint.key();
        ctx.accounts.escrow.buy_mint = ctx.accounts.buy_mint.key();
        ctx.accounts.escrow.receive_ata = ctx.accounts.authority_buy_ata.key();
        ctx.accounts.escrow.sell_amount = sell_amount;
        ctx.accounts.escrow.buy_amount = buy_amount;
        ctx.accounts.escrow.bump = ctx.bumps.escrow;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.authority_sell_ata.to_account_info(),
            mint: ctx.accounts.sell_mint.to_account_info(),
            to: ctx.accounts.escrow_ata.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        let cpi_context =
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);

        transfer_checked(cpi_context, sell_amount, ctx.accounts.sell_mint.decimals)?;

        Ok(())
    }

    pub fn exchange(ctx: Context<Exchange>, sell_amount: u64, buy_amount: u64) -> Result<()> {
        if ctx.accounts.taker.is_signer == false {
            return Err(EscrowError::MissingRequiredSignature.into());
        }

        let (escrow_address, bump) = Pubkey::find_program_address(
            &[
                b"escrow",
                ctx.accounts.authority.key().as_ref(),
                ctx.accounts.taker_buy_mint.key().as_ref(),
            ],
            &id(),
        );

        if !cmp_pubkeys(&escrow_address, &ctx.accounts.escrow.key()) {
            return Err(EscrowError::InvalidSeeds.into());
        }
        if ctx.accounts.escrow.bump != bump {
            return Err(EscrowError::InvalidSeeds.into());
        }

        if buy_amount != ctx.accounts.escrow.sell_amount {
            return Err(EscrowError::AmountNotEqual.into());
        }

        if buy_amount != ctx.accounts.escrow_ata.amount {
            return Err(EscrowError::AmountNotEqual.into());
        }

        if sell_amount != ctx.accounts.escrow.buy_amount {
            return Err(EscrowError::AmountNotEqual.into());
        }

        let signer_seed: &[&[&[u8]]] = &[&[
            b"escrow",
            ctx.accounts.escrow.authority.as_ref(),
            ctx.accounts.escrow.sell_mint.as_ref(),
            &[ctx.accounts.escrow.bump],
        ]];

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.escrow_ata.to_account_info(),
            mint: ctx.accounts.taker_buy_mint.to_account_info(),
            to: ctx.accounts.taker_buy_ata.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seed,
        );

        transfer_checked(
            cpi_context,
            buy_amount,
            ctx.accounts.taker_buy_mint.decimals,
        )?;

        let cpi_accounts = CloseAccount {
            account: ctx.accounts.escrow_ata.to_account_info(),
            destination: ctx.accounts.authority.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seed,
        );

        close_account(cpi_context)?;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.taker_sell_ata.to_account_info(),
            mint: ctx.accounts.taker_sell_mint.to_account_info(),
            to: ctx.accounts.receive_ata.to_account_info(),
            authority: ctx.accounts.taker.to_account_info(),
        };

        let cpi_context =
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);

        transfer_checked(
            cpi_context,
            sell_amount,
            ctx.accounts.taker_sell_mint.decimals,
        )?;

        ctx.accounts.taker_sell_ata.reload()?;

        ctx.accounts
            .escrow
            .close(ctx.accounts.authority.to_account_info())?;

        Ok(())
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct Escrow {
    pub is_initialized: bool,
    pub authority: Pubkey,
    pub sell_mint: Pubkey,
    pub buy_mint: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub receive_ata: Pubkey,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitEscrow<'info> {
    pub sell_mint: Account<'info, Mint>,
    pub buy_mint: Account<'info, Mint>,
    #[account(mut, signer)]
    pub authority: Signer<'info>,
    #[account(mut,
        associated_token::mint = sell_mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program )]
    pub authority_sell_ata: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,
        payer=authority,
        associated_token::mint = buy_mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program )]
    pub authority_buy_ata: Box<Account<'info, TokenAccount>>,
    #[account(init, payer=authority, seeds=[b"escrow", authority.key().as_ref(), sell_mint.key().as_ref()], bump, space=8+Escrow::INIT_SPACE)]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(init,
        payer=authority,
        associated_token::mint = sell_mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program )]
    pub escrow_ata: Account<'info, TokenAccount>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

//discriminator : 8 bytes hash
// instruction, state 등을 구별하는데 사용
// 기존에 instruction 이 enum 형태로 1byte로 구분하던 것을 8byte로 구분
// 기존에 state 가 무엇인지 구별하려면 앞에 1byte 의 enum을 사용했던걸 8byte로 구분

#[derive(Accounts)]
pub struct Exchange<'info> {
    ///CHECK : creator of escrow
    #[account(mut)]
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub taker: Signer<'info>,
    pub taker_sell_mint: Box<Account<'info, Mint>>,
    pub taker_buy_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        associated_token::mint = taker_sell_mint,
        associated_token::authority = taker,
        associated_token::token_program = token_program )]
    pub taker_sell_ata: Account<'info, TokenAccount>,
    #[account(mut,
        associated_token::mint = taker_buy_mint,
        associated_token::authority = taker,
        associated_token::token_program = token_program )]
    pub taker_buy_ata: Account<'info, TokenAccount>,
    #[account(init_if_needed,
        payer=taker,
        associated_token::mint = taker_sell_mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program )]
    pub receive_ata: Account<'info, TokenAccount>,
    #[account(mut,
        seeds=[b"escrow", authority.key().as_ref(), taker_buy_mint.key().as_ref()], bump = {msg!("input escrow key : {}", &escrow.key()); escrow.bump})]
    pub escrow: Account<'info, Escrow>,
    #[account(mut, associated_token::mint = taker_buy_mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program )]
    pub escrow_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
