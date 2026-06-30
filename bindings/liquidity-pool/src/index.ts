import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




/**
 * Errors returned by the liquidity pool. Surfaced to clients via
 * `panic_with_error!`, so each maps to a stable numeric code.
 */
export const Errors = {
  1: {message:"InvalidTokenCount"},
  2: {message:"TokensNotSorted"},
  3: {message:"CapsLengthMismatch"},
  4: {message:"InvalidAmpFactor"},
  5: {message:"InvalidSwapFee"},
  6: {message:"InvalidProtocolFee"},
  7: {message:"InvalidDecimals"},
  8: {message:"InvalidCap"},
  9: {message:"AmountsLengthMismatch"},
  10: {message:"InvalidAmount"},
  11: {message:"ZeroDeposit"},
  12: {message:"FirstDepositNotFull"},
  13: {message:"MathError"},
  14: {message:"SlippageExceeded"},
  15: {message:"CapExceeded"},
  16: {message:"BalanceTooLarge"},
  17: {message:"UnknownToken"},
  18: {message:"SameToken"},
  19: {message:"TransferAmountMismatch"},
  20: {message:"DirectLpBurnDisabled"}
}

export const RoleTransferError = {
  2200: {message:"NoPendingTransfer"},
  2201: {message:"InvalidLiveUntilLedger"},
  2202: {message:"InvalidPendingAccount"},
  2203: {message:"TransferExpired"}
}

export const OwnableError = {
  2100: {message:"OwnerNotSet"},
  2101: {message:"TransferInProgress"},
  2102: {message:"OwnerAlreadySet"}
}






export const PausableError = {
  /**
   * The operation failed because the contract is paused.
   */
  1000: {message:"EnforcedPause"},
  /**
   * The operation failed because the contract is not paused.
   */
  1001: {message:"ExpectedPause"}
}





export const FungibleTokenError = {
  /**
   * Indicates an error related to the current balance of account from which
   * tokens are expected to be transferred.
   */
  100: {message:"InsufficientBalance"},
  /**
   * Indicates a failure with the allowance mechanism when a given spender
   * doesn't have enough allowance.
   */
  101: {message:"InsufficientAllowance"},
  /**
   * Indicates an invalid value for `live_until_ledger` when setting an
   * allowance.
   */
  102: {message:"InvalidLiveUntilLedger"},
  /**
   * Indicates an error when an input that must be >= 0
   */
  103: {message:"LessThanZero"},
  /**
   * Indicates overflow when adding two values
   */
  104: {message:"MathOverflow"},
  /**
   * Indicates access to uninitialized metadata
   */
  105: {message:"UnsetMetadata"},
  /**
   * Indicates that the operation would have caused `total_supply` to exceed
   * the `cap`.
   */
  106: {message:"ExceededCap"},
  /**
   * Indicates the supplied `cap` is not a valid cap value.
   */
  107: {message:"InvalidCap"},
  /**
   * Indicates the Cap was not set.
   */
  108: {message:"CapNotSet"},
  /**
   * Indicates the SAC address was not set.
   */
  109: {message:"SACNotSet"},
  /**
   * Indicates a SAC address different than expected.
   */
  110: {message:"SACAddressMismatch"},
  /**
   * Indicates a missing function parameter in the SAC contract context.
   */
  111: {message:"SACMissingFnParam"},
  /**
   * Indicates an invalid function parameter in the SAC contract context.
   */
  112: {message:"SACInvalidFnParam"},
  /**
   * The user is not allowed to perform this operation
   */
  113: {message:"UserNotAllowed"},
  /**
   * The user is blocked and cannot perform this operation
   */
  114: {message:"UserBlocked"}
}

