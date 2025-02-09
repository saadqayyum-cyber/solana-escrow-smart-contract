use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};

declare_id!("ABkdGF6rfAVxU9zC9n961YBTLKmNAEM3waZ2936fa1f");

#[program]
pub mod crypto_solana_project {
    use super::*;

    pub fn start_subscription(
        ctx: Context<StartSubscription>,
        subscription_id: String,
        validation_threshold: u64,
    ) -> Result<()> {
        require!(
            subscription_id.len() <= 32,
            ErrorCode::SubscriptionIdTooLong
        );

        let escrow = &mut ctx.accounts.escrow_account;
        escrow.seller = ctx.accounts.seller.key();
        escrow.buyer = ctx.accounts.buyer.key();
        escrow.subscription_id = subscription_id;
        escrow.payment_count = 0;
        escrow.total_amount = 0;
        escrow.is_active = true;
        escrow.validation_threshold = validation_threshold;

        msg!(
            "Subscription started - ID: {}, Seller: {}, Buyer: {}",
            escrow.subscription_id,
            escrow.seller,
            escrow.buyer
        );
        Ok(())
    }

    pub fn make_payment(ctx: Context<MakePayment>, amount: u64) -> Result<()> {
        let is_active = ctx.accounts.escrow_account.is_active;
        let payment_count = ctx.accounts.escrow_account.payment_count;

        require!(is_active, ErrorCode::SubscriptionInactive);

        if payment_count < 5 {
            // First 5 payments go to escrow
            let transfer_ix = system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.escrow_account.key(),
                amount,
            );

            invoke(
                &transfer_ix,
                &[
                    ctx.accounts.buyer.to_account_info(),
                    ctx.accounts.escrow_account.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;

            ctx.accounts.escrow_account.total_amount = ctx
                .accounts
                .escrow_account
                .total_amount
                .checked_add(amount)
                .ok_or(ErrorCode::AmountOverflow)?;

            msg!(
                "Payment {} held in escrow. Amount: {} lamports",
                payment_count + 1,
                amount
            );
        } else {
            // Direct transfer to seller after 5 payments
            let transfer_ix = system_instruction::transfer(
                &ctx.accounts.buyer.key(),
                &ctx.accounts.seller.key(),
                amount,
            );

            invoke(
                &transfer_ix,
                &[
                    ctx.accounts.buyer.to_account_info(),
                    ctx.accounts.seller.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;

            msg!("Direct payment to seller. Amount: {} lamports", amount);
        }

        ctx.accounts.escrow_account.payment_count = payment_count
            .checked_add(1)
            .ok_or(ErrorCode::PaymentCountOverflow)?;

        Ok(())
    }

    pub fn cancel_subscription(ctx: Context<CancelSubscription>) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow_account;
        require!(escrow.is_active, ErrorCode::SubscriptionInactive);
        require!(
            ctx.accounts.buyer.key() == escrow.buyer,
            ErrorCode::UnauthorizedAccess
        );

        // Just mark as inactive - don't transfer funds
        escrow.is_active = false;

        msg!("Subscription cancelled. Funds remain in escrow until seller withdrawal");
        Ok(())
    }
    pub fn withdraw_funds(ctx: Context<WithdrawFunds>, validation_data: u64) -> Result<()> {
        let escrow = &ctx.accounts.escrow_account;
        require!(!escrow.is_active, ErrorCode::SubscriptionStillActive);
        require!(escrow.payment_count >= 5, ErrorCode::InsufficientPayments);

        // Validate the seller
        if validation_data > escrow.validation_threshold {
            // Validation failed - return funds to buyer
            let transfer_amount = escrow.total_amount;

            // Transfer funds to buyer
            **ctx.accounts.buyer.try_borrow_mut_lamports()? = ctx
                .accounts
                .buyer
                .lamports()
                .checked_add(transfer_amount)
                .ok_or(ErrorCode::AmountOverflow)?;

            **ctx
                .accounts
                .escrow_account
                .to_account_info()
                .try_borrow_mut_lamports()? = ctx
                .accounts
                .escrow_account
                .to_account_info()
                .lamports()
                .checked_sub(transfer_amount)
                .ok_or(ErrorCode::InsufficientFunds)?;

            msg!(
                "Validation failed! Funds returned to buyer: {} lamports",
                transfer_amount
            );
        } else {
            // Validation passed - transfer funds to seller
            let transfer_amount = escrow.total_amount;

            // Transfer funds to seller
            **ctx.accounts.seller.try_borrow_mut_lamports()? = ctx
                .accounts
                .seller
                .lamports()
                .checked_add(transfer_amount)
                .ok_or(ErrorCode::AmountOverflow)?;

            **ctx
                .accounts
                .escrow_account
                .to_account_info()
                .try_borrow_mut_lamports()? = ctx
                .accounts
                .escrow_account
                .to_account_info()
                .lamports()
                .checked_sub(transfer_amount)
                .ok_or(ErrorCode::InsufficientFunds)?;

            msg!(
                "Validation passed! Funds transferred to seller: {} lamports",
                transfer_amount
            );
        }

        // Close the escrow account - this will automatically return rent to buyer
        ctx.accounts
            .escrow_account
            .close(ctx.accounts.buyer.to_account_info())?;

        Ok(())
    }
}

// Contexts
#[derive(Accounts)]
#[instruction(subscription_id: String)] // Add this line to receive the parameter
pub struct StartSubscription<'info> {
    #[account(
        init,
        seeds = [
            b"escrow",
            buyer.key().as_ref(),
            seller.key().as_ref(),
            subscription_id.as_bytes()  // Now subscription_id is in scope
        ],
        bump,
        payer = buyer,
        space = 8 + EscrowAccount::LEN
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Seller address stored in escrow
    pub seller: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MakePayment<'info> {
    #[account(
        mut,
        seeds = [
            b"escrow",
            buyer.key().as_ref(),
            seller.key().as_ref(),
            escrow_account.subscription_id.as_bytes()
        ],
        bump,
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Validated in constraint
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct CancelSubscription<'info> {
    #[account(
        mut,
        seeds = [
            b"escrow",
            buyer.key().as_ref(),
            seller.key().as_ref(),
            escrow_account.subscription_id.as_bytes()
        ],
        bump,
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        constraint = seller.key() == escrow_account.seller @ ErrorCode::InvalidSeller
    )]
    /// CHECK: Validated in constraint
    pub seller: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    #[account(
        mut,
        seeds = [
            b"escrow",
            buyer.key().as_ref(),
            seller.key().as_ref(),
            escrow_account.subscription_id.as_bytes()
        ],
        bump,
        close = buyer,
        constraint = !escrow_account.is_active @ ErrorCode::SubscriptionStillActive
    )]
    pub escrow_account: Account<'info, EscrowAccount>,

    /// CHECK: Validated in constraint
    #[account(mut)]
    pub buyer: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = seller.key() == escrow_account.seller @ ErrorCode::InvalidSeller
    )]
    pub seller: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct EscrowAccount {
    pub seller: Pubkey,
    pub buyer: Pubkey,
    pub subscription_id: String,
    pub payment_count: u8,
    pub total_amount: u64,
    pub is_active: bool,
    pub validation_threshold: u64,
}

impl EscrowAccount {
    pub const LEN: usize = 32 + // seller
        32 + // buyer
        32 + // subscription_id (max length)
        1 + // payment_count
        8 + // total_amount
        1 + // is_active
        8; // validation_threshold
}

#[error_code]
pub enum ErrorCode {
    #[msg("Amount overflow")]
    AmountOverflow,
    #[msg("Payment count overflow")]
    PaymentCountOverflow,
    #[msg("Invalid seller")]
    InvalidSeller,
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Subscription is inactive")]
    SubscriptionInactive,
    #[msg("Unauthorized access")]
    UnauthorizedAccess,
    #[msg("Subscription ID too long")]
    SubscriptionIdTooLong,
    #[msg("Subscription is still active")]
    SubscriptionStillActive,
    #[msg("Minimum 5 payments required before withdrawal")]
    InsufficientPayments,
}
