use anchor_lang::prelude::*;

declare_id!("F4rUAeBfjpeV5SLQJv6BCNhSr4QrignBDxFEzTU7bPyp");

pub const MAX_MINE_UNITS: u32 = 500;
pub const DAY_SECONDS: i64 = 86_400;
pub const DAILY_HASH_GRANT: u64 = 5_000;
pub const DAILY_FORGE_GRANT: u64 = 5;
pub const IDLE_CAP_SECONDS: i64 = 86_400;

#[program]
pub mod solforge_game {
    use super::*;

    pub fn initialize_game(ctx: Context<InitializeGame>, hash_mint: Pubkey, max_hash_supply: u64) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.hash_mint = hash_mint;
        config.max_hash_supply = max_hash_supply;
        config.minted_hash = 0;
        config.player_count = 0;
        config.paused = false;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        Ok(())
    }

    pub fn initialize_player(ctx: Context<InitializePlayer>) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);

        let player = &mut ctx.accounts.player;
        player.owner = ctx.accounts.owner.key();
        player.hash = 0;
        player.total_hash = 0;
        player.forge = DAILY_FORGE_GRANT;
        player.genesis_cores = 0;
        player.auto_bot = false;
        player.overclock_bps = 0;
        player.rigs = [0; 7];
        player.relics = [0; 5];
        player.last_action_ts = Clock::get()?.unix_timestamp;
        player.last_daily_ts = 0;
        player.nonce = 0;
        player.bump = ctx.bumps.player;

        ctx.accounts.config.player_count = ctx
            .accounts
            .config
            .player_count
            .checked_add(1)
            .ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn mine(ctx: Context<PlayerAction>, units: u32) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        require!(units > 0 && units <= MAX_MINE_UNITS, SolforgeError::InvalidUnits);

        let now = Clock::get()?.unix_timestamp;
        let elapsed = now
            .checked_sub(ctx.accounts.player.last_action_ts)
            .unwrap_or(0)
            .max(0);

        // Prevent transaction spam from creating unbounded click throughput.
        require!(elapsed >= 1 || ctx.accounts.player.nonce == 0, SolforgeError::RateLimited);

        let multiplier_bps = 10_000u64
            .checked_add((ctx.accounts.player.genesis_cores as u64).checked_mul(2_500).ok_or(SolforgeError::Overflow)?)
            .and_then(|v| v.checked_add(ctx.accounts.player.overclock_bps as u64))
            .ok_or(SolforgeError::Overflow)?;

        let earned = (units as u64)
            .checked_mul(multiplier_bps)
            .and_then(|v| v.checked_div(10_000))
            .ok_or(SolforgeError::Overflow)?;

        ctx.accounts.player.hash = ctx.accounts.player.hash.checked_add(earned).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.total_hash = ctx.accounts.player.total_hash.checked_add(earned).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.last_action_ts = now;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn claim_idle(ctx: Context<PlayerAction>) -> Result<u64> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);

        let now = Clock::get()?.unix_timestamp;
        let elapsed = now
            .checked_sub(ctx.accounts.player.last_action_ts)
            .unwrap_or(0)
            .clamp(0, IDLE_CAP_SECONDS);

        let rate = rig_hash_rate(&ctx.accounts.player.rigs);
        let earned = if ctx.accounts.player.auto_bot {
            rate
                .checked_mul(elapsed as u64)
                .and_then(|v| v.checked_mul(8))
                .and_then(|v| v.checked_div(10))
                .ok_or(SolforgeError::Overflow)?
        } else {
            0
        };

        ctx.accounts.player.hash = ctx.accounts.player.hash.checked_add(earned).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.total_hash = ctx.accounts.player.total_hash.checked_add(earned).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.last_action_ts = now;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(earned)
    }

    pub fn claim_daily_grant(ctx: Context<PlayerAction>) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);

        let now = Clock::get()?.unix_timestamp;
        let last = ctx.accounts.player.last_daily_ts;
        require!(last == 0 || now.saturating_sub(last) >= DAY_SECONDS, SolforgeError::DailyAlreadyClaimed);

        ctx.accounts.player.forge = ctx.accounts.player.forge.checked_add(DAILY_FORGE_GRANT).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.hash = ctx.accounts.player.hash.checked_add(DAILY_HASH_GRANT).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.total_hash = ctx.accounts.player.total_hash.checked_add(DAILY_HASH_GRANT).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.last_daily_ts = now;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn buy_rig(ctx: Context<PlayerAction>, rig_id: u8, quantity: u32) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        require!(rig_id < 7 && quantity > 0 && quantity <= 100, SolforgeError::InvalidRig);
        let cost = rig_cost(rig_id, ctx.accounts.player.rigs[rig_id as usize], quantity)?;
        require!(ctx.accounts.player.hash >= cost, SolforgeError::InsufficientHash);

        ctx.accounts.player.hash = ctx.accounts.player.hash.checked_sub(cost).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.rigs[rig_id as usize] = ctx.accounts.player.rigs[rig_id as usize]
            .checked_add(quantity)
            .ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn synthesize_relic(ctx: Context<PlayerAction>, tier: u8) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        require!(tier < 5, SolforgeError::InvalidRelicTier);
        require!(ctx.accounts.player.forge >= 2, SolforgeError::InsufficientForge);

        ctx.accounts.player.forge = ctx.accounts.player.forge.checked_sub(2).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.relics[tier as usize] = ctx.accounts.player.relics[tier as usize]
            .checked_add(1)
            .ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn set_auto_bot(ctx: Context<PlayerAction>, enabled: bool) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        ctx.accounts.player.auto_bot = enabled;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn hard_fork(ctx: Context<PlayerAction>) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        let pending = calculate_pending_cores(&ctx.accounts.player)?;
        require!(pending > 0, SolforgeError::NoPendingCore);

        ctx.accounts.player.genesis_cores = ctx.accounts.player.genesis_cores
            .checked_add(pending as u16)
            .ok_or(SolforgeError::Overflow)?;
        ctx.accounts.player.hash = 0;
        ctx.accounts.player.total_hash = 0;
        ctx.accounts.player.rigs = [0; 7];
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }

    pub fn claim_hash(ctx: Context<ClaimHash>, amount: u64) -> Result<()> {
        require!(!ctx.accounts.config.paused, SolforgeError::Paused);
        require!(amount > 0, SolforgeError::InvalidAmount);
        require!(ctx.accounts.player.hash >= amount, SolforgeError::InsufficientHash);

        let new_minted = ctx.accounts.config
            .minted_hash
            .checked_add(amount)
            .ok_or(SolforgeError::Overflow)?;
        require!(new_minted <= ctx.accounts.config.max_hash_supply, SolforgeError::MaxSupplyExceeded);

        let seeds: &[&[u8]] = &[b"config", &[ctx.accounts.config.bump]];
        let signer = &[seeds];

        anchor_spl::token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::MintTo {
                    mint: ctx.accounts.hash_mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer,
            ),
            amount,
        )?;

        ctx.accounts.player.hash = ctx.accounts.player.hash.checked_sub(amount).ok_or(SolforgeError::Overflow)?;
        ctx.accounts.config.minted_hash = new_minted;
        ctx.accounts.player.nonce = ctx.accounts.player.nonce.checked_add(1).ok_or(SolforgeError::Overflow)?;
        Ok(())
    }
}