export interface Client {
  /**
   * Construct and simulate a burn transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  burn: ({from, amount}: {from: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a name transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the name for this token.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   */
  name: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the pool: blocks deposit/withdraw/swap until unpaused.
   */
  pause: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Whether the pool is currently paused.
   */
  paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a symbol transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the symbol for this token.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   */
  symbol: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a approve transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sets the amount of tokens a `spender` is allowed to spend on behalf of
   * an `owner`. Overrides any existing allowance set between `spender` and
   * `owner`.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `owner` - The address holding the tokens.
   * * `spender` - The address authorized to spend the tokens.
   * * `amount` - The amount of tokens made available to `spender`.
   * * `live_until_ledger` - The ledger number at which the allowance
   * expires.
   * 
   * # Errors
   * 
   * * [`FungibleTokenError::InvalidLiveUntilLedger`] - Occurs when
   * attempting to set `live_until_ledger` that is less than the current
   * ledger number and greater than `0`.
   * * [`FungibleTokenError::LessThanZero`] - Occurs when `amount < 0`.
   * 
   * # Events
   * 
   * * topics - `["approve", from: Address, spender: Address]`
   * * data - `[amount: i128, live_until_ledger: u32]`
   */
  approve: ({owner, spender, amount, live_until_ledger}: {owner: string, spender: string, amount: i128, live_until_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the amount of tokens held by `account`.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   * * `account` - The address for which the balance is being queried.
   */
  balance: ({account}: {account: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deposit: ({to, amounts_in, min_lp_out}: {to: string, amounts_in: Array<i128>, min_lp_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_amp transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The current amplification *factor* (effective A), reflecting any ramp in
   * progress at the current ledger time.
   */
  get_amp: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resume a paused pool.
   */
  unpause: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a decimals transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the number of decimals used to represent amounts of this token.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   */
  decimals: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a transfer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfers `amount` of tokens from `from` to `to`.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `from` - The address holding the tokens.
   * * `to` - The address receiving the transferred tokens.
   * * `amount` - The amount of tokens to be transferred.
   * 
   * # Errors
   * 
   * * [`FungibleTokenError::InsufficientBalance`] - When attempting to
   * transfer more tokens than `from` current balance.
   * * [`FungibleTokenError::LessThanZero`] - When `amount < 0`.
   * 
   * # Events
   * 
   * * topics - `["transfer", from: Address, to: Address]`
   * * data - `[to_muxed_id: Option<u64>, amount: i128]`
   */
  transfer: ({from, to, amount}: {from: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Burn `lp_amount` shares and withdraw a proportional slice of every
   * reserve. Returns the raw amounts paid out, in token order.
   */
  withdraw: ({to, lp_amount, min_amounts_out}: {to: string, lp_amount: i128, min_amounts_out: Array<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<Array<i128>>>

  /**
   * Construct and simulate a allowance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the amount of tokens a `spender` is allowed to spend on behalf
   * of an `owner`.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `owner` - The address holding the tokens.
   * * `spender` - The address authorized to spend the tokens.
   */
  allowance: ({owner, spender}: {owner: string, spender: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a burn_from transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  burn_from: ({spender, from, amount}: {spender: string, from: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_owner transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns `Some(Address)` if ownership is set, or `None` if ownership has
   * been renounced.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   */
  get_owner: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a get_tokens transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The pool's token addresses, in token order.
   */
  get_tokens: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a get_reserves transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current reserves in raw token units, in token order.
   */
  get_reserves: (options?: MethodOptions) => Promise<AssembledTransaction<Array<i128>>>

  /**
   * Construct and simulate a set_amp_ramp transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Start (or replace) a linear amplification ramp toward `target_factor`
   * over `duration` seconds. The ramp begins from the current interpolated
   * factor, so there is no discontinuity. `duration == 0` applies it at once.
   */
  set_amp_ramp: ({target_factor, duration}: {target_factor: u32, duration: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_swap_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the swap fee (1e9 == 100%), within the configured fee range.
   */
  set_swap_fee: ({swap_fee}: {swap_fee: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a total_supply transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the total amount of tokens in circulation.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   */
  total_supply: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_token_cap transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the per-token reserve cap (in `token`'s raw units). Must be >= the
   * current reserve and within the safe math range.
   */
  set_token_cap: ({token, max_cap}: {token: string, max_cap: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a swap_exact_in transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Swap an exact `amount_in` of `token_in` for `token_out`, requiring at
   * least `min_out` back. The swap fee is charged on the output; the
   * protocol's cut of it is routed to the beneficiary and the rest stays in
   * the pool for LPs. Returns the amount of `token_out` sent to `to`.
   */
  swap_exact_in: ({to, token_in, token_out, amount_in, min_out}: {to: string, token_in: string, token_out: string, amount_in: i128, min_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a transfer_from transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfers `amount` of tokens from `from` to `to` using the
   * allowance mechanism. `amount` is then deducted from `spender`
   * allowance.
   * 
   * # Arguments
   * 
   * * `e` - Access to Soroban environment.
   * * `spender` - The address authorizing the transfer, and having its
   * allowance consumed during the transfer.
   * * `from` - The address holding the tokens which will be transferred.
   * * `to` - The address receiving the transferred tokens.
   * * `amount` - The amount of tokens to be transferred.
   * 
   * # Errors
   * 
   * * [`FungibleTokenError::InsufficientBalance`] - When attempting to
   * transfer more tokens than `from` current balance.
   * * [`FungibleTokenError::LessThanZero`] - When `amount < 0`.
   * * [`FungibleTokenError::InsufficientAllowance`] - When attempting to
   * transfer more tokens than `spender` current allowance.
   * 
   * # Events
   * 
   * * topics - `["transfer", from: Address, to: Address]`
   * * data - `[amount: i128]`
   */
  transfer_from: ({spender, from, to, amount}: {spender: string, from: string, to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_max_supply transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the cap on total LP-share supply (the pool's own token).
   */
  set_max_supply: ({max_supply}: {max_supply: i128}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a swap_exact_out transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Swap `token_in` for an exact `amount_out` of `token_out`, spending at
   * most `max_in`. Returns the amount of `token_in` taken from `to`.
   */
  swap_exact_out: ({to, token_in, token_out, amount_out, max_in}: {to: string, token_in: string, token_out: string, amount_out: i128, max_in: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the address that receives the protocol fee.
   */
  set_beneficiary: ({beneficiary}: {beneficiary: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a accept_ownership transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accepts a pending ownership transfer.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   * 
   * # Errors
   * 
   * * [`crate::role_transfer::RoleTransferError::NoPendingTransfer`] - If
   * there is no pending transfer to accept.
   * 
   * # Events
   * 
   * * topics - `["ownership_transfer_completed"]`
   * * data - `[new_owner: Address]`
   */
  accept_ownership: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Set the protocol's cut of the swap fee (1e9 == 100% of the swap fee).
   */
  set_protocol_fee: ({protocol_fee}: {protocol_fee: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a renounce_ownership transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Renounces ownership of the contract.
   * 
   * Permanently removes the owner, disabling all functions gated by
   * `#[only_owner]`.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   * 
   * # Errors
   * 
   * * [`OwnableError::TransferInProgress`] - If there is a pending ownership
   * transfer.
   * * [`OwnableError::OwnerNotSet`] - If the owner is not set.
   * 
   * # Notes
   * 
   * * Authorization for the current owner is required.
   */
  renounce_ownership: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a transfer_ownership transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initiates a 2-step ownership transfer to a new address.
   * 
   * Requires authorization from the current owner. The new owner must later
   * call `accept_ownership()` to complete the transfer.
   * 
   * # Arguments
   * 
   * * `e` - Access to the Soroban environment.
   * * `new_owner` - The proposed new owner.
   * * `live_until_ledger` - Ledger number until which the new owner can
   * accept. A value of `0` cancels any pending transfer.
   * 
   * # Errors
   * 
   * * [`OwnableError::OwnerNotSet`] - If the owner is not set.
   * * [`crate::role_transfer::RoleTransferError::NoPendingTransfer`] - If
   * trying to cancel a transfer that doesn't exist.
   * * [`crate::role_transfer::RoleTransferError::InvalidLiveUntilLedger`] -
   * If the specified ledger is in the past.
   * * [`crate::role_transfer::RoleTransferError::InvalidPendingAccount`] -
   * If the specified pending account is not the same as the provided `new`
   * address.
   * 
   * # Notes
   * 
   * * Authorization for the current owner is required.
   */
  transfer_ownership: ({new_owner, live_until_ledger}: {new_owner: string, live_until_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a withdraw_one_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Burn `lp_amount` shares and withdraw a single token. The burned share
   * lowers the stable invariant, and the selected token pays swap fees on the
   * imbalanced portion of the exit.
   */
  withdraw_one_token: ({to, lp_amount, token_out, min_amount_out}: {to: string, lp_amount: i128, token_out: string, min_amount_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {owner, tokens, amp_factor, swap_fee, protocol_fee, beneficiary, max_caps, lp_max_supply}: {owner: string, tokens: Array<string>, amp_factor: u32, swap_fee: u64, protocol_fee: u64, beneficiary: string, max_caps: Array<i128>, lp_max_supply: i128},
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy({owner, tokens, amp_factor, swap_fee, protocol_fee, beneficiary, max_caps, lp_max_supply}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAHpFcnJvcnMgcmV0dXJuZWQgYnkgdGhlIGxpcXVpZGl0eSBwb29sLiBTdXJmYWNlZCB0byBjbGllbnRzIHZpYQpgcGFuaWNfd2l0aF9lcnJvciFgLCBzbyBlYWNoIG1hcHMgdG8gYSBzdGFibGUgbnVtZXJpYyBjb2RlLgAAAAAAAAAAAAVFcnJvcgAAAAAAABQAAAAAAAAAEUludmFsaWRUb2tlbkNvdW50AAAAAAAAAQAAAAAAAAAPVG9rZW5zTm90U29ydGVkAAAAAAIAAAAAAAAAEkNhcHNMZW5ndGhNaXNtYXRjaAAAAAAAAwAAAAAAAAAQSW52YWxpZEFtcEZhY3RvcgAAAAQAAAAAAAAADkludmFsaWRTd2FwRmVlAAAAAAAFAAAAAAAAABJJbnZhbGlkUHJvdG9jb2xGZWUAAAAAAAYAAAAAAAAAD0ludmFsaWREZWNpbWFscwAAAAAHAAAAAAAAAApJbnZhbGlkQ2FwAAAAAAAIAAAAAAAAABVBbW91bnRzTGVuZ3RoTWlzbWF0Y2gAAAAAAAAJAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAACgAAAAAAAAALWmVyb0RlcG9zaXQAAAAACwAAAAAAAAATRmlyc3REZXBvc2l0Tm90RnVsbAAAAAAMAAAAAAAAAAlNYXRoRXJyb3IAAAAAAAANAAAAAAAAABBTbGlwcGFnZUV4Y2VlZGVkAAAADgAAAAAAAAALQ2FwRXhjZWVkZWQAAAAADwAAAAAAAAAPQmFsYW5jZVRvb0xhcmdlAAAAABAAAAAAAAAADFVua25vd25Ub2tlbgAAABEAAAAAAAAACVNhbWVUb2tlbgAAAAAAABIAAAAAAAAAFlRyYW5zZmVyQW1vdW50TWlzbWF0Y2gAAAAAABMAAAAAAAAAFERpcmVjdExwQnVybkRpc2FibGVkAAAAFA==",
        "AAAAAAAAAAAAAAAEYnVybgAAAAIAAAAAAAAABGZyb20AAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
        "AAAAAAAAAFVSZXR1cm5zIHRoZSBuYW1lIGZvciB0aGlzIHRva2VuLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIFNvcm9iYW4gZW52aXJvbm1lbnQuAAAAAAAABG5hbWUAAAAAAAAAAQAAABA=",
        "AAAAAAAAADxQYXVzZSB0aGUgcG9vbDogYmxvY2tzIGRlcG9zaXQvd2l0aGRyYXcvc3dhcCB1bnRpbCB1bnBhdXNlZC4AAAAFcGF1c2UAAAAAAAAAAAAAAA==",
        "AAAAAAAAACVXaGV0aGVyIHRoZSBwb29sIGlzIGN1cnJlbnRseSBwYXVzZWQuAAAAAAAABnBhdXNlZAAAAAAAAAAAAAEAAAAB",
        "AAAAAAAAAFdSZXR1cm5zIHRoZSBzeW1ib2wgZm9yIHRoaXMgdG9rZW4uCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gU29yb2JhbiBlbnZpcm9ubWVudC4AAAAABnN5bWJvbAAAAAAAAAAAAAEAAAAQ",
        "AAAAAAAAAyZTZXRzIHRoZSBhbW91bnQgb2YgdG9rZW5zIGEgYHNwZW5kZXJgIGlzIGFsbG93ZWQgdG8gc3BlbmQgb24gYmVoYWxmIG9mCmFuIGBvd25lcmAuIE92ZXJyaWRlcyBhbnkgZXhpc3RpbmcgYWxsb3dhbmNlIHNldCBiZXR3ZWVuIGBzcGVuZGVyYCBhbmQKYG93bmVyYC4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byBTb3JvYmFuIGVudmlyb25tZW50LgoqIGBvd25lcmAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSB0b2tlbnMuCiogYHNwZW5kZXJgIC0gVGhlIGFkZHJlc3MgYXV0aG9yaXplZCB0byBzcGVuZCB0aGUgdG9rZW5zLgoqIGBhbW91bnRgIC0gVGhlIGFtb3VudCBvZiB0b2tlbnMgbWFkZSBhdmFpbGFibGUgdG8gYHNwZW5kZXJgLgoqIGBsaXZlX3VudGlsX2xlZGdlcmAgLSBUaGUgbGVkZ2VyIG51bWJlciBhdCB3aGljaCB0aGUgYWxsb3dhbmNlCmV4cGlyZXMuCgojIEVycm9ycwoKKiBbYEZ1bmdpYmxlVG9rZW5FcnJvcjo6SW52YWxpZExpdmVVbnRpbExlZGdlcmBdIC0gT2NjdXJzIHdoZW4KYXR0ZW1wdGluZyB0byBzZXQgYGxpdmVfdW50aWxfbGVkZ2VyYCB0aGF0IGlzIGxlc3MgdGhhbiB0aGUgY3VycmVudApsZWRnZXIgbnVtYmVyIGFuZCBncmVhdGVyIHRoYW4gYDBgLgoqIFtgRnVuZ2libGVUb2tlbkVycm9yOjpMZXNzVGhhblplcm9gXSAtIE9jY3VycyB3aGVuIGBhbW91bnQgPCAwYC4KCiMgRXZlbnRzCgoqIHRvcGljcyAtIGBbImFwcHJvdmUiLCBmcm9tOiBBZGRyZXNzLCBzcGVuZGVyOiBBZGRyZXNzXWAKKiBkYXRhIC0gYFthbW91bnQ6IGkxMjgsIGxpdmVfdW50aWxfbGVkZ2VyOiB1MzJdYAAAAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdzcGVuZGVyAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAEAAAAAA==",
        "AAAAAAAAAKpSZXR1cm5zIHRoZSBhbW91bnQgb2YgdG9rZW5zIGhlbGQgYnkgYGFjY291bnRgLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIHRoZSBTb3JvYmFuIGVudmlyb25tZW50LgoqIGBhY2NvdW50YCAtIFRoZSBhZGRyZXNzIGZvciB3aGljaCB0aGUgYmFsYW5jZSBpcyBiZWluZyBxdWVyaWVkLgAAAAAAB2JhbGFuY2UAAAAAAQAAAAAAAAAHYWNjb3VudAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAHZGVwb3NpdAAAAAADAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAKYW1vdW50c19pbgAAAAAD6gAAAAsAAAAAAAAACm1pbl9scF9vdXQAAAAAAAsAAAABAAAACw==",
        "AAAAAAAAAG1UaGUgY3VycmVudCBhbXBsaWZpY2F0aW9uICpmYWN0b3IqIChlZmZlY3RpdmUgQSksIHJlZmxlY3RpbmcgYW55IHJhbXAgaW4KcHJvZ3Jlc3MgYXQgdGhlIGN1cnJlbnQgbGVkZ2VyIHRpbWUuAAAAAAAAB2dldF9hbXAAAAAAAAAAAAEAAAAE",
        "AAAAAAAAABVSZXN1bWUgYSBwYXVzZWQgcG9vbC4AAAAAAAAHdW5wYXVzZQAAAAAAAAAAAA==",
        "AAAAAAAAAHxSZXR1cm5zIHRoZSBudW1iZXIgb2YgZGVjaW1hbHMgdXNlZCB0byByZXByZXNlbnQgYW1vdW50cyBvZiB0aGlzIHRva2VuLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIFNvcm9iYW4gZW52aXJvbm1lbnQuAAAACGRlY2ltYWxzAAAAAAAAAAEAAAAE",
        "AAAAAAAAAi5UcmFuc2ZlcnMgYGFtb3VudGAgb2YgdG9rZW5zIGZyb20gYGZyb21gIHRvIGB0b2AuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgZnJvbWAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSB0b2tlbnMuCiogYHRvYCAtIFRoZSBhZGRyZXNzIHJlY2VpdmluZyB0aGUgdHJhbnNmZXJyZWQgdG9rZW5zLgoqIGBhbW91bnRgIC0gVGhlIGFtb3VudCBvZiB0b2tlbnMgdG8gYmUgdHJhbnNmZXJyZWQuCgojIEVycm9ycwoKKiBbYEZ1bmdpYmxlVG9rZW5FcnJvcjo6SW5zdWZmaWNpZW50QmFsYW5jZWBdIC0gV2hlbiBhdHRlbXB0aW5nIHRvCnRyYW5zZmVyIG1vcmUgdG9rZW5zIHRoYW4gYGZyb21gIGN1cnJlbnQgYmFsYW5jZS4KKiBbYEZ1bmdpYmxlVG9rZW5FcnJvcjo6TGVzc1RoYW5aZXJvYF0gLSBXaGVuIGBhbW91bnQgPCAwYC4KCiMgRXZlbnRzCgoqIHRvcGljcyAtIGBbInRyYW5zZmVyIiwgZnJvbTogQWRkcmVzcywgdG86IEFkZHJlc3NdYAoqIGRhdGEgLSBgW3RvX211eGVkX2lkOiBPcHRpb248dTY0PiwgYW1vdW50OiBpMTI4XWAAAAAAAAh0cmFuc2ZlcgAAAAMAAAAAAAAABGZyb20AAAATAAAAAAAAAAJ0bwAAAAAAFAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAH1CdXJuIGBscF9hbW91bnRgIHNoYXJlcyBhbmQgd2l0aGRyYXcgYSBwcm9wb3J0aW9uYWwgc2xpY2Ugb2YgZXZlcnkKcmVzZXJ2ZS4gUmV0dXJucyB0aGUgcmF3IGFtb3VudHMgcGFpZCBvdXQsIGluIHRva2VuIG9yZGVyLgAAAAAAAAh3aXRoZHJhdwAAAAMAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAlscF9hbW91bnQAAAAAAAALAAAAAAAAAA9taW5fYW1vdW50c19vdXQAAAAD6gAAAAsAAAABAAAD6gAAAAs=",
        "AAAAAAAAAPBSZXR1cm5zIHRoZSBhbW91bnQgb2YgdG9rZW5zIGEgYHNwZW5kZXJgIGlzIGFsbG93ZWQgdG8gc3BlbmQgb24gYmVoYWxmCm9mIGFuIGBvd25lcmAuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgb3duZXJgIC0gVGhlIGFkZHJlc3MgaG9sZGluZyB0aGUgdG9rZW5zLgoqIGBzcGVuZGVyYCAtIFRoZSBhZGRyZXNzIGF1dGhvcml6ZWQgdG8gc3BlbmQgdGhlIHRva2Vucy4AAAAJYWxsb3dhbmNlAAAAAAAAAgAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAdzcGVuZGVyAAAAABMAAAABAAAACw==",
        "AAAAAAAAAAAAAAAJYnVybl9mcm9tAAAAAAAAAwAAAAAAAAAHc3BlbmRlcgAAAAATAAAAAAAAAARmcm9tAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAA==",
        "AAAAAAAAAJBSZXR1cm5zIGBTb21lKEFkZHJlc3MpYCBpZiBvd25lcnNoaXAgaXMgc2V0LCBvciBgTm9uZWAgaWYgb3duZXJzaGlwIGhhcwpiZWVuIHJlbm91bmNlZC4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4AAAAJZ2V0X293bmVyAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAACtUaGUgcG9vbCdzIHRva2VuIGFkZHJlc3NlcywgaW4gdG9rZW4gb3JkZXIuAAAAAApnZXRfdG9rZW5zAAAAAAAAAAAAAQAAA+oAAAAT",
        "AAAAAAAAADRDdXJyZW50IHJlc2VydmVzIGluIHJhdyB0b2tlbiB1bml0cywgaW4gdG9rZW4gb3JkZXIuAAAADGdldF9yZXNlcnZlcwAAAAAAAAABAAAD6gAAAAs=",
        "AAAAAAAAANZTdGFydCAob3IgcmVwbGFjZSkgYSBsaW5lYXIgYW1wbGlmaWNhdGlvbiByYW1wIHRvd2FyZCBgdGFyZ2V0X2ZhY3RvcmAKb3ZlciBgZHVyYXRpb25gIHNlY29uZHMuIFRoZSByYW1wIGJlZ2lucyBmcm9tIHRoZSBjdXJyZW50IGludGVycG9sYXRlZApmYWN0b3IsIHNvIHRoZXJlIGlzIG5vIGRpc2NvbnRpbnVpdHkuIGBkdXJhdGlvbiA9PSAwYCBhcHBsaWVzIGl0IGF0IG9uY2UuAAAAAAAMc2V0X2FtcF9yYW1wAAAAAgAAAAAAAAANdGFyZ2V0X2ZhY3RvcgAAAAAAAAQAAAAAAAAACGR1cmF0aW9uAAAABgAAAAA=",
        "AAAAAAAAAEBTZXQgdGhlIHN3YXAgZmVlICgxZTkgPT0gMTAwJSksIHdpdGhpbiB0aGUgY29uZmlndXJlZCBmZWUgcmFuZ2UuAAAADHNldF9zd2FwX2ZlZQAAAAEAAAAAAAAACHN3YXBfZmVlAAAABgAAAAA=",
        "AAAAAAAAAGtSZXR1cm5zIHRoZSB0b3RhbCBhbW91bnQgb2YgdG9rZW5zIGluIGNpcmN1bGF0aW9uLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIHRoZSBTb3JvYmFuIGVudmlyb25tZW50LgAAAAAMdG90YWxfc3VwcGx5AAAAAAAAAAEAAAAL",
        "AAAAAAAAAZdJbml0aWFsaXplIHRoZSBwb29sLgoKKiBgdG9rZW5zYCBtdXN0IGJlIDIuLj1NQVhfVE9LRU5TIGRpc3RpbmN0IGFkZHJlc3NlcyBpbiBzdHJpY3RseQphc2NlbmRpbmcgb3JkZXIgKGNhbm9uaWNhbCwgZGVkdXAtZnJlZSkuCiogYGFtcF9mYWN0b3JgIGlzIHRoZSBhbXBsaWZpY2F0aW9uICpmYWN0b3IqIChlZmZlY3RpdmUgQSk7IHRoZSByYW1wCnN0YXJ0cyBzdGF0aWMgKGluaXRpYWwgPT0gdGFyZ2V0KS4KKiBgc3dhcF9mZWVgIC8gYHByb3RvY29sX2ZlZWAgdXNlIDFlOSA9PSAxMDAlLgoqIGBtYXhfY2Fwc2AgYXJlIHBlci10b2tlbiBjYXBzIGluIHRoYXQgdG9rZW4ncyByYXcgdW5pdHMuCiogYGxwX21heF9zdXBwbHlgIGNhcHMgdG90YWwgTFAgc2hhcmVzICh0aGUgcG9vbCdzIG93biB0b2tlbiBzdXBwbHkpLgAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAgAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAGdG9rZW5zAAAAAAPqAAAAEwAAAAAAAAAKYW1wX2ZhY3RvcgAAAAAABAAAAAAAAAAIc3dhcF9mZWUAAAAGAAAAAAAAAAxwcm90b2NvbF9mZWUAAAAGAAAAAAAAAAtiZW5lZmljaWFyeQAAAAATAAAAAAAAAAhtYXhfY2FwcwAAA+oAAAALAAAAAAAAAA1scF9tYXhfc3VwcGx5AAAAAAAACwAAAAA=",
        "AAAAAAAAAHZTZXQgdGhlIHBlci10b2tlbiByZXNlcnZlIGNhcCAoaW4gYHRva2VuYCdzIHJhdyB1bml0cykuIE11c3QgYmUgPj0gdGhlCmN1cnJlbnQgcmVzZXJ2ZSBhbmQgd2l0aGluIHRoZSBzYWZlIG1hdGggcmFuZ2UuAAAAAAANc2V0X3Rva2VuX2NhcAAAAAAAAAIAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAHbWF4X2NhcAAAAAALAAAAAA==",
        "AAAAAAAAARBTd2FwIGFuIGV4YWN0IGBhbW91bnRfaW5gIG9mIGB0b2tlbl9pbmAgZm9yIGB0b2tlbl9vdXRgLCByZXF1aXJpbmcgYXQKbGVhc3QgYG1pbl9vdXRgIGJhY2suIFRoZSBzd2FwIGZlZSBpcyBjaGFyZ2VkIG9uIHRoZSBvdXRwdXQ7IHRoZQpwcm90b2NvbCdzIGN1dCBvZiBpdCBpcyByb3V0ZWQgdG8gdGhlIGJlbmVmaWNpYXJ5IGFuZCB0aGUgcmVzdCBzdGF5cyBpbgp0aGUgcG9vbCBmb3IgTFBzLiBSZXR1cm5zIHRoZSBhbW91bnQgb2YgYHRva2VuX291dGAgc2VudCB0byBgdG9gLgAAAA1zd2FwX2V4YWN0X2luAAAAAAAABQAAAAAAAAACdG8AAAAAABMAAAAAAAAACHRva2VuX2luAAAAEwAAAAAAAAAJdG9rZW5fb3V0AAAAAAAAEwAAAAAAAAAJYW1vdW50X2luAAAAAAAACwAAAAAAAAAHbWluX291dAAAAAALAAAAAQAAAAs=",
        "AAAAAAAAA2dUcmFuc2ZlcnMgYGFtb3VudGAgb2YgdG9rZW5zIGZyb20gYGZyb21gIHRvIGB0b2AgdXNpbmcgdGhlCmFsbG93YW5jZSBtZWNoYW5pc20uIGBhbW91bnRgIGlzIHRoZW4gZGVkdWN0ZWQgZnJvbSBgc3BlbmRlcmAKYWxsb3dhbmNlLgoKIyBBcmd1bWVudHMKCiogYGVgIC0gQWNjZXNzIHRvIFNvcm9iYW4gZW52aXJvbm1lbnQuCiogYHNwZW5kZXJgIC0gVGhlIGFkZHJlc3MgYXV0aG9yaXppbmcgdGhlIHRyYW5zZmVyLCBhbmQgaGF2aW5nIGl0cwphbGxvd2FuY2UgY29uc3VtZWQgZHVyaW5nIHRoZSB0cmFuc2Zlci4KKiBgZnJvbWAgLSBUaGUgYWRkcmVzcyBob2xkaW5nIHRoZSB0b2tlbnMgd2hpY2ggd2lsbCBiZSB0cmFuc2ZlcnJlZC4KKiBgdG9gIC0gVGhlIGFkZHJlc3MgcmVjZWl2aW5nIHRoZSB0cmFuc2ZlcnJlZCB0b2tlbnMuCiogYGFtb3VudGAgLSBUaGUgYW1vdW50IG9mIHRva2VucyB0byBiZSB0cmFuc2ZlcnJlZC4KCiMgRXJyb3JzCgoqIFtgRnVuZ2libGVUb2tlbkVycm9yOjpJbnN1ZmZpY2llbnRCYWxhbmNlYF0gLSBXaGVuIGF0dGVtcHRpbmcgdG8KdHJhbnNmZXIgbW9yZSB0b2tlbnMgdGhhbiBgZnJvbWAgY3VycmVudCBiYWxhbmNlLgoqIFtgRnVuZ2libGVUb2tlbkVycm9yOjpMZXNzVGhhblplcm9gXSAtIFdoZW4gYGFtb3VudCA8IDBgLgoqIFtgRnVuZ2libGVUb2tlbkVycm9yOjpJbnN1ZmZpY2llbnRBbGxvd2FuY2VgXSAtIFdoZW4gYXR0ZW1wdGluZyB0bwp0cmFuc2ZlciBtb3JlIHRva2VucyB0aGFuIGBzcGVuZGVyYCBjdXJyZW50IGFsbG93YW5jZS4KCiMgRXZlbnRzCgoqIHRvcGljcyAtIGBbInRyYW5zZmVyIiwgZnJvbTogQWRkcmVzcywgdG86IEFkZHJlc3NdYAoqIGRhdGEgLSBgW2Ftb3VudDogaTEyOF1gAAAAAA10cmFuc2Zlcl9mcm9tAAAAAAAABAAAAAAAAAAHc3BlbmRlcgAAAAATAAAAAAAAAARmcm9tAAAAEwAAAAAAAAACdG8AAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAA=",
        "AAAAAAAAADxTZXQgdGhlIGNhcCBvbiB0b3RhbCBMUC1zaGFyZSBzdXBwbHkgKHRoZSBwb29sJ3Mgb3duIHRva2VuKS4AAAAOc2V0X21heF9zdXBwbHkAAAAAAAEAAAAAAAAACm1heF9zdXBwbHkAAAAAAAsAAAAA",
        "AAAAAAAAAIZTd2FwIGB0b2tlbl9pbmAgZm9yIGFuIGV4YWN0IGBhbW91bnRfb3V0YCBvZiBgdG9rZW5fb3V0YCwgc3BlbmRpbmcgYXQKbW9zdCBgbWF4X2luYC4gUmV0dXJucyB0aGUgYW1vdW50IG9mIGB0b2tlbl9pbmAgdGFrZW4gZnJvbSBgdG9gLgAAAAAADnN3YXBfZXhhY3Rfb3V0AAAAAAAFAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAIdG9rZW5faW4AAAATAAAAAAAAAAl0b2tlbl9vdXQAAAAAAAATAAAAAAAAAAphbW91bnRfb3V0AAAAAAALAAAAAAAAAAZtYXhfaW4AAAAAAAsAAAABAAAACw==",
        "AAAAAAAAAC9TZXQgdGhlIGFkZHJlc3MgdGhhdCByZWNlaXZlcyB0aGUgcHJvdG9jb2wgZmVlLgAAAAAPc2V0X2JlbmVmaWNpYXJ5AAAAAAEAAAAAAAAAC2JlbmVmaWNpYXJ5AAAAABMAAAAA",
        "AAAAAAAAATBBY2NlcHRzIGEgcGVuZGluZyBvd25lcnNoaXAgdHJhbnNmZXIuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gdGhlIFNvcm9iYW4gZW52aXJvbm1lbnQuCgojIEVycm9ycwoKKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRoZXJlIGlzIG5vIHBlbmRpbmcgdHJhbnNmZXIgdG8gYWNjZXB0LgoKIyBFdmVudHMKCiogdG9waWNzIC0gYFsib3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZCJdYAoqIGRhdGEgLSBgW25ld19vd25lcjogQWRkcmVzc11gAAAAEGFjY2VwdF9vd25lcnNoaXAAAAAAAAAAAA==",
        "AAAAAAAAAEVTZXQgdGhlIHByb3RvY29sJ3MgY3V0IG9mIHRoZSBzd2FwIGZlZSAoMWU5ID09IDEwMCUgb2YgdGhlIHN3YXAgZmVlKS4AAAAAAAAQc2V0X3Byb3RvY29sX2ZlZQAAAAEAAAAAAAAADHByb3RvY29sX2ZlZQAAAAYAAAAA",
        "AAAAAAAAAYVSZW5vdW5jZXMgb3duZXJzaGlwIG9mIHRoZSBjb250cmFjdC4KClBlcm1hbmVudGx5IHJlbW92ZXMgdGhlIG93bmVyLCBkaXNhYmxpbmcgYWxsIGZ1bmN0aW9ucyBnYXRlZCBieQpgI1tvbmx5X293bmVyXWAuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gdGhlIFNvcm9iYW4gZW52aXJvbm1lbnQuCgojIEVycm9ycwoKKiBbYE93bmFibGVFcnJvcjo6VHJhbnNmZXJJblByb2dyZXNzYF0gLSBJZiB0aGVyZSBpcyBhIHBlbmRpbmcgb3duZXJzaGlwCnRyYW5zZmVyLgoqIFtgT3duYWJsZUVycm9yOjpPd25lck5vdFNldGBdIC0gSWYgdGhlIG93bmVyIGlzIG5vdCBzZXQuCgojIE5vdGVzCgoqIEF1dGhvcml6YXRpb24gZm9yIHRoZSBjdXJyZW50IG93bmVyIGlzIHJlcXVpcmVkLgAAAAAAABJyZW5vdW5jZV9vd25lcnNoaXAAAAAAAAAAAAAA",
        "AAAAAAAAA45Jbml0aWF0ZXMgYSAyLXN0ZXAgb3duZXJzaGlwIHRyYW5zZmVyIHRvIGEgbmV3IGFkZHJlc3MuCgpSZXF1aXJlcyBhdXRob3JpemF0aW9uIGZyb20gdGhlIGN1cnJlbnQgb3duZXIuIFRoZSBuZXcgb3duZXIgbXVzdCBsYXRlcgpjYWxsIGBhY2NlcHRfb3duZXJzaGlwKClgIHRvIGNvbXBsZXRlIHRoZSB0cmFuc2Zlci4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgbmV3X293bmVyYCAtIFRoZSBwcm9wb3NlZCBuZXcgb3duZXIuCiogYGxpdmVfdW50aWxfbGVkZ2VyYCAtIExlZGdlciBudW1iZXIgdW50aWwgd2hpY2ggdGhlIG5ldyBvd25lciBjYW4KYWNjZXB0LiBBIHZhbHVlIG9mIGAwYCBjYW5jZWxzIGFueSBwZW5kaW5nIHRyYW5zZmVyLgoKIyBFcnJvcnMKCiogW2BPd25hYmxlRXJyb3I6Ok93bmVyTm90U2V0YF0gLSBJZiB0aGUgb3duZXIgaXMgbm90IHNldC4KKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRyeWluZyB0byBjYW5jZWwgYSB0cmFuc2ZlciB0aGF0IGRvZXNuJ3QgZXhpc3QuCiogW2BjcmF0ZTo6cm9sZV90cmFuc2Zlcjo6Um9sZVRyYW5zZmVyRXJyb3I6OkludmFsaWRMaXZlVW50aWxMZWRnZXJgXSAtCklmIHRoZSBzcGVjaWZpZWQgbGVkZ2VyIGlzIGluIHRoZSBwYXN0LgoqIFtgY3JhdGU6OnJvbGVfdHJhbnNmZXI6OlJvbGVUcmFuc2ZlckVycm9yOjpJbnZhbGlkUGVuZGluZ0FjY291bnRgXSAtCklmIHRoZSBzcGVjaWZpZWQgcGVuZGluZyBhY2NvdW50IGlzIG5vdCB0aGUgc2FtZSBhcyB0aGUgcHJvdmlkZWQgYG5ld2AKYWRkcmVzcy4KCiMgTm90ZXMKCiogQXV0aG9yaXphdGlvbiBmb3IgdGhlIGN1cnJlbnQgb3duZXIgaXMgcmVxdWlyZWQuAAAAAAASdHJhbnNmZXJfb3duZXJzaGlwAAAAAAACAAAAAAAAAAluZXdfb3duZXIAAAAAAAATAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAA",
        "AAAAAAAAAK9CdXJuIGBscF9hbW91bnRgIHNoYXJlcyBhbmQgd2l0aGRyYXcgYSBzaW5nbGUgdG9rZW4uIFRoZSBidXJuZWQgc2hhcmUKbG93ZXJzIHRoZSBzdGFibGUgaW52YXJpYW50LCBhbmQgdGhlIHNlbGVjdGVkIHRva2VuIHBheXMgc3dhcCBmZWVzIG9uIHRoZQppbWJhbGFuY2VkIHBvcnRpb24gb2YgdGhlIGV4aXQuAAAAABJ3aXRoZHJhd19vbmVfdG9rZW4AAAAAAAQAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAlscF9hbW91bnQAAAAAAAALAAAAAAAAAAl0b2tlbl9vdXQAAAAAAAATAAAAAAAAAA5taW5fYW1vdW50X291dAAAAAAACwAAAAEAAAAL",
        "AAAABAAAAAAAAAAAAAAAEVJvbGVUcmFuc2ZlckVycm9yAAAAAAAABAAAAAAAAAARTm9QZW5kaW5nVHJhbnNmZXIAAAAAAAiYAAAAAAAAABZJbnZhbGlkTGl2ZVVudGlsTGVkZ2VyAAAAAAiZAAAAAAAAABVJbnZhbGlkUGVuZGluZ0FjY291bnQAAAAAAAiaAAAAAAAAAA9UcmFuc2ZlckV4cGlyZWQAAAAImw==",
        "AAAABAAAAAAAAAAAAAAADE93bmFibGVFcnJvcgAAAAMAAAAAAAAAC093bmVyTm90U2V0AAAACDQAAAAAAAAAElRyYW5zZmVySW5Qcm9ncmVzcwAAAAAINQAAAAAAAAAPT3duZXJBbHJlYWR5U2V0AAAACDY=",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGluaXRpYXRlZC4AAAAAAAAAAAART3duZXJzaGlwVHJhbnNmZXIAAAAAAAABAAAAEm93bmVyc2hpcF90cmFuc2ZlcgAAAAAAAwAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gb3duZXJzaGlwIGlzIHJlbm91bmNlZC4AAAAAAAAAAAAST3duZXJzaGlwUmVub3VuY2VkAAAAAAABAAAAE293bmVyc2hpcF9yZW5vdW5jZWQAAAAAAQAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAC",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGNvbXBsZXRlZC4AAAAAAAAAAAAaT3duZXJzaGlwVHJhbnNmZXJDb21wbGV0ZWQAAAAAAAEAAAAcb3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZAAAAAEAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAAAAAAAAAGUGF1c2VkAAAAAAABAAAABnBhdXNlZAAAAAAAAAAAAAI=",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gdGhlIGNvbnRyYWN0IGlzIHVucGF1c2VkLgAAAAAAAAAIVW5wYXVzZWQAAAABAAAACHVucGF1c2VkAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAADVBhdXNhYmxlRXJyb3IAAAAAAAACAAAANFRoZSBvcGVyYXRpb24gZmFpbGVkIGJlY2F1c2UgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAANRW5mb3JjZWRQYXVzZQAAAAAAA+gAAAA4VGhlIG9wZXJhdGlvbiBmYWlsZWQgYmVjYXVzZSB0aGUgY29udHJhY3QgaXMgbm90IHBhdXNlZC4AAAANRXhwZWN0ZWRQYXVzZQAAAAAAA+k=",
        "AAAABQAAACVFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSBtaW50ZWQuAAAAAAAAAAAAAARNaW50AAAAAQAAAARtaW50AAAAAgAAAAAAAAACdG8AAAAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAACxFdmVudCBlbWl0dGVkIHdoZW4gYW4gYWxsb3dhbmNlIGlzIGFwcHJvdmVkLgAAAAAAAAAHQXBwcm92ZQAAAAABAAAAB2FwcHJvdmUAAAAABAAAAAAAAAAFb3duZXIAAAAAAAATAAAAAQAAAAAAAAAHc3BlbmRlcgAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAAAAAARbGl2ZV91bnRpbF9sZWRnZXIAAAAAAAAEAAAAAAAAAAI=",
        "AAAABQAAASFFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSB0cmFuc2ZlcnJlZCBiZXR3ZWVuIGFkZHJlc3NlcyB3aXRob3V0IGEKbXV4ZWQgZGVzdGluYXRpb24uCgpQZXIgU0VQLTQxLCB0aGUgZXZlbnQgZGF0YSBpcyBhIGJhcmUgYGkxMjhgIHdoZW4gbm8gbXV4ZWQgYWRkcmVzcyBpcwppbnZvbHZlZC4gVGhlIGBkYXRhX2Zvcm1hdCA9ICJzaW5nbGUtdmFsdWUiYCBhdHRyaWJ1dGUgZW5zdXJlcyB0aGUKYGFtb3VudGAgZmllbGQgaXMgc2VyaWFsaXplZCBhcyBhIGJhcmUgdmFsdWUgcmF0aGVyIHRoYW4gYSBtYXAuAAAAAAAAAAAAAAhUcmFuc2ZlcgAAAAEAAAAIdHJhbnNmZXIAAAADAAAAAAAAAARmcm9tAAAAEwAAAAEAAAAAAAAAAnRvAAAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAA=",
        "AAAABQAAAZdFdmVudCBlbWl0dGVkIHdoZW4gdG9rZW5zIGFyZSB0cmFuc2ZlcnJlZCB0byBhIG11eGVkIGFkZHJlc3MuCgpQZXIgU0VQLTQxLCB3aGVuIHRoZSBkZXN0aW5hdGlvbiBpcyBhIFtgTXV4ZWRBZGRyZXNzYF0gdGhlIGV2ZW50IGRhdGEKY2FycmllcyBib3RoIHRoZSBhbW91bnQgYW5kIHRoZSBtdXhlZCBpZGVudGlmaWVyIHNvIHRoYXQgb2ZmLWNoYWluCmNvbnN1bWVycyBjYW4gYXR0cmlidXRlIHRoZSB0cmFuc2ZlciB0byB0aGUgY29ycmVjdCBzdWItYWNjb3VudC4KClVzZXMgYHRvcGljcyA9IFsidHJhbnNmZXIiXWAgc28gdGhhdCBib3RoIFtgVHJhbnNmZXJgXSBhbmQKW2BNdXhlZFRyYW5zZmVyYF0gc2hhcmUgdGhlIHNhbWUgYCJ0cmFuc2ZlciJgIGV2ZW50IHN5bWJvbCwgYXMgcmVxdWlyZWQKYnkgU0VQLTQxLgAAAAAAAAAADU11eGVkVHJhbnNmZXIAAAAAAAABAAAACHRyYW5zZmVyAAAABAAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAJ0bwAAAAAAEwAAAAEAAAAAAAAAC3RvX211eGVkX2lkAAAAA+gAAAAGAAAAAAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAI=",
        "AAAABAAAAAAAAAAAAAAAEkZ1bmdpYmxlVG9rZW5FcnJvcgAAAAAADwAAAG5JbmRpY2F0ZXMgYW4gZXJyb3IgcmVsYXRlZCB0byB0aGUgY3VycmVudCBiYWxhbmNlIG9mIGFjY291bnQgZnJvbSB3aGljaAp0b2tlbnMgYXJlIGV4cGVjdGVkIHRvIGJlIHRyYW5zZmVycmVkLgAAAAAAE0luc3VmZmljaWVudEJhbGFuY2UAAAAAZAAAAGRJbmRpY2F0ZXMgYSBmYWlsdXJlIHdpdGggdGhlIGFsbG93YW5jZSBtZWNoYW5pc20gd2hlbiBhIGdpdmVuIHNwZW5kZXIKZG9lc24ndCBoYXZlIGVub3VnaCBhbGxvd2FuY2UuAAAAFUluc3VmZmljaWVudEFsbG93YW5jZQAAAAAAAGUAAABNSW5kaWNhdGVzIGFuIGludmFsaWQgdmFsdWUgZm9yIGBsaXZlX3VudGlsX2xlZGdlcmAgd2hlbiBzZXR0aW5nIGFuCmFsbG93YW5jZS4AAAAAAAAWSW52YWxpZExpdmVVbnRpbExlZGdlcgAAAAAAZgAAADJJbmRpY2F0ZXMgYW4gZXJyb3Igd2hlbiBhbiBpbnB1dCB0aGF0IG11c3QgYmUgPj0gMAAAAAAADExlc3NUaGFuWmVybwAAAGcAAAApSW5kaWNhdGVzIG92ZXJmbG93IHdoZW4gYWRkaW5nIHR3byB2YWx1ZXMAAAAAAAAMTWF0aE92ZXJmbG93AAAAaAAAACpJbmRpY2F0ZXMgYWNjZXNzIHRvIHVuaW5pdGlhbGl6ZWQgbWV0YWRhdGEAAAAAAA1VbnNldE1ldGFkYXRhAAAAAAAAaQAAAFJJbmRpY2F0ZXMgdGhhdCB0aGUgb3BlcmF0aW9uIHdvdWxkIGhhdmUgY2F1c2VkIGB0b3RhbF9zdXBwbHlgIHRvIGV4Y2VlZAp0aGUgYGNhcGAuAAAAAAALRXhjZWVkZWRDYXAAAAAAagAAADZJbmRpY2F0ZXMgdGhlIHN1cHBsaWVkIGBjYXBgIGlzIG5vdCBhIHZhbGlkIGNhcCB2YWx1ZS4AAAAAAApJbnZhbGlkQ2FwAAAAAABrAAAAHkluZGljYXRlcyB0aGUgQ2FwIHdhcyBub3Qgc2V0LgAAAAAACUNhcE5vdFNldAAAAAAAAGwAAAAmSW5kaWNhdGVzIHRoZSBTQUMgYWRkcmVzcyB3YXMgbm90IHNldC4AAAAAAAlTQUNOb3RTZXQAAAAAAABtAAAAMEluZGljYXRlcyBhIFNBQyBhZGRyZXNzIGRpZmZlcmVudCB0aGFuIGV4cGVjdGVkLgAAABJTQUNBZGRyZXNzTWlzbWF0Y2gAAAAAAG4AAABDSW5kaWNhdGVzIGEgbWlzc2luZyBmdW5jdGlvbiBwYXJhbWV0ZXIgaW4gdGhlIFNBQyBjb250cmFjdCBjb250ZXh0LgAAAAARU0FDTWlzc2luZ0ZuUGFyYW0AAAAAAABvAAAAREluZGljYXRlcyBhbiBpbnZhbGlkIGZ1bmN0aW9uIHBhcmFtZXRlciBpbiB0aGUgU0FDIGNvbnRyYWN0IGNvbnRleHQuAAAAEVNBQ0ludmFsaWRGblBhcmFtAAAAAAAAcAAAADFUaGUgdXNlciBpcyBub3QgYWxsb3dlZCB0byBwZXJmb3JtIHRoaXMgb3BlcmF0aW9uAAAAAAAADlVzZXJOb3RBbGxvd2VkAAAAAABxAAAANVRoZSB1c2VyIGlzIGJsb2NrZWQgYW5kIGNhbm5vdCBwZXJmb3JtIHRoaXMgb3BlcmF0aW9uAAAAAAAAC1VzZXJCbG9ja2VkAAAAAHI=" ]),
      options
    )
  }
  public readonly fromJSON = {
    burn: this.txFromJSON<null>,
        name: this.txFromJSON<string>,
        pause: this.txFromJSON<null>,
        paused: this.txFromJSON<boolean>,
        symbol: this.txFromJSON<string>,
        approve: this.txFromJSON<null>,
        balance: this.txFromJSON<i128>,
        deposit: this.txFromJSON<i128>,
        get_amp: this.txFromJSON<u32>,
        unpause: this.txFromJSON<null>,
        decimals: this.txFromJSON<u32>,
        transfer: this.txFromJSON<null>,
        withdraw: this.txFromJSON<Array<i128>>,
        allowance: this.txFromJSON<i128>,
        burn_from: this.txFromJSON<null>,
        get_owner: this.txFromJSON<Option<string>>,
        get_tokens: this.txFromJSON<Array<string>>,
        get_reserves: this.txFromJSON<Array<i128>>,
        set_amp_ramp: this.txFromJSON<null>,
        set_swap_fee: this.txFromJSON<null>,
        total_supply: this.txFromJSON<i128>,
        set_token_cap: this.txFromJSON<null>,
        swap_exact_in: this.txFromJSON<i128>,
        transfer_from: this.txFromJSON<null>,
        set_max_supply: this.txFromJSON<null>,
        swap_exact_out: this.txFromJSON<i128>,
        set_beneficiary: this.txFromJSON<null>,
        accept_ownership: this.txFromJSON<null>,
        set_protocol_fee: this.txFromJSON<null>,
        renounce_ownership: this.txFromJSON<null>,
        transfer_ownership: this.txFromJSON<null>,
        withdraw_one_token: this.txFromJSON<i128>
  }
}