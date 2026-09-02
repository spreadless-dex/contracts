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
    2: {
        message: string;
    };
    3: {
        message: string;
    };
    4: {
        message: string;
    };
};
export type AmpControl = {
    tag: "Locked";
    values: void;
} | {
    tag: "ProtocolManaged";
    values: void;
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
     * Construct and simulate a pool_at transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    pool_at: ({ id }: {
        id: u32;
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
     * Construct and simulate a pause_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    pause_pool: ({ pool_id }: {
        pool_id: u32;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a create_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    create_pool: ({ creator, tokens, amp_factor, amp_control, swap_fee, max_caps, lp_max_supply, lp_name, lp_symbol }: {
        creator: string;
        tokens: Array<string>;
        amp_factor: u32;
        amp_control: AmpControl;
        swap_fee: u64;
        max_caps: Array<i128>;
        lp_max_supply: i128;
        lp_name: string;
        lp_symbol: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<string>>;
    /**
     * Construct and simulate a next_pool_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    next_pool_id: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>;
    /**
     * Construct and simulate a unpause_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    unpause_pool: ({ pool_id }: {
        pool_id: u32;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
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
     * Construct and simulate a set_pool_amp_ramp transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_pool_amp_ramp: ({ pool_id, target_factor, duration }: {
        pool_id: u32;
        target_factor: u32;
        duration: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_pool_wasm_hash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_pool_wasm_hash: (options?: MethodOptions) => Promise<AssembledTransaction<Buffer>>;
    /**
     * Construct and simulate a renounce_ownership transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
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
    /**
     * Construct and simulate a set_pool_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_pool_beneficiary: ({ pool_id, new_beneficiary }: {
        pool_id: u32;
        new_beneficiary: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a set_pool_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_pool_protocol_fee: ({ pool_id, new_fee }: {
        pool_id: u32;
        new_fee: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_default_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_default_protocol_fee: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>;
    /**
     * Construct and simulate a set_default_protocol_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_default_protocol_fee: ({ new_fee }: {
        new_fee: u64;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
    /**
     * Construct and simulate a get_default_protocol_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    get_default_protocol_beneficiary: (options?: MethodOptions) => Promise<AssembledTransaction<string>>;
    /**
     * Construct and simulate a set_default_protocol_beneficiary transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
     */
    set_default_protocol_beneficiary: ({ new_beneficiary }: {
        new_beneficiary: string;
    }, options?: MethodOptions) => Promise<AssembledTransaction<null>>;
}
export declare class Client extends ContractClient {
    readonly options: ContractClientOptions;
    static deploy<T = Client>(
    /** Constructor/Initialization Args for the contract's `__constructor` method */
    { owner, pool_wasm_hash, default_protocol_fee, default_protocol_beneficiary }: {
        owner: string;
        pool_wasm_hash: Buffer;
        default_protocol_fee: u64;
        default_protocol_beneficiary: string;
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
        pool_at: (json: string) => AssembledTransaction<Option<string>>;
        get_owner: (json: string) => AssembledTransaction<Option<string>>;
        pause_pool: (json: string) => AssembledTransaction<null>;
        create_pool: (json: string) => AssembledTransaction<string>;
        next_pool_id: (json: string) => AssembledTransaction<number>;
        unpause_pool: (json: string) => AssembledTransaction<null>;
        accept_ownership: (json: string) => AssembledTransaction<null>;
        set_pool_amp_ramp: (json: string) => AssembledTransaction<null>;
        get_pool_wasm_hash: (json: string) => AssembledTransaction<Buffer>;
        renounce_ownership: (json: string) => AssembledTransaction<null>;
        set_pool_wasm_hash: (json: string) => AssembledTransaction<null>;
        transfer_ownership: (json: string) => AssembledTransaction<null>;
        set_pool_beneficiary: (json: string) => AssembledTransaction<null>;
        set_pool_protocol_fee: (json: string) => AssembledTransaction<null>;
        get_default_protocol_fee: (json: string) => AssembledTransaction<bigint>;
        set_default_protocol_fee: (json: string) => AssembledTransaction<null>;
        get_default_protocol_beneficiary: (json: string) => AssembledTransaction<string>;
        set_default_protocol_beneficiary: (json: string) => AssembledTransaction<null>;
    };
}
