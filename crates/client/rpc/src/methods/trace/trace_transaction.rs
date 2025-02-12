use dc_exec::execution_result_to_tx_trace;
use dc_exec::ExecutionContext;
use dp_block::StarknetVersion;
use dp_convert::ToStarkFelt;
use starknet_api::transaction::TransactionHash;
use starknet_core::types::Felt;
use starknet_core::types::TransactionTraceWithHash;

use crate::errors::StarknetRpcApiError;
use crate::errors::StarknetRpcResult;
use crate::utils::transaction::to_blockifier_transactions;
use crate::utils::{OptionExt, ResultExt};
use crate::Starknet;

// For now, we fallback to the sequencer - that is what pathfinder and juno do too, but this is temporary
pub const FALLBACK_TO_SEQUENCER_WHEN_VERSION_BELOW: StarknetVersion = StarknetVersion::STARKNET_VERSION_0_13_0;

pub async fn trace_transaction(
    starknet: &Starknet,
    transaction_hash: Felt,
) -> StarknetRpcResult<TransactionTraceWithHash> {
    let (block, tx_index) = starknet
        .backend
        .find_tx_hash_block(&transaction_hash)
        .or_internal_server_error("Error while getting block from tx hash")?
        .ok_or(StarknetRpcApiError::TxnHashNotFound)?;

    if block.info.protocol_version() < &FALLBACK_TO_SEQUENCER_WHEN_VERSION_BELOW {
        return Err(StarknetRpcApiError::UnsupportedTxnVersion);
    }

    let exec_context = ExecutionContext::new(&starknet.backend, &block.info)?;

    let mut block_txs = Iterator::zip(block.inner.transactions.iter(), block.info.tx_hashes()).map(|(tx, hash)| {
        to_blockifier_transactions(starknet, block.info.as_block_id(), tx, &TransactionHash(hash.to_stark_felt()))
    });

    // takes up until not including last tx
    let transactions_before: Vec<_> = block_txs.by_ref().take(tx_index.0 as usize).collect::<Result<_, _>>()?;
    // the one we're interested in comes next in the iterator
    let transaction =
        block_txs.next().ok_or_internal_server_error("There should be at least one transaction in the block")??;

    let mut executions_results = exec_context.execute_transactions(transactions_before, [transaction], true, true)?;

    let execution_result =
        executions_results.pop().ok_or_internal_server_error("No execution info returned for the last transaction")?;

    let trace = execution_result_to_tx_trace(&execution_result)
        .or_internal_server_error("Converting execution infos to tx trace")?;

    Ok(TransactionTraceWithHash { transaction_hash, trace_root: trace })
}
