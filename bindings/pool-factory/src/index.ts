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




export const Errors = {
  1: {message:"NextPoolIdOverflow"},
  2: {message:"InvalidProtocolFee"},
  3: {message:"PoolNotRegistered"},
  4: {message:"OwnershipRenunciationDisabled"},
  5: {message:"EmptySwapPath"},
  6: {message:"InvalidUpgradeOperator"},
  7: {message:"DepegGuardNotSet"},
  8: {message:"InvalidDepegGuard"},
  9: {message:"StalePrice"},
  10: {message:"PriceUnavailable"},
  11: {message:"PriceOverflow"}
}















/**
 * One leg of an exact-input routed swap.
 * 
 * `token_in` is implicit: it is the router's `token_in` for the first hop and
 * the previous hop's `token_out` thereafter.
 */
export interface SwapHop {
  pool_id: u32;
  token_out: string;
}


/**
 * One asset on one SEP-40 oracle.
 */
export interface PriceFeed {
  asset: OracleAsset;
  oracle: string;
}


export interface DepegCheck {
  /**
 * Its smallest deviation across the confirmation readings, in bps.
 */
deviation_bps: u32;
  /**
 * Its latest price, normalized to 14 decimals.
 */
price: i128;
  status: DepegStatus;
  /**
 * The token furthest from the peg (`None` when `AlreadyPaused`).
 */
token: Option<string>;
}

export type DepegStatus = {tag: "Healthy", values: void} | {tag: "Paused", values: void} | {tag: "AlreadyPaused", values: void};

/**
 * SEP-40 asset identifier: a Stellar asset contract or an off-chain symbol.
 */
export type OracleAsset = {tag: "Stellar", values: readonly [string]} | {tag: "Other", values: readonly [string]};


/**
 * Per-pool depeg protection, set by router governance. Pools without one are
 * unprotected.
 */
export interface DepegGuardConfig {
  /**
 * How many of the most recent readings must all breach the threshold.
 * `1..=10`.
 */
confirmations: u32;
  /**
 * Pause when a token is strictly further than this from the peg, in
 * basis points of the peg. `1..=10_000`.
 */
max_deviation_bps: u32;
  /**
 * Reject the latest reading when it is older than this, in seconds.
 * `1..=86_400`.
 */
max_price_age: u64;
  /**
 * Target price, normalized to 14 decimals (1.0 == 100_000_000_000_000).
 */
peg_price: i128;
  /**
 * One source per pool token, in the pool's token order.
 */
sources: Array<TokenPriceSource>;
}


/**
 * How to price one pool token in the peg's unit: `feed`, multiplied in order
 * by each of `quotes`. With no quotes, `feed` must already be denominated in
 * the peg's unit. With one, for example, a Stellar-DEX feed priced in USDC is
 * multiplied by USDC's USD price. Every quote must publish at the same
 * timestamps as `feed`. At most 2 quotes.
 */
export interface TokenPriceSource {
  feed: PriceFeed;
  quotes: Array<PriceFeed>;
}

/**
 * Determines whether the pool's amplification factor is permanently fixed or
 * delegated to the immutable protocol controller.
 */
export type AmpControl = {tag: "Locked", values: void} | {tag: "ProtocolManaged", values: void};

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



export interface Client {
  /**
   * Construct and simulate a pool_at transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pool_at: ({id}: {id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a upgrade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade: ({new_wasm_hash, operator}: {new_wasm_hash: Buffer, operator: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
   * Construct and simulate a pause_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause_pool: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a check_depeg transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  check_depeg: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<DepegCheck>>

  /**
   * Construct and simulate a create_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_pool: ({creator, tokens, amp_factor, amp_control, swap_fee, max_caps, lp_max_supply, lp_name, lp_symbol}: {creator: string, tokens: Array<string>, amp_factor: u32, amp_control: AmpControl, swap_fee: u64, max_caps: Array<i128>, lp_max_supply: i128, lp_name: string, lp_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a next_pool_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  next_pool_id: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a unpause_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause_pool: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a upgrade_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  upgrade_pool: ({pool_id, new_wasm_hash}: {pool_id: u32, new_wasm_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a swap_exact_in transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  swap_exact_in: ({to, token_in, path, amount_in, min_out}: {to: string, token_in: string, path: Array<SwapHop>, amount_in: i128, min_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_depeg_guard transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_depeg_guard: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<DepegGuardConfig>>>

  /**
   * Construct and simulate a set_depeg_guard transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_depeg_guard: ({pool_id, config}: {pool_id: u32, config: DepegGuardConfig}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
   * Construct and simulate a set_pool_amp_ramp transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_pool_amp_ramp: ({pool_id, target_factor, duration}: {pool_id: u32, target_factor: u32, duration: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_pool_wasm_hash: (options?: MethodOptions) => Promise<AssembledTransaction<Buffer>>

  /**
   * Construct and simulate a remove_depeg_guard transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  remove_depeg_guard: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a renounce_ownership transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  renounce_ownership: (options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_pool_wasm_hash: ({new_hash}: {new_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a stop_pool_amp_ramp transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  stop_pool_amp_ramp: ({pool_id}: {pool_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
   * Construct and simulate a set_pool_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_pool_beneficiary: ({pool_id, new_beneficiary}: {pool_id: u32, new_beneficiary: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a set_pool_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_pool_protocol_fee: ({pool_id, new_fee}: {pool_id: u32, new_fee: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_default_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_default_protocol_fee: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a set_default_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_default_protocol_fee: ({new_fee}: {new_fee: u64}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_default_protocol_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_default_protocol_beneficiary: (options?: MethodOptions) => Promise<AssembledTransaction<string>>

  /**
   * Construct and simulate a set_default_protocol_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_default_protocol_beneficiary: ({new_beneficiary}: {new_beneficiary: string}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {owner, pool_wasm_hash, default_protocol_fee, default_protocol_beneficiary}: {owner: string, pool_wasm_hash: Buffer, default_protocol_fee: u64, default_protocol_beneficiary: string},
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
    return ContractClient.deploy({owner, pool_wasm_hash, default_protocol_fee, default_protocol_beneficiary}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAACwAAAAAAAAASTmV4dFBvb2xJZE92ZXJmbG93AAAAAAABAAAAAAAAABJJbnZhbGlkUHJvdG9jb2xGZWUAAAAAAAIAAAAAAAAAEVBvb2xOb3RSZWdpc3RlcmVkAAAAAAAAAwAAAAAAAAAdT3duZXJzaGlwUmVudW5jaWF0aW9uRGlzYWJsZWQAAAAAAAAEAAAAAAAAAA1FbXB0eVN3YXBQYXRoAAAAAAAABQAAAAAAAAAWSW52YWxpZFVwZ3JhZGVPcGVyYXRvcgAAAAAABgAAAAAAAAAQRGVwZWdHdWFyZE5vdFNldAAAAAcAAAAAAAAAEUludmFsaWREZXBlZ0d1YXJkAAAAAAAACAAAAAAAAAAKU3RhbGVQcmljZQAAAAAACQAAAAAAAAAQUHJpY2VVbmF2YWlsYWJsZQAAAAoAAAAAAAAADVByaWNlT3ZlcmZsb3cAAAAAAAAL",
        "AAAABQAAAAAAAAAAAAAAC1Bvb2xDcmVhdGVkAAAAAAEAAAAMcG9vbF9jcmVhdGVkAAAABAAAAAAAAAAEcG9vbAAAABMAAAABAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAAJpZAAAAAAABAAAAAAAAAAAAAAAC2FtcF9jb250cm9sAAAAB9AAAAAKQW1wQ29udHJvbAAAAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADFBvb2xVcGdyYWRlZAAAAAEAAAANcG9vbF91cGdyYWRlZAAAAAAAAAIAAAAAAAAABHBvb2wAAAATAAAAAQAAAAAAAAANbmV3X3dhc21faGFzaAAAAAAAA+4AAAAgAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADURlcGVnR3VhcmRTZXQAAAAAAAABAAAAD2RlcGVnX2d1YXJkX3NldAAAAAACAAAAAAAAAARwb29sAAAAEwAAAAEAAAAAAAAABmNvbmZpZwAAAAAH0AAAABBEZXBlZ0d1YXJkQ29uZmlnAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAADlBvb2xBbXBSYW1wU2V0AAAAAAABAAAAEXBvb2xfYW1wX3JhbXBfc2V0AAAAAAAAAwAAAAAAAAAEcG9vbAAAABMAAAABAAAAAAAAAA10YXJnZXRfZmFjdG9yAAAAAAAABAAAAAAAAAAAAAAACGR1cmF0aW9uAAAABgAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAD1Bvb2xXYXNtVXBkYXRlZAAAAAABAAAAEXBvb2xfd2FzbV91cGRhdGVkAAAAAAAAAgAAAAAAAAAIb2xkX2hhc2gAAAPuAAAAIAAAAAAAAAAAAAAACG5ld19oYXNoAAAD7gAAACAAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAAEFBvb2xQYXVzZVVwZGF0ZWQAAAABAAAAEnBvb2xfcGF1c2VfdXBkYXRlZAAAAAAAAgAAAAAAAAAEcG9vbAAAABMAAAABAAAAAAAAAAZwYXVzZWQAAAAAAAEAAAAAAAAAAg==",
        "AAAAAAAAAAAAAAAHcG9vbF9hdAAAAAABAAAAAAAAAAJpZAAAAAAABAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAAAAAAAHdXBncmFkZQAAAAACAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAAAAAACG9wZXJhdG9yAAAAEwAAAAA=",
        "AAAABQAAAAAAAAAAAAAAEURlcGVnR3VhcmRSZW1vdmVkAAAAAAAAAQAAABNkZXBlZ19ndWFyZF9yZW1vdmVkAAAAAAEAAAAAAAAABHBvb2wAAAATAAAAAQAAAAI=",
        "AAAABQAAAAAAAAAAAAAAElBvb2xBbXBSYW1wU3RvcHBlZAAAAAAAAQAAABVwb29sX2FtcF9yYW1wX3N0b3BwZWQAAAAAAAABAAAAAAAAAARwb29sAAAAEwAAAAEAAAAC",
        "AAAAAAAAAJBSZXR1cm5zIGBTb21lKEFkZHJlc3MpYCBpZiBvd25lcnNoaXAgaXMgc2V0LCBvciBgTm9uZWAgaWYgb3duZXJzaGlwIGhhcwpiZWVuIHJlbm91bmNlZC4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4AAAAJZ2V0X293bmVyAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAABQAAAEdFbWl0dGVkIHdoZW4gYGNoZWNrX2RlcGVnYCBwYXVzZXMgYSBwb29sLCBhbG9uZ3NpZGUgYFBvb2xQYXVzZVVwZGF0ZWRgLgAAAAAAAAAAE0RlcGVnUGF1c2VUcmlnZ2VyZWQAAAAAAQAAABVkZXBlZ19wYXVzZV90cmlnZ2VyZWQAAAAAAAAEAAAAAAAAAARwb29sAAAAEwAAAAEAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAAAAAADWRldmlhdGlvbl9icHMAAAAAAAAEAAAAAAAAAAI=",
        "AAAAAAAAAAAAAAAKcGF1c2VfcG9vbAAAAAAAAQAAAAAAAAAHcG9vbF9pZAAAAAAEAAAAAA==",
        "AAAAAAAAAAAAAAALY2hlY2tfZGVwZWcAAAAAAQAAAAAAAAAHcG9vbF9pZAAAAAAEAAAAAQAAB9AAAAAKRGVwZWdDaGVjawAA",
        "AAAAAAAAAAAAAAALY3JlYXRlX3Bvb2wAAAAACQAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZ0b2tlbnMAAAAAA+oAAAATAAAAAAAAAAphbXBfZmFjdG9yAAAAAAAEAAAAAAAAAAthbXBfY29udHJvbAAAAAfQAAAACkFtcENvbnRyb2wAAAAAAAAAAAAIc3dhcF9mZWUAAAAGAAAAAAAAAAhtYXhfY2FwcwAAA+oAAAALAAAAAAAAAA1scF9tYXhfc3VwcGx5AAAAAAAACwAAAAAAAAAHbHBfbmFtZQAAAAAQAAAAAAAAAAlscF9zeW1ib2wAAAAAAAAQAAAAAQAAABM=",
        "AAAAAAAAAAAAAAAMbmV4dF9wb29sX2lkAAAAAAAAAAEAAAAE",
        "AAAAAAAAAAAAAAAMdW5wYXVzZV9wb29sAAAAAQAAAAAAAAAHcG9vbF9pZAAAAAAEAAAAAA==",
        "AAAAAAAAAAAAAAAMdXBncmFkZV9wb29sAAAAAgAAAAAAAAAHcG9vbF9pZAAAAAAEAAAAAAAAAA1uZXdfd2FzbV9oYXNoAAAAAAAD7gAAACAAAAAA",
        "AAAABQAAAAAAAAAAAAAAFlBvb2xCZW5lZmljaWFyeVVwZGF0ZWQAAAAAAAEAAAAYcG9vbF9iZW5lZmljaWFyeV91cGRhdGVkAAAAAgAAAAAAAAAEcG9vbAAAABMAAAABAAAAAAAAAA9uZXdfYmVuZWZpY2lhcnkAAAAAEwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAFlBvb2xQcm90b2NvbEZlZVVwZGF0ZWQAAAAAAAEAAAAZcG9vbF9wcm90b2NvbF9mZWVfdXBkYXRlZAAAAAAAAAIAAAAAAAAABHBvb2wAAAATAAAAAQAAAAAAAAAHbmV3X2ZlZQAAAAAGAAAAAAAAAAI=",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAQAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAOcG9vbF93YXNtX2hhc2gAAAAAA+4AAAAgAAAAAAAAABRkZWZhdWx0X3Byb3RvY29sX2ZlZQAAAAYAAAAAAAAAHGRlZmF1bHRfcHJvdG9jb2xfYmVuZWZpY2lhcnkAAAATAAAAAA==",
        "AAAAAAAAAAAAAAANc3dhcF9leGFjdF9pbgAAAAAAAAUAAAAAAAAAAnRvAAAAAAATAAAAAAAAAAh0b2tlbl9pbgAAABMAAAAAAAAABHBhdGgAAAPqAAAH0AAAAAdTd2FwSG9wAAAAAAAAAAAJYW1vdW50X2luAAAAAAAACwAAAAAAAAAHbWluX291dAAAAAALAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAPZ2V0X2RlcGVnX2d1YXJkAAAAAAEAAAAAAAAAB3Bvb2xfaWQAAAAABAAAAAEAAAPoAAAH0AAAABBEZXBlZ0d1YXJkQ29uZmln",
        "AAAAAAAAAAAAAAAPc2V0X2RlcGVnX2d1YXJkAAAAAAIAAAAAAAAAB3Bvb2xfaWQAAAAABAAAAAAAAAAGY29uZmlnAAAAAAfQAAAAEERlcGVnR3VhcmRDb25maWcAAAAA",
        "AAAABQAAAAAAAAAAAAAAGURlZmF1bHRCZW5lZmljaWFyeVVwZGF0ZWQAAAAAAAABAAAAG2RlZmF1bHRfYmVuZWZpY2lhcnlfdXBkYXRlZAAAAAACAAAAAAAAAA9vbGRfYmVuZWZpY2lhcnkAAAAAEwAAAAAAAAAAAAAAD25ld19iZW5lZmljaWFyeQAAAAATAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAGURlZmF1bHRQcm90b2NvbEZlZVVwZGF0ZWQAAAAAAAABAAAAHGRlZmF1bHRfcHJvdG9jb2xfZmVlX3VwZGF0ZWQAAAACAAAAAAAAAAdvbGRfZmVlAAAAAAYAAAAAAAAAAAAAAAduZXdfZmVlAAAAAAYAAAAAAAAAAg==",
        "AAAAAAAAATBBY2NlcHRzIGEgcGVuZGluZyBvd25lcnNoaXAgdHJhbnNmZXIuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gdGhlIFNvcm9iYW4gZW52aXJvbm1lbnQuCgojIEVycm9ycwoKKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRoZXJlIGlzIG5vIHBlbmRpbmcgdHJhbnNmZXIgdG8gYWNjZXB0LgoKIyBFdmVudHMKCiogdG9waWNzIC0gYFsib3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZCJdYAoqIGRhdGEgLSBgW25ld19vd25lcjogQWRkcmVzc11gAAAAEGFjY2VwdF9vd25lcnNoaXAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAARc2V0X3Bvb2xfYW1wX3JhbXAAAAAAAAADAAAAAAAAAAdwb29sX2lkAAAAAAQAAAAAAAAADXRhcmdldF9mYWN0b3IAAAAAAAAEAAAAAAAAAAhkdXJhdGlvbgAAAAYAAAAA",
        "AAAAAAAAAAAAAAASZ2V0X3Bvb2xfd2FzbV9oYXNoAAAAAAAAAAAAAQAAA+4AAAAg",
        "AAAAAAAAAAAAAAAScmVtb3ZlX2RlcGVnX2d1YXJkAAAAAAABAAAAAAAAAAdwb29sX2lkAAAAAAQAAAAA",
        "AAAAAAAAAAAAAAAScmVub3VuY2Vfb3duZXJzaGlwAAAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAASc2V0X3Bvb2xfd2FzbV9oYXNoAAAAAAABAAAAAAAAAAhuZXdfaGFzaAAAA+4AAAAgAAAAAA==",
        "AAAAAAAAAAAAAAASc3RvcF9wb29sX2FtcF9yYW1wAAAAAAABAAAAAAAAAAdwb29sX2lkAAAAAAQAAAAA",
        "AAAAAAAAA45Jbml0aWF0ZXMgYSAyLXN0ZXAgb3duZXJzaGlwIHRyYW5zZmVyIHRvIGEgbmV3IGFkZHJlc3MuCgpSZXF1aXJlcyBhdXRob3JpemF0aW9uIGZyb20gdGhlIGN1cnJlbnQgb3duZXIuIFRoZSBuZXcgb3duZXIgbXVzdCBsYXRlcgpjYWxsIGBhY2NlcHRfb3duZXJzaGlwKClgIHRvIGNvbXBsZXRlIHRoZSB0cmFuc2Zlci4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgbmV3X293bmVyYCAtIFRoZSBwcm9wb3NlZCBuZXcgb3duZXIuCiogYGxpdmVfdW50aWxfbGVkZ2VyYCAtIExlZGdlciBudW1iZXIgdW50aWwgd2hpY2ggdGhlIG5ldyBvd25lciBjYW4KYWNjZXB0LiBBIHZhbHVlIG9mIGAwYCBjYW5jZWxzIGFueSBwZW5kaW5nIHRyYW5zZmVyLgoKIyBFcnJvcnMKCiogW2BPd25hYmxlRXJyb3I6Ok93bmVyTm90U2V0YF0gLSBJZiB0aGUgb3duZXIgaXMgbm90IHNldC4KKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRyeWluZyB0byBjYW5jZWwgYSB0cmFuc2ZlciB0aGF0IGRvZXNuJ3QgZXhpc3QuCiogW2BjcmF0ZTo6cm9sZV90cmFuc2Zlcjo6Um9sZVRyYW5zZmVyRXJyb3I6OkludmFsaWRMaXZlVW50aWxMZWRnZXJgXSAtCklmIHRoZSBzcGVjaWZpZWQgbGVkZ2VyIGlzIGluIHRoZSBwYXN0LgoqIFtgY3JhdGU6OnJvbGVfdHJhbnNmZXI6OlJvbGVUcmFuc2ZlckVycm9yOjpJbnZhbGlkUGVuZGluZ0FjY291bnRgXSAtCklmIHRoZSBzcGVjaWZpZWQgcGVuZGluZyBhY2NvdW50IGlzIG5vdCB0aGUgc2FtZSBhcyB0aGUgcHJvdmlkZWQgYG5ld2AKYWRkcmVzcy4KCiMgTm90ZXMKCiogQXV0aG9yaXphdGlvbiBmb3IgdGhlIGN1cnJlbnQgb3duZXIgaXMgcmVxdWlyZWQuAAAAAAASdHJhbnNmZXJfb3duZXJzaGlwAAAAAAACAAAAAAAAAAluZXdfb3duZXIAAAAAAAATAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAA",
        "AAAAAAAAAAAAAAAUc2V0X3Bvb2xfYmVuZWZpY2lhcnkAAAACAAAAAAAAAAdwb29sX2lkAAAAAAQAAAAAAAAAD25ld19iZW5lZmljaWFyeQAAAAATAAAAAA==",
        "AAAAAAAAAAAAAAAVc2V0X3Bvb2xfcHJvdG9jb2xfZmVlAAAAAAAAAgAAAAAAAAAHcG9vbF9pZAAAAAAEAAAAAAAAAAduZXdfZmVlAAAAAAYAAAAA",
        "AAAAAAAAAAAAAAAYZ2V0X2RlZmF1bHRfcHJvdG9jb2xfZmVlAAAAAAAAAAEAAAAG",
        "AAAAAAAAAAAAAAAYc2V0X2RlZmF1bHRfcHJvdG9jb2xfZmVlAAAAAQAAAAAAAAAHbmV3X2ZlZQAAAAAGAAAAAA==",
        "AAAAAAAAAAAAAAAgZ2V0X2RlZmF1bHRfcHJvdG9jb2xfYmVuZWZpY2lhcnkAAAAAAAAAAQAAABM=",
        "AAAAAAAAAAAAAAAgc2V0X2RlZmF1bHRfcHJvdG9jb2xfYmVuZWZpY2lhcnkAAAABAAAAAAAAAA9uZXdfYmVuZWZpY2lhcnkAAAAAEwAAAAA=",
        "AAAAAQAAAJ5PbmUgbGVnIG9mIGFuIGV4YWN0LWlucHV0IHJvdXRlZCBzd2FwLgoKYHRva2VuX2luYCBpcyBpbXBsaWNpdDogaXQgaXMgdGhlIHJvdXRlcidzIGB0b2tlbl9pbmAgZm9yIHRoZSBmaXJzdCBob3AgYW5kCnRoZSBwcmV2aW91cyBob3AncyBgdG9rZW5fb3V0YCB0aGVyZWFmdGVyLgAAAAAAAAAAAAdTd2FwSG9wAAAAAAIAAAAAAAAAB3Bvb2xfaWQAAAAABAAAAAAAAAAJdG9rZW5fb3V0AAAAAAAAEw==",
        "AAAAAQAAAB9PbmUgYXNzZXQgb24gb25lIFNFUC00MCBvcmFjbGUuAAAAAAAAAAAJUHJpY2VGZWVkAAAAAAAAAgAAAAAAAAAFYXNzZXQAAAAAAAfQAAAAC09yYWNsZUFzc2V0AAAAAAAAAAAGb3JhY2xlAAAAAAAT",
        "AAAAAQAAAAAAAAAAAAAACkRlcGVnQ2hlY2sAAAAAAAQAAABASXRzIHNtYWxsZXN0IGRldmlhdGlvbiBhY3Jvc3MgdGhlIGNvbmZpcm1hdGlvbiByZWFkaW5ncywgaW4gYnBzLgAAAA1kZXZpYXRpb25fYnBzAAAAAAAABAAAACxJdHMgbGF0ZXN0IHByaWNlLCBub3JtYWxpemVkIHRvIDE0IGRlY2ltYWxzLgAAAAVwcmljZQAAAAAAAAsAAAAAAAAABnN0YXR1cwAAAAAH0AAAAAtEZXBlZ1N0YXR1cwAAAAA+VGhlIHRva2VuIGZ1cnRoZXN0IGZyb20gdGhlIHBlZyAoYE5vbmVgIHdoZW4gYEFscmVhZHlQYXVzZWRgKS4AAAAAAAV0b2tlbgAAAAAAA+gAAAAT",
        "AAAAAgAAAAAAAAAAAAAAC0RlcGVnU3RhdHVzAAAAAAMAAAAAAAAAR0V2ZXJ5IHRva2VuIGlzIHdpdGhpbiB0aGUgdGhyZXNob2xkIChvciBub3QgYnJlYWNoZWQgb24gZXZlcnkgcmVhZGluZykuAAAAAAdIZWFsdGh5AAAAAAAAAAAoVGhpcyBjYWxsIHNldCB0aGUgcG9vbCdzIHByb3RvY29sIHBhdXNlLgAAAAZQYXVzZWQAAAAAAAAAAAA5VGhlIHBvb2wgd2FzIGFscmVhZHkgcHJvdG9jb2wtcGF1c2VkOyBubyBvcmFjbGUgd2FzIHJlYWQuAAAAAAAADUFscmVhZHlQYXVzZWQAAAA=",
        "AAAAAgAAAElTRVAtNDAgYXNzZXQgaWRlbnRpZmllcjogYSBTdGVsbGFyIGFzc2V0IGNvbnRyYWN0IG9yIGFuIG9mZi1jaGFpbiBzeW1ib2wuAAAAAAAAAAAAAAtPcmFjbGVBc3NldAAAAAACAAAAAQAAAAAAAAAHU3RlbGxhcgAAAAABAAAAEwAAAAEAAAAAAAAABU90aGVyAAAAAAAAAQAAABE=",
        "AAAAAQAAAFdQZXItcG9vbCBkZXBlZyBwcm90ZWN0aW9uLCBzZXQgYnkgcm91dGVyIGdvdmVybmFuY2UuIFBvb2xzIHdpdGhvdXQgb25lIGFyZQp1bnByb3RlY3RlZC4AAAAAAAAAABBEZXBlZ0d1YXJkQ29uZmlnAAAABQAAAE1Ib3cgbWFueSBvZiB0aGUgbW9zdCByZWNlbnQgcmVhZGluZ3MgbXVzdCBhbGwgYnJlYWNoIHRoZSB0aHJlc2hvbGQuCmAxLi49MTBgLgAAAAAAAA1jb25maXJtYXRpb25zAAAAAAAABAAAAGhQYXVzZSB3aGVuIGEgdG9rZW4gaXMgc3RyaWN0bHkgZnVydGhlciB0aGFuIHRoaXMgZnJvbSB0aGUgcGVnLCBpbgpiYXNpcyBwb2ludHMgb2YgdGhlIHBlZy4gYDEuLj0xMF8wMDBgLgAAABFtYXhfZGV2aWF0aW9uX2JwcwAAAAAAAAQAAABPUmVqZWN0IHRoZSBsYXRlc3QgcmVhZGluZyB3aGVuIGl0IGlzIG9sZGVyIHRoYW4gdGhpcywgaW4gc2Vjb25kcy4KYDEuLj04Nl80MDBgLgAAAAANbWF4X3ByaWNlX2FnZQAAAAAAAAYAAABFVGFyZ2V0IHByaWNlLCBub3JtYWxpemVkIHRvIDE0IGRlY2ltYWxzICgxLjAgPT0gMTAwXzAwMF8wMDBfMDAwXzAwMCkuAAAAAAAACXBlZ19wcmljZQAAAAAAAAsAAAA1T25lIHNvdXJjZSBwZXIgcG9vbCB0b2tlbiwgaW4gdGhlIHBvb2wncyB0b2tlbiBvcmRlci4AAAAAAAAHc291cmNlcwAAAAPqAAAH0AAAABBUb2tlblByaWNlU291cmNl",
        "AAAAAQAAAU5Ib3cgdG8gcHJpY2Ugb25lIHBvb2wgdG9rZW4gaW4gdGhlIHBlZydzIHVuaXQ6IGBmZWVkYCwgbXVsdGlwbGllZCBpbiBvcmRlcgpieSBlYWNoIG9mIGBxdW90ZXNgLiBXaXRoIG5vIHF1b3RlcywgYGZlZWRgIG11c3QgYWxyZWFkeSBiZSBkZW5vbWluYXRlZCBpbgp0aGUgcGVnJ3MgdW5pdC4gV2l0aCBvbmUsIGZvciBleGFtcGxlLCBhIFN0ZWxsYXItREVYIGZlZWQgcHJpY2VkIGluIFVTREMgaXMKbXVsdGlwbGllZCBieSBVU0RDJ3MgVVNEIHByaWNlLiBFdmVyeSBxdW90ZSBtdXN0IHB1Ymxpc2ggYXQgdGhlIHNhbWUKdGltZXN0YW1wcyBhcyBgZmVlZGAuIEF0IG1vc3QgMiBxdW90ZXMuAAAAAAAAAAAAEFRva2VuUHJpY2VTb3VyY2UAAAACAAAAAAAAAARmZWVkAAAH0AAAAAlQcmljZUZlZWQAAAAAAAAAAAAABnF1b3RlcwAAAAAD6gAAB9AAAAAJUHJpY2VGZWVkAAAA",
        "AAAAAgAAAHpEZXRlcm1pbmVzIHdoZXRoZXIgdGhlIHBvb2wncyBhbXBsaWZpY2F0aW9uIGZhY3RvciBpcyBwZXJtYW5lbnRseSBmaXhlZCBvcgpkZWxlZ2F0ZWQgdG8gdGhlIGltbXV0YWJsZSBwcm90b2NvbCBjb250cm9sbGVyLgAAAAAAAAAAAApBbXBDb250cm9sAAAAAAACAAAAAAAAAAAAAAAGTG9ja2VkAAAAAAAAAAAAAAAAAA9Qcm90b2NvbE1hbmFnZWQA",
        "AAAABAAAAAAAAAAAAAAAEVJvbGVUcmFuc2ZlckVycm9yAAAAAAAABAAAAAAAAAARTm9QZW5kaW5nVHJhbnNmZXIAAAAAAAiYAAAAAAAAABZJbnZhbGlkTGl2ZVVudGlsTGVkZ2VyAAAAAAiZAAAAAAAAABVJbnZhbGlkUGVuZGluZ0FjY291bnQAAAAAAAiaAAAAAAAAAA9UcmFuc2ZlckV4cGlyZWQAAAAImw==",
        "AAAABAAAAAAAAAAAAAAADE93bmFibGVFcnJvcgAAAAMAAAAAAAAAC093bmVyTm90U2V0AAAACDQAAAAAAAAAElRyYW5zZmVySW5Qcm9ncmVzcwAAAAAINQAAAAAAAAAPT3duZXJBbHJlYWR5U2V0AAAACDY=",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGluaXRpYXRlZC4AAAAAAAAAAAART3duZXJzaGlwVHJhbnNmZXIAAAAAAAABAAAAEm93bmVyc2hpcF90cmFuc2ZlcgAAAAAAAwAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAAAAAAAg==",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGNvbXBsZXRlZC4AAAAAAAAAAAAaT3duZXJzaGlwVHJhbnNmZXJDb21wbGV0ZWQAAAAAAAEAAAAcb3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZAAAAAEAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAg==" ]),
      options
    )
  }
  public readonly fromJSON = {
    pool_at: this.txFromJSON<Option<string>>,
        upgrade: this.txFromJSON<null>,
        get_owner: this.txFromJSON<Option<string>>,
        pause_pool: this.txFromJSON<null>,
        check_depeg: this.txFromJSON<DepegCheck>,
        create_pool: this.txFromJSON<string>,
        next_pool_id: this.txFromJSON<u32>,
        unpause_pool: this.txFromJSON<null>,
        upgrade_pool: this.txFromJSON<null>,
        swap_exact_in: this.txFromJSON<i128>,
        get_depeg_guard: this.txFromJSON<Option<DepegGuardConfig>>,
        set_depeg_guard: this.txFromJSON<null>,
        accept_ownership: this.txFromJSON<null>,
        set_pool_amp_ramp: this.txFromJSON<null>,
        get_pool_wasm_hash: this.txFromJSON<Buffer>,
        remove_depeg_guard: this.txFromJSON<null>,
        renounce_ownership: this.txFromJSON<null>,
        set_pool_wasm_hash: this.txFromJSON<null>,
        stop_pool_amp_ramp: this.txFromJSON<null>,
        transfer_ownership: this.txFromJSON<null>,
        set_pool_beneficiary: this.txFromJSON<null>,
        set_pool_protocol_fee: this.txFromJSON<null>,
        get_default_protocol_fee: this.txFromJSON<u64>,
        set_default_protocol_fee: this.txFromJSON<null>,
        get_default_protocol_beneficiary: this.txFromJSON<string>,
        set_default_protocol_beneficiary: this.txFromJSON<null>
  }
}