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
  1: {message:"PoolCountOverflow"}
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




export interface Client {
  /**
   * Construct and simulate a is_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_pool: ({pool}: {pool: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a pool_at transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pool_at: ({index}: {index: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

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
   * Construct and simulate a pool_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pool_count: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a create_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_pool: ({creator, tokens, amp_factor, swap_fee, protocol_fee, beneficiary, max_caps, lp_max_supply, lp_name, lp_symbol}: {creator: string, tokens: Array<string>, amp_factor: u32, swap_fee: u64, protocol_fee: u64, beneficiary: string, max_caps: Array<i128>, lp_max_supply: i128, lp_name: string, lp_symbol: string}, options?: MethodOptions) => Promise<AssembledTransaction<string>>

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
   * Construct and simulate a get_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_pool_wasm_hash: (options?: MethodOptions) => Promise<AssembledTransaction<Buffer>>

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
   * Construct and simulate a set_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_pool_wasm_hash: ({new_hash}: {new_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {owner, pool_wasm_hash}: {owner: string, pool_wasm_hash: Buffer},
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
    return ContractClient.deploy({owner, pool_wasm_hash}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAAAQAAAAAAAAARUG9vbENvdW50T3ZlcmZsb3cAAAAAAAAB",
        "AAAAAAAAAAAAAAAHaXNfcG9vbAAAAAABAAAAAAAAAARwb29sAAAAEwAAAAEAAAAB",
        "AAAAAAAAAAAAAAAHcG9vbF9hdAAAAAABAAAAAAAAAAVpbmRleAAAAAAAAAQAAAABAAAD6AAAABM=",
        "AAAABQAAAAAAAAAAAAAAC1Bvb2xDcmVhdGVkAAAAAAEAAAAMcG9vbF9jcmVhdGVkAAAAAwAAAAAAAAAEcG9vbAAAABMAAAABAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAAAAAAAAVpbmRleAAAAAAAAAQAAAAAAAAAAg==",
        "AAAAAAAAAJBSZXR1cm5zIGBTb21lKEFkZHJlc3MpYCBpZiBvd25lcnNoaXAgaXMgc2V0LCBvciBgTm9uZWAgaWYgb3duZXJzaGlwIGhhcwpiZWVuIHJlbm91bmNlZC4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4AAAAJZ2V0X293bmVyAAAAAAAAAAAAAAEAAAPoAAAAEw==",
        "AAAAAAAAAAAAAAAKcG9vbF9jb3VudAAAAAAAAAAAAAEAAAAE",
        "AAAAAAAAAAAAAAALY3JlYXRlX3Bvb2wAAAAACgAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZ0b2tlbnMAAAAAA+oAAAATAAAAAAAAAAphbXBfZmFjdG9yAAAAAAAEAAAAAAAAAAhzd2FwX2ZlZQAAAAYAAAAAAAAADHByb3RvY29sX2ZlZQAAAAYAAAAAAAAAC2JlbmVmaWNpYXJ5AAAAABMAAAAAAAAACG1heF9jYXBzAAAD6gAAAAsAAAAAAAAADWxwX21heF9zdXBwbHkAAAAAAAALAAAAAAAAAAdscF9uYW1lAAAAABAAAAAAAAAACWxwX3N5bWJvbAAAAAAAABAAAAABAAAAEw==",
        "AAAABQAAAAAAAAAAAAAAD1Bvb2xXYXNtVXBkYXRlZAAAAAABAAAAEXBvb2xfd2FzbV91cGRhdGVkAAAAAAAAAgAAAAAAAAAIb2xkX2hhc2gAAAPuAAAAIAAAAAAAAAAAAAAACG5ld19oYXNoAAAD7gAAACAAAAAAAAAAAg==",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAOcG9vbF93YXNtX2hhc2gAAAAAA+4AAAAgAAAAAA==",
        "AAAAAAAAATBBY2NlcHRzIGEgcGVuZGluZyBvd25lcnNoaXAgdHJhbnNmZXIuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gdGhlIFNvcm9iYW4gZW52aXJvbm1lbnQuCgojIEVycm9ycwoKKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRoZXJlIGlzIG5vIHBlbmRpbmcgdHJhbnNmZXIgdG8gYWNjZXB0LgoKIyBFdmVudHMKCiogdG9waWNzIC0gYFsib3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZCJdYAoqIGRhdGEgLSBgW25ld19vd25lcjogQWRkcmVzc11gAAAAEGFjY2VwdF9vd25lcnNoaXAAAAAAAAAAAA==",
        "AAAAAAAAAAAAAAASZ2V0X3Bvb2xfd2FzbV9oYXNoAAAAAAAAAAAAAQAAA+4AAAAg",
        "AAAAAAAAAYVSZW5vdW5jZXMgb3duZXJzaGlwIG9mIHRoZSBjb250cmFjdC4KClBlcm1hbmVudGx5IHJlbW92ZXMgdGhlIG93bmVyLCBkaXNhYmxpbmcgYWxsIGZ1bmN0aW9ucyBnYXRlZCBieQpgI1tvbmx5X293bmVyXWAuCgojIEFyZ3VtZW50cwoKKiBgZWAgLSBBY2Nlc3MgdG8gdGhlIFNvcm9iYW4gZW52aXJvbm1lbnQuCgojIEVycm9ycwoKKiBbYE93bmFibGVFcnJvcjo6VHJhbnNmZXJJblByb2dyZXNzYF0gLSBJZiB0aGVyZSBpcyBhIHBlbmRpbmcgb3duZXJzaGlwCnRyYW5zZmVyLgoqIFtgT3duYWJsZUVycm9yOjpPd25lck5vdFNldGBdIC0gSWYgdGhlIG93bmVyIGlzIG5vdCBzZXQuCgojIE5vdGVzCgoqIEF1dGhvcml6YXRpb24gZm9yIHRoZSBjdXJyZW50IG93bmVyIGlzIHJlcXVpcmVkLgAAAAAAABJyZW5vdW5jZV9vd25lcnNoaXAAAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAASc2V0X3Bvb2xfd2FzbV9oYXNoAAAAAAABAAAAAAAAAAhuZXdfaGFzaAAAA+4AAAAgAAAAAA==",
        "AAAAAAAAA45Jbml0aWF0ZXMgYSAyLXN0ZXAgb3duZXJzaGlwIHRyYW5zZmVyIHRvIGEgbmV3IGFkZHJlc3MuCgpSZXF1aXJlcyBhdXRob3JpemF0aW9uIGZyb20gdGhlIGN1cnJlbnQgb3duZXIuIFRoZSBuZXcgb3duZXIgbXVzdCBsYXRlcgpjYWxsIGBhY2NlcHRfb3duZXJzaGlwKClgIHRvIGNvbXBsZXRlIHRoZSB0cmFuc2Zlci4KCiMgQXJndW1lbnRzCgoqIGBlYCAtIEFjY2VzcyB0byB0aGUgU29yb2JhbiBlbnZpcm9ubWVudC4KKiBgbmV3X293bmVyYCAtIFRoZSBwcm9wb3NlZCBuZXcgb3duZXIuCiogYGxpdmVfdW50aWxfbGVkZ2VyYCAtIExlZGdlciBudW1iZXIgdW50aWwgd2hpY2ggdGhlIG5ldyBvd25lciBjYW4KYWNjZXB0LiBBIHZhbHVlIG9mIGAwYCBjYW5jZWxzIGFueSBwZW5kaW5nIHRyYW5zZmVyLgoKIyBFcnJvcnMKCiogW2BPd25hYmxlRXJyb3I6Ok93bmVyTm90U2V0YF0gLSBJZiB0aGUgb3duZXIgaXMgbm90IHNldC4KKiBbYGNyYXRlOjpyb2xlX3RyYW5zZmVyOjpSb2xlVHJhbnNmZXJFcnJvcjo6Tm9QZW5kaW5nVHJhbnNmZXJgXSAtIElmCnRyeWluZyB0byBjYW5jZWwgYSB0cmFuc2ZlciB0aGF0IGRvZXNuJ3QgZXhpc3QuCiogW2BjcmF0ZTo6cm9sZV90cmFuc2Zlcjo6Um9sZVRyYW5zZmVyRXJyb3I6OkludmFsaWRMaXZlVW50aWxMZWRnZXJgXSAtCklmIHRoZSBzcGVjaWZpZWQgbGVkZ2VyIGlzIGluIHRoZSBwYXN0LgoqIFtgY3JhdGU6OnJvbGVfdHJhbnNmZXI6OlJvbGVUcmFuc2ZlckVycm9yOjpJbnZhbGlkUGVuZGluZ0FjY291bnRgXSAtCklmIHRoZSBzcGVjaWZpZWQgcGVuZGluZyBhY2NvdW50IGlzIG5vdCB0aGUgc2FtZSBhcyB0aGUgcHJvdmlkZWQgYG5ld2AKYWRkcmVzcy4KCiMgTm90ZXMKCiogQXV0aG9yaXphdGlvbiBmb3IgdGhlIGN1cnJlbnQgb3duZXIgaXMgcmVxdWlyZWQuAAAAAAASdHJhbnNmZXJfb3duZXJzaGlwAAAAAAACAAAAAAAAAAluZXdfb3duZXIAAAAAAAATAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAA",
        "AAAABAAAAAAAAAAAAAAAEVJvbGVUcmFuc2ZlckVycm9yAAAAAAAABAAAAAAAAAARTm9QZW5kaW5nVHJhbnNmZXIAAAAAAAiYAAAAAAAAABZJbnZhbGlkTGl2ZVVudGlsTGVkZ2VyAAAAAAiZAAAAAAAAABVJbnZhbGlkUGVuZGluZ0FjY291bnQAAAAAAAiaAAAAAAAAAA9UcmFuc2ZlckV4cGlyZWQAAAAImw==",
        "AAAABAAAAAAAAAAAAAAADE93bmFibGVFcnJvcgAAAAMAAAAAAAAAC093bmVyTm90U2V0AAAACDQAAAAAAAAAElRyYW5zZmVySW5Qcm9ncmVzcwAAAAAINQAAAAAAAAAPT3duZXJBbHJlYWR5U2V0AAAACDY=",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGluaXRpYXRlZC4AAAAAAAAAAAART3duZXJzaGlwVHJhbnNmZXIAAAAAAAABAAAAEm93bmVyc2hpcF90cmFuc2ZlcgAAAAAAAwAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAAAAABFsaXZlX3VudGlsX2xlZGdlcgAAAAAAAAQAAAAAAAAAAg==",
        "AAAABQAAACpFdmVudCBlbWl0dGVkIHdoZW4gb3duZXJzaGlwIGlzIHJlbm91bmNlZC4AAAAAAAAAAAAST3duZXJzaGlwUmVub3VuY2VkAAAAAAABAAAAE293bmVyc2hpcF9yZW5vdW5jZWQAAAAAAQAAAAAAAAAJb2xkX293bmVyAAAAAAAAEwAAAAAAAAAC",
        "AAAABQAAADZFdmVudCBlbWl0dGVkIHdoZW4gYW4gb3duZXJzaGlwIHRyYW5zZmVyIGlzIGNvbXBsZXRlZC4AAAAAAAAAAAAaT3duZXJzaGlwVHJhbnNmZXJDb21wbGV0ZWQAAAAAAAEAAAAcb3duZXJzaGlwX3RyYW5zZmVyX2NvbXBsZXRlZAAAAAEAAAAAAAAACW5ld19vd25lcgAAAAAAABMAAAAAAAAAAg==" ]),
      options
    )
  }
  public readonly fromJSON = {
    is_pool: this.txFromJSON<boolean>,
        pool_at: this.txFromJSON<Option<string>>,
        get_owner: this.txFromJSON<Option<string>>,
        pool_count: this.txFromJSON<u32>,
        create_pool: this.txFromJSON<string>,
        accept_ownership: this.txFromJSON<null>,
        get_pool_wasm_hash: this.txFromJSON<Buffer>,
        renounce_ownership: this.txFromJSON<null>,
        set_pool_wasm_hash: this.txFromJSON<null>,
        transfer_ownership: this.txFromJSON<null>
  }
}