fn rig_hash_rate(rigs: &[u32; 7]) -> u64 {
    const RATES: [u64; 7] = [1, 8, 65, 440, 3_100, 22_000, 180_000];
    rigs.iter().enumerate().map(|(i, n)| RATES[i].saturating_mul(*n as u64)).sum()
}

fn rig_cost(rig_id: u8, owned: u32, quantity: u32) -> Result<u64> {
    const BASE: [u64; 7] = [15, 100, 850, 5_500, 38_000, 280_000, 2_200_000];
    const MULT_BPS: [u64; 7] = [11_500, 11_800, 12_000, 12_200, 12_400, 12_600, 12_800];
    let mut total = 0u64;
    let mut i = 0;
    while i < quantity {
        let exp = owned.checked_add(i).ok_or(SolforgeError::Overflow)?;
        let mut price = BASE[rig_id as usize];
        let mut j = 0;
        while j < exp.min(32) {
            price = price.checked_mul(MULT_BPS[rig_id as usize]).and_then(|v| v.checked_div(10_000)).ok_or(SolforgeError::Overflow)?;
            j += 1;
        }
        total = total.checked_add(price).ok_or(SolforgeError::Overflow)?;
        i += 1;
    }
    Ok(total)
}

fn calculate_pending_cores(player: &Player) -> Result<u32> {
    Ok((player.total_hash / 25_000) as u32)
}

#[derive(Accounts)]
pub struct InitializeGame<'info> {
    #[account(init, payer = authority, space = 8 + GameConfig::SIZE, seeds = [b"config"], bump)]
    pub config: Account<'info, GameConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump, has_one = authority)]
    pub config: Account<'info, GameConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializePlayer<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GameConfig>,
    #[account(init, payer = owner, space = 8 + Player::SIZE, seeds = [b"player", owner.key().as_ref()], bump)]
    pub player: Account<'info, Player>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlayerAction<'info> {
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GameConfig>,
    #[account(mut, seeds = [b"player", owner.key().as_ref()], bump = player.bump, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimHash<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GameConfig>,
    #[account(mut, seeds = [b"player", owner.key().as_ref()], bump = player.bump, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
    #[account(mut, address = config.hash_mint)]
    pub hash_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub destination: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

#[account]
pub struct GameConfig {
    pub authority: Pubkey,
    pub hash_mint: Pubkey,
    pub max_hash_supply: u64,
    pub minted_hash: u64,
    pub player_count: u64,
    pub paused: bool,
    pub bump: u8,
}

impl GameConfig {
    pub const SIZE: usize = 32 + 32 + 8 + 8 + 8 + 1 + 1;
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub hash: u64,
    pub total_hash: u64,
    pub forge: u64,
    pub genesis_cores: u16,
    pub auto_bot: bool,
    pub overclock_bps: u16,
    pub rigs: [u32; 7],
    pub relics: [u32; 5],
    pub last_action_ts: i64,
    pub last_daily_ts: i64,
    pub nonce: u64,
    pub bump: u8,
}

impl Player {
    pub const SIZE: usize = 32 + 8 + 8 + 8 + 2 + 1 + 2 + (4 * 7) + (4 * 5) + 8 + 8 + 8 + 1;
}

#[error_code]
pub enum SolforgeError {
    #[msg("Game is paused.")]
    Paused,
    #[msg("Invalid mining units.")]
    InvalidUnits,
    #[msg("Mining rate limit reached.")]
    RateLimited,
    #[msg("Arithmetic overflow.")]
    Overflow,
    #[msg("Daily grant already claimed.")]
    DailyAlreadyClaimed,
    #[msg("Invalid rig.")]
    InvalidRig,
    #[msg("Insufficient $FORGE.")]
    InsufficientForge,
    #[msg("Invalid relic tier.")]
    InvalidRelicTier,
    #[msg("No pending Genesis Core.")]
    NoPendingCore,
    #[msg("Invalid amount.")]
    InvalidAmount,
    #[msg("Insufficient $HASH.")]
    InsufficientHash,
    #[msg("Maximum $HASH supply reached.")]
    MaxSupplyExceeded,
}
