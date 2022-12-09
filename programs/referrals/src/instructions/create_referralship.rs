use std::borrow::Borrow;

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use mpl_token_metadata::{
    assertions::assert_owned_by,
    state::{CollectionDetails, Metadata, TokenMetadataAccount},
};
use plege::{program::Plege, state::App};

use crate::{
    error::ReferralError,
    state::{AddressWithWeight, Referralship, Splits8, APP, REFERRALSHIP},
};

#[derive(Accounts)]
#[instruction(app_id: u8)]
pub struct CreateReferralship<'info> {
    #[account(
        init,
        payer = app_authority,
        space = 1000,
        seeds = [REFERRALSHIP.as_bytes(), app.key().as_ref()],
        bump
    )]
    pub referralship: Account<'info, Referralship>,
    #[account(
        seeds = [APP.as_bytes(), app_authority.key().as_ref(), app_id.to_be_bytes().as_ref()],
        seeds::program = plege_program.key(),
        bump,
    )]
    pub app: Account<'info, App>,
    pub treasury_mint: Account<'info, Mint>,
    pub referral_agents_collection_nft_mint: Account<'info, Mint>,
    /// CHECK: we will manually deserialize and check this account
    pub referral_agents_collection_nft_metadata: UncheckedAccount<'info>,
    #[account(mut)]
    #[account(constraint = app_authority.key() == app.auth.key() @ ReferralError::InvalidAppAuthority)]
    pub app_authority: Signer<'info>,
    pub plege_program: Program<'info, Plege>,
    pub system_program: Program<'info, System>,
}

pub fn create_referralship(
    ctx: Context<CreateReferralship>,
    app_id: u8,
    referral_agent_split: u8,
    splits: Vec<AddressWithWeight>,
) -> Result<()> {
    let app = &ctx.accounts.app;
    let app_authority = &ctx.accounts.app_authority;
    let referralship = &mut ctx.accounts.referralship;
    let referral_agents_collection_nft_mint = &ctx.accounts.referral_agents_collection_nft_mint;
    let maybe_referral_agents_collection_nft_metadata =
        &ctx.accounts.referral_agents_collection_nft_metadata;
    let treasury_mint = &ctx.accounts.treasury_mint;

    if treasury_mint.key() == referral_agents_collection_nft_mint.key() {
        return Err(ReferralError::InvalidTreasuryMint.into());
    }

    if splits.len() > 7 {
        return Err(ReferralError::TooManySplitsProvided.into());
    }

    let mut splits_iter = splits.into_iter();

    // iterate over the addresses provided as splits, if there are fewer than 7, fill the rest with
    // None.
    let splits = Splits8 {
        referral_agent: referral_agent_split,
        slot_1: splits_iter.next(),
        slot_2: splits_iter.next(),
        slot_3: splits_iter.next(),
        slot_4: splits_iter.next(),
        slot_5: splits_iter.next(),
        slot_6: splits_iter.next(),
        slot_7: splits_iter.next(),
    };

    // make sure the splits add up to 100
    splits.validate_weights()?;

    // make sure the metadata belongs to the metaplex token metadata program;
    assert_owned_by(
        maybe_referral_agents_collection_nft_metadata
            .to_account_info()
            .borrow(),
        &mpl_token_metadata::id(),
    )?;

    // make sure the metadata can be deserialized.
    let collection_metadata = Metadata::from_account_info(
        maybe_referral_agents_collection_nft_metadata
            .to_account_info()
            .borrow(),
    )?;

    // make sure the mint account matches the mint in the metadata
    if collection_metadata.mint != referral_agents_collection_nft_mint.key() {
        return Err(ReferralError::CollectionMetadataMintMismatch.into());
    }

    // make sure the metadata is for a collection NFT
    if collection_metadata.collection_details.is_none() {
        return Err(ReferralError::InvalidCollectionMetadata.into());
    }

    // set referralship state
    referralship.app = app.key();
    referralship.app_id = app_id;
    referralship.referral_agents_collection_nft_mint = referral_agents_collection_nft_mint.key();
    referralship.splits = splits;
    referralship.treasury_mint = treasury_mint.key();

    Ok(())
}