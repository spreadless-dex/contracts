// Swap-fee policy. Fees use fixed-point rates where 1e9 is 100%.

use crate::math::fixed_math::{self, FixedComplement, FixedDiv, FixedMul};

const MIN_SWAP_FEE: u64 = 10_000; // 0.001%
const MAX_SWAP_FEE: u64 = 10_000_000; // 1%

pub struct OutputFee {
    pub gross_out: u64,
    pub net_out: u64,
    pub protocol: u64,
}

pub fn is_valid_swap_fee(swap_fee: u64) -> bool {
    (MIN_SWAP_FEE..=MAX_SWAP_FEE).contains(&swap_fee)
}

pub fn is_valid_protocol_fee(protocol_fee: u64) -> bool {
    protocol_fee <= fixed_math::ONE
}

pub fn output_from_gross(swap_fee: u64, protocol_fee: u64, gross_out: u64) -> Option<OutputFee> {
    let net_out = gross_out.mul_down(swap_fee.complement())?;
    let protocol = protocol_cut(protocol_fee, gross_out, net_out)?;
    Some(OutputFee {
        gross_out,
        net_out,
        protocol,
    })
}

pub fn output_from_net(swap_fee: u64, protocol_fee: u64, net_out: u64) -> Option<OutputFee> {
    let gross_out = net_out.div_up(swap_fee.complement())?;
    let protocol = protocol_cut(protocol_fee, gross_out, net_out)?;
    Some(OutputFee {
        gross_out,
        net_out,
        protocol,
    })
}

/// The protocol's share of a fee amount: `fee_amount * protocol_fee`, rounded
/// down. The single policy used everywhere a fee is charged — swaps (fee =
/// gross − net), single-token withdrawals (fee withheld from the output), and
/// deposits (fee expressed in LP shares).
pub fn protocol_share(fee_amount: u64, protocol_fee: u64) -> Option<u64> {
    fee_amount.mul_down(protocol_fee)
}

fn protocol_cut(protocol_fee: u64, gross_out: u64, net_out: u64) -> Option<u64> {
    protocol_share(gross_out.saturating_sub(net_out), protocol_fee)
}
