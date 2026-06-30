use soroban_sdk::contracterror;

/// Errors returned by the liquidity pool. Surfaced to clients via
/// `panic_with_error!`, so each maps to a stable numeric code.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // --- initialization ---
    InvalidTokenCount = 1,  // not in 2..=MAX_TOKENS
    TokensNotSorted = 2,    // tokens not strictly ascending (also catches duplicates)
    CapsLengthMismatch = 3, // max_caps length != tokens length
    InvalidAmpFactor = 4,   // amp factor out of [MIN_AMP, MAX_AMP]
    InvalidSwapFee = 5,     // swap fee out of [MIN_SWAP_FEE, MAX_SWAP_FEE]
    InvalidProtocolFee = 6, // protocol fee fraction > ONE (100%)
    InvalidDecimals = 7,    // token decimals too large to scale
    InvalidCap = 8,         // per-token cap negative or exceeds MAX_SAFE_BALANCE

    // --- deposit / withdraw ---
    AmountsLengthMismatch = 9, // amounts vector length != tokens length
    InvalidAmount = 10,        // negative, or too large to scale into u64
    ZeroDeposit = 11,          // no positive amount supplied
    FirstDepositNotFull = 12,  // first deposit must fund every token
    MathError = 13,            // a math routine returned None (no convergence / bad input)
    SlippageExceeded = 14,     // output below / input above the caller's limit
    CapExceeded = 15,          // a reserve would exceed its per-token cap
    BalanceTooLarge = 16,      // an i128 value did not fit in the u64 math domain

    // --- swap ---
    UnknownToken = 17,           // an address is not one of the pool's tokens
    SameToken = 18,              // token_in == token_out
    TransferAmountMismatch = 19, // token balance delta differed from requested transfer

    // --- LP shares ---
    DirectLpBurnDisabled = 20, // LP exits must use withdraw so reserves are updated
}
