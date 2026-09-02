import { Buffer } from "buffer";
import { AssembledTransaction, Client as ContractClient, ClientOptions as ContractClientOptions, MethodOptions } from "@stellar/stellar-sdk/contract";
import type { u32, u64, i128, Option } from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";
export declare const Errors: {
    1: {
        message: string;
    };
};
export declare const RoleTransferError: {
    2200: {
        message: string;
    };
    2201: {
        message: string;
    };
    2202: {
        message: string;
    };
    2203: {
        message: string;
    };
};
export declare const OwnableError: {
    2100: {
        message: string;
    };
    2101: {
        message: string;
    };
    2102: {
        message: string;
    };
};
export interface Client {
    /**
     * Construct and simulate a is_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    is_pool: ({ pool }: {
        pool: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>;
    /**
     * Construct and simulate a pool_at transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    pool_at: ({ index }: {
        index: u32;
    }, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>;
    /**
     * Construct and simulate a get_owner transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     * Returns `Some(Address)` if ownership is set, or `None` if ownership has
     * been renounced.
     *
     * # Arguments
     *
     * * `e` - Access to the Soroban environment.
     */
    get_owner: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>;
    /**
     * Construct and simulate a pool_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    pool_count: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>;
    /**
     * Construct and simulate a create_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    create_pool: ({ creator, tokens, amp_factor, swap_fee, protocol_fee, beneficiary, max_caps, lp_max_supply, lp_name, lp_symbol }: {
        creator: string;
        tokens: Array<string>;
        amp_factor: u32;
        swap_fee: u64;
        protocol_fee: u64;
        beneficiary: string;
        max_caps: Array<i128>;
        lp_max_supply: i128;
        lp_name: string;
        lp_symbol: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<string>>;
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
    accept_ownership: (options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_pool_wasm_hash: (options?: MethodOptions) => Promise<AssembledTransaction<Buffer>>;
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
    renounce_ownership: (options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a set_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_pool_wasm_hash: ({ new_hash }: {
        new_hash: Buffer;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
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
    transfer_ownership: ({ new_owner, live_until_ledger }: {
        new_owner: string;
        live_until_ledger: u32;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
}
export declare class Client extends ContractClient {
    readonly options: ContractClientOptions;
    static deploy<T = Client>(
    /** Constructor/Initialization Args for the contract's `__constructor` method */
    { owner, pool_wasm_hash }: {
        owner: string;
        pool_wasm_hash: Buffer;
    },
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions & Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
    }): Promise<AssembledTransaction<T>>;
    constructor(options: ContractClientOptions);
    readonly fromJSON: {
        is_pool: (json: string) => AssembledTransaction<boolean>;
        pool_at: (json: string) => AssembledTransaction<Option<string>>;
        get_owner: (json: string) => AssembledTransaction<Option<string>>;
        pool_count: (json: string) => AssembledTransaction<number>;
        create_pool: (json: string) => AssembledTransaction<string>;
        accept_ownership: (json: string) => AssembledTransaction<null>;
        get_pool_wasm_hash: (json: string) => AssembledTransaction<Buffer>;
        renounce_ownership: (json: string) => AssembledTransaction<null>;
        set_pool_wasm_hash: (json: string) => AssembledTransaction<null>;
        transfer_ownership: (json: string) => AssembledTransaction<null>;
    };
}
