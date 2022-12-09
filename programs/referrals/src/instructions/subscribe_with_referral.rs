use std::borrow::Borrow;

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use mpl_token_metadata::{
    assertions::assert_owned_by,
    state::{Metadata, TokenMetadataAccount},
};

use plege::{
    cpi::accounts::CreateSubscription,
    program::Plege,
    state::{App, Subscription, Tier},
};

use crate::{
    error::ReferralError,
    state::{Referral, Referralship},
};

#[derive(Accounts)]
pub struct SubscribeWithReferral<'info> {
    pub referral: Account<'info, Referral>,
    pub referralship: Box<Account<'info, Referralship>>,
    pub referral_agent_nft_mint: Account<'info, Mint>,
    /// CHECK: we will manually deserialize and check this account
    pub referral_agent_nft_metadata: UncheckedAccount<'info>,
    pub referralship_collection_nft_mint: Account<'info, Mint>,
    /// CHECK: we will manually deserialize and check this account
    pub referral_agents_collection_nft_metadata: UncheckedAccount<'info>,
    pub treasury_mint: Account<'info, Mint>,
    pub app: Account<'info, App>,
    pub subscription: Account<'info, Subscription>,
    // The subscriber needs to sign because they need to delegate tokens to the subscription program.
    pub subscriber: Signer<'info>,
    pub subscriber_token_account: Account<'info, TokenAccount>,
    pub tier: Account<'info, Tier>,
    pub plege_program: Program<'info, Plege>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn subscribe_with_referral(ctx: Context<SubscribeWithReferral>) -> Result<()> {
    let referral = &mut ctx.accounts.referral;
    let referralship = &ctx.accounts.referralship;
    let referral_agent_nft_mint = &ctx.accounts.referral_agent_nft_mint;
    let maybe_referral_agent_nft_metadata = &ctx.accounts.referral_agent_nft_metadata;
    let maybe_referral_agents_collection_nft_metadata =
        &ctx.accounts.referral_agents_collection_nft_metadata;
    let treasury_mint = &ctx.accounts.treasury_mint;
    let subscriber_token_account = &ctx.accounts.subscriber_token_account;
    let subscriber = &ctx.accounts.subscriber;
    let subscription = &ctx.accounts.subscription;
    let app = &ctx.accounts.app;
    let tier = &ctx.accounts.tier;
    let plege_program = &ctx.accounts.plege_program;
    let token_program = &ctx.accounts.token_program;
    let system_program = &ctx.accounts.system_program;

    // make sure the treasury mint matches what's stored in the referralship.
    if treasury_mint.key() != referralship.treasury_mint.key() {
        return Err(ReferralError::InvalidTreasuryMint.into());
    }

    // make sure the collection NFT metadata is owned by the token metadata program.
    assert_owned_by(
        maybe_referral_agents_collection_nft_metadata,
        &mpl_token_metadata::id(),
    )?;

    // make sure the referral agent's NFT metadata is owned by the token metadata program.
    assert_owned_by(maybe_referral_agent_nft_metadata, &mpl_token_metadata::id())?;

    // make sure the we can deserialize the metadata.
    let collection_metadata = Metadata::from_account_info(
        maybe_referral_agents_collection_nft_metadata
            .to_account_info()
            .borrow(),
    )?;

    // make sure the referralship_collection_nft_mint matches what's stored in the referral.
    if referralship.referral_agents_collection_nft_mint.key() != collection_metadata.mint.key() {
        return Err(ReferralError::InvalidCollectionMetadata.into());
    }

    // make sure the referral agent's NFT metadata can be deserialized.
    let referral_agent_metadata =
        Metadata::from_account_info(maybe_referral_agent_nft_metadata.to_account_info().borrow())?;

    // make sure the referral agent's NFT mint matches the specified mint.
    if referral_agent_metadata.mint.key() != referral_agent_nft_mint.key() {
        return Err(ReferralError::InvalidReferralAgentMetadata.into());
    }

    // make sure the referrer's nft is a member of the collection.
    match referral_agent_metadata.collection {
        Some(collection) => {
            if collection.key != referralship.referral_agents_collection_nft_mint.key()
                || !collection.verified
            {
                return Err(ReferralError::InvalidCollection.into());
            }
        }
        None => {
            return Err(ReferralError::InvalidCollection.into());
        }
    }

    // make sure the subscriber's token account belongs to the treasury mint.
    if subscriber_token_account.mint.key() != treasury_mint.key() {
        return Err(ReferralError::InvalidSubscriberTokenAccount.into());
    }

    // make sure the subscriber's token account belongs to the subscriber.
    if subscriber_token_account.owner.key() != subscriber.key() {
        return Err(ReferralError::InvalidSubscriberTokenAccount.into());
    }

    // make sure the tier belongs to the app.
    if tier.app.key() != app.key() {
        return Err(ReferralError::InvalidTier.into());
    }

    // call the subscription program to create a subscription.
    let create_subscription_accounts = CreateSubscription {
        app: app.to_account_info(),
        tier: tier.to_account_info(),
        subscription: subscription.to_account_info(),
        subscriber: subscriber.to_account_info(),
        subscriber_ata: subscriber_token_account.to_account_info(),
        token_program: token_program.to_account_info(),
        system_program: system_program.to_account_info(),
    };

    let create_subscription_context = CpiContext::new(
        plege_program.to_account_info(),
        create_subscription_accounts,
    );

    plege::cpi::create_subscription(create_subscription_context)?;

    // set the referral account state
    referral.app = app.key();
    referral.referral_agent_nft_mint = referral_agent_nft_mint.key();
    referral.subscription = subscription.key();

    Ok(())
}