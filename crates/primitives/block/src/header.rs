use core::num::NonZeroU128;

use blockifier::versioned_constants::VersionedConstants;
use dp_transactions::MAIN_CHAIN_ID;
use dp_transactions::V0_7_BLOCK_NUMBER;
use starknet_types_core::felt::Felt;
use starknet_types_core::hash::Pedersen;
use starknet_types_core::hash::Poseidon;
use starknet_types_core::hash::StarkHash as StarkHashTrait;

use crate::starknet_version::StarknetVersion;

/// Block status.
///
/// The status of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockStatus {
    Pending,
    #[default]
    AcceptedOnL2,
    AcceptedOnL1,
    Rejected,
}

impl From<BlockStatus> for starknet_core::types::BlockStatus {
    fn from(status: BlockStatus) -> Self {
        match status {
            BlockStatus::Pending => starknet_core::types::BlockStatus::Pending,
            BlockStatus::AcceptedOnL2 => starknet_core::types::BlockStatus::AcceptedOnL2,
            BlockStatus::AcceptedOnL1 => starknet_core::types::BlockStatus::AcceptedOnL1,
            BlockStatus::Rejected => starknet_core::types::BlockStatus::Rejected,
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PendingHeader {
    /// The hash of this block’s parent.
    pub parent_block_hash: Felt,
    /// The Starknet address of the sequencer who created this block.
    pub sequencer_address: Felt,
    /// The time the sequencer created this block before executing transactions
    pub block_timestamp: u64,
    /// The version of the Starknet protocol used when creating this block
    pub protocol_version: StarknetVersion,
    /// Gas prices for this block
    pub l1_gas_price: GasPrices,
    /// The mode of data availability for this block
    pub l1_da_mode: L1DataAvailabilityMode,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
/// Starknet header definition.
pub struct Header {
    /// The hash of this block’s parent.
    pub parent_block_hash: Felt,
    /// The number (height) of this block.
    pub block_number: u64,
    /// The state commitment after this block.
    pub global_state_root: Felt,
    /// The Starknet address of the sequencer who created this block.
    pub sequencer_address: Felt,
    /// The time the sequencer created this block before executing transactions
    pub block_timestamp: u64,
    /// The number of transactions in a block
    pub transaction_count: u64,
    /// A commitment to the transactions included in the block
    pub transaction_commitment: Felt,
    /// The number of events
    pub event_count: u64,
    /// A commitment to the events produced in this block
    pub event_commitment: Felt,
    pub state_diff_length: u64,
    pub state_diff_commitment: Felt,
    pub receipt_commitment: Felt,
    /// The version of the Starknet protocol used when creating this block
    pub protocol_version: StarknetVersion,
    /// Gas prices for this block
    pub l1_gas_price: GasPrices,
    /// The mode of data availability for this block
    pub l1_da_mode: L1DataAvailabilityMode,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GasPrices {
    pub eth_l1_gas_price: u128,
    pub strk_l1_gas_price: u128,
    pub eth_l1_data_gas_price: u128,
    pub strk_l1_data_gas_price: u128,
}

impl From<&GasPrices> for blockifier::block::GasPrices {
    fn from(gas_prices: &GasPrices) -> Self {
        let one = NonZeroU128::new(1).unwrap();

        Self {
            eth_l1_gas_price: NonZeroU128::new(gas_prices.eth_l1_gas_price).unwrap_or(one),
            strk_l1_gas_price: NonZeroU128::new(gas_prices.strk_l1_gas_price).unwrap_or(one),
            eth_l1_data_gas_price: NonZeroU128::new(gas_prices.eth_l1_data_gas_price).unwrap_or(one),
            strk_l1_data_gas_price: NonZeroU128::new(gas_prices.strk_l1_data_gas_price).unwrap_or(one),
        }
    }
}

impl GasPrices {
    pub fn l1_gas_price(&self) -> starknet_core::types::ResourcePrice {
        starknet_core::types::ResourcePrice {
            price_in_fri: self.strk_l1_gas_price.into(),
            price_in_wei: self.eth_l1_gas_price.into(),
        }
    }
    pub fn l1_data_gas_price(&self) -> starknet_core::types::ResourcePrice {
        starknet_core::types::ResourcePrice {
            price_in_fri: self.strk_l1_data_gas_price.into(),
            price_in_wei: self.eth_l1_data_gas_price.into(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum L1DataAvailabilityMode {
    #[default]
    Calldata,
    Blob,
}

impl From<L1DataAvailabilityMode> for starknet_core::types::L1DataAvailabilityMode {
    fn from(value: L1DataAvailabilityMode) -> Self {
        match value {
            L1DataAvailabilityMode::Calldata => Self::Calldata,
            L1DataAvailabilityMode::Blob => Self::Blob,
        }
    }
}

const BLOCKIFIER_VERSIONED_CONSTANTS_JSON_0_13_0: &[u8] = include_bytes!("../resources/versioned_constants_13_0.json");
const BLOCKIFIER_VERSIONED_CONSTANTS_JSON_0_13_1: &[u8] = include_bytes!("../resources/versioned_constants_13_1.json");

lazy_static::lazy_static! {
pub static ref BLOCKIFIER_VERSIONED_CONSTANTS_0_13_0: VersionedConstants =
    serde_json::from_slice(BLOCKIFIER_VERSIONED_CONSTANTS_JSON_0_13_0).unwrap();

pub static ref BLOCKIFIER_VERSIONED_CONSTANTS_0_13_1: VersionedConstants =
    serde_json::from_slice(BLOCKIFIER_VERSIONED_CONSTANTS_JSON_0_13_1).unwrap();
}

#[derive(thiserror::Error, Debug)]
pub enum BlockFormatError {
    #[error("The block is a pending block")]
    BlockIsPending,
    #[error("Unsupported protocol version")]
    UnsupportedBlockVersion,
}

impl Header {
    /// Creates a new header.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        parent_block_hash: Felt,
        block_number: u64,
        global_state_root: Felt,
        sequencer_address: Felt,
        block_timestamp: u64,
        transaction_count: u64,
        transaction_commitment: Felt,
        event_count: u64,
        event_commitment: Felt,
        state_diff_length: u64,
        state_diff_commitment: Felt,
        receipt_commitment: Felt,
        protocol_version: StarknetVersion,
        gas_prices: GasPrices,
        l1_da_mode: L1DataAvailabilityMode,
    ) -> Self {
        Self {
            parent_block_hash,
            block_number,
            global_state_root,
            sequencer_address,
            block_timestamp,
            transaction_count,
            transaction_commitment,
            event_count,
            event_commitment,
            state_diff_length,
            state_diff_commitment,
            receipt_commitment,
            protocol_version,
            l1_gas_price: gas_prices,
            l1_da_mode,
        }
    }

    /// Compute the hash of the header.
    pub fn compute_hash(&self, chain_id: Felt) -> Felt {
        if self.block_number < V0_7_BLOCK_NUMBER && chain_id == MAIN_CHAIN_ID {
            self.compute_hash_inner_pre_v0_7(chain_id)
        } else if self.protocol_version < StarknetVersion::STARKNET_VERSION_0_13_2 {
            Pedersen::hash_array(&[
                Felt::from(self.block_number),      // block number
                self.global_state_root,             // global state root
                self.sequencer_address,             // sequencer address
                Felt::from(self.block_timestamp),   // block timestamp
                Felt::from(self.transaction_count), // number of transactions
                self.transaction_commitment,        // transaction commitment
                Felt::from(self.event_count),       // number of events
                self.event_commitment,              // event commitment
                Felt::ZERO,                         // reserved: protocol version
                Felt::ZERO,                         // reserved: extra data
                self.parent_block_hash,             // parent block hash
            ])
        } else {
            Poseidon::hash_array(&[
                Felt::from_bytes_be_slice(b"STARKNET_BLOCK_HASH0"),
                Felt::from(self.block_number),
                self.global_state_root,
                self.sequencer_address,
                Felt::from(self.block_timestamp),
                concat_counts(self.transaction_count, self.event_count, self.state_diff_length, self.l1_da_mode),
                self.state_diff_commitment,
                self.transaction_commitment,
                self.event_commitment,
                self.receipt_commitment,
                self.l1_gas_price.eth_l1_gas_price.into(),
                self.l1_gas_price.strk_l1_gas_price.into(),
                self.l1_gas_price.eth_l1_data_gas_price.into(),
                self.l1_gas_price.strk_l1_data_gas_price.into(),
                Felt::from_bytes_be_slice(self.protocol_version.to_string().as_bytes()),
                Felt::ZERO,
                self.parent_block_hash,
            ])
        }
    }

    fn compute_hash_inner_pre_v0_7(&self, chain_id: Felt) -> Felt {
        Pedersen::hash_array(&[
            Felt::from(self.block_number),
            self.global_state_root,
            Felt::ZERO,
            Felt::ZERO,
            Felt::from(self.transaction_count),
            self.transaction_commitment,
            Felt::ZERO,
            Felt::ZERO,
            Felt::ZERO,
            Felt::ZERO,
            chain_id,
            self.parent_block_hash,
        ])
    }
}

fn concat_counts(
    transaction_count: u64,
    event_count: u64,
    state_diff_length: u64,
    l1_da_mode: L1DataAvailabilityMode,
) -> Felt {
    let l1_data_availability_byte: u8 = match l1_da_mode {
        L1DataAvailabilityMode::Calldata => 0,
        L1DataAvailabilityMode::Blob => 0b10000000,
    };

    let concat_bytes = [
        transaction_count.to_be_bytes(),
        event_count.to_be_bytes(),
        state_diff_length.to_be_bytes(),
        [l1_data_availability_byte, 0_u8, 0_u8, 0_u8, 0_u8, 0_u8, 0_u8, 0_u8],
    ]
    .concat();

    Felt::from_bytes_be_slice(concat_bytes.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat_counts() {
        let concated = concat_counts(4, 3, 2, L1DataAvailabilityMode::Blob);
        assert_eq!(
            concated,
            Felt::from_hex_unchecked("0x0000000000000004000000000000000300000000000000028000000000000000")
        );
    }

    #[test]
    fn test_header_hash_v0_13_2() {
        let header = Header {
            parent_block_hash: Felt::from(1),
            block_number: 2,
            global_state_root: Felt::from(3),
            sequencer_address: Felt::from(4),
            block_timestamp: 5,
            transaction_count: 6,
            transaction_commitment: Felt::from(7),
            event_count: 8,
            event_commitment: Felt::from(9),
            state_diff_length: 10,
            state_diff_commitment: Felt::from(11),
            receipt_commitment: Felt::from(12),
            protocol_version: "0.13.2".parse().unwrap(),
            l1_gas_price: GasPrices {
                eth_l1_gas_price: 14,
                strk_l1_gas_price: 15,
                eth_l1_data_gas_price: 16,
                strk_l1_data_gas_price: 17,
            },
            l1_da_mode: L1DataAvailabilityMode::Blob,
        };

        let hash = header.compute_hash(Felt::from_bytes_be_slice(b"CHAIN_ID"));

        assert_eq!(hash, Felt::from_hex_unchecked("0x545dd9ef652b07cebb3c8b6d43b6c477998f124e75df970dfee300fb32a698b"));
    }

    #[test]
    fn test_header_hash_v0_11_1() {
        let header = Header {
            parent_block_hash: Felt::from(1),
            block_number: 2,
            global_state_root: Felt::from(3),
            sequencer_address: Felt::from(4),
            block_timestamp: 5,
            transaction_count: 6,
            transaction_commitment: Felt::from(7),
            event_count: 8,
            event_commitment: Felt::from(9),
            state_diff_length: 10,
            state_diff_commitment: Felt::from(11),
            receipt_commitment: Felt::from(12),
            protocol_version: "0.11.1".parse().unwrap(),
            l1_gas_price: GasPrices {
                eth_l1_gas_price: 0,
                strk_l1_gas_price: 0,
                eth_l1_data_gas_price: 0,
                strk_l1_data_gas_price: 0,
            },
            l1_da_mode: L1DataAvailabilityMode::Calldata,
        };

        let hash = header.compute_hash(Felt::from_bytes_be_slice(b"CHAIN_ID"));

        assert_eq!(hash, Felt::from_hex_unchecked("0x42ec5792c165e0235d7576dc9b4a56140b217faba0b2f57c0a48b850ea5999c"));
    }

    #[test]
    fn test_header_hash_pre_v0_7() {
        let header = Header {
            parent_block_hash: Felt::from(1),
            block_number: 2,
            global_state_root: Felt::from(3),
            sequencer_address: Felt::from(4),
            block_timestamp: 5,
            transaction_count: 6,
            transaction_commitment: Felt::from(7),
            event_count: 8,
            event_commitment: Felt::from(9),
            state_diff_length: 10,
            state_diff_commitment: Felt::from(11),
            receipt_commitment: Felt::from(12),
            protocol_version: Default::default(),
            l1_gas_price: GasPrices {
                eth_l1_gas_price: 0,
                strk_l1_gas_price: 0,
                eth_l1_data_gas_price: 0,
                strk_l1_data_gas_price: 0,
            },
            l1_da_mode: L1DataAvailabilityMode::Calldata,
        };

        let hash = header.compute_hash(Felt::from_bytes_be_slice(b"SN_MAIN"));

        assert_eq!(hash, Felt::from_hex_unchecked("0x6028bf0975e1d4c95713e021a0f0217e74d5a748a20691d881c86d9d62d1432"));
    }
}
