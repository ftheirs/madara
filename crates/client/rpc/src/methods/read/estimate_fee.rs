use dc_exec::ExecutionContext;
use dp_transactions::broadcasted_to_blockifier;
use starknet_core::types::{BlockId, BroadcastedTransaction, FeeEstimate, SimulationFlagForEstimateFee};

use crate::errors::StarknetRpcResult;
use crate::utils::ResultExt;
use crate::Starknet;
use crate::{errors::StarknetRpcApiError, methods::trace::trace_transaction::FALLBACK_TO_SEQUENCER_WHEN_VERSION_BELOW};

/// Estimate the fee associated with transaction
///
/// # Arguments
///
/// * `request` - starknet transaction request
/// * `block_id` - hash of the requested block, number (height), or tag
///
/// # Returns
///
/// * `fee_estimate` - fee estimate in gwei
pub async fn estimate_fee(
    starknet: &Starknet,
    request: Vec<BroadcastedTransaction>,
    simulation_flags: Vec<SimulationFlagForEstimateFee>,
    block_id: BlockId,
) -> StarknetRpcResult<Vec<FeeEstimate>> {
    let block_info = starknet.get_block_info(&block_id)?;

    if block_info.protocol_version() < &FALLBACK_TO_SEQUENCER_WHEN_VERSION_BELOW {
        return Err(StarknetRpcApiError::UnsupportedTxnVersion);
    }

    let exec_context = ExecutionContext::new(&starknet.backend, &block_info)?;

    let transactions = request
        .into_iter()
        .map(|tx| broadcasted_to_blockifier(tx, starknet.chain_id()))
        .collect::<Result<Vec<_>, _>>()
        .or_internal_server_error("Failed to convert BroadcastedTransaction to AccountTransaction")?;

    let validate = !simulation_flags.contains(&SimulationFlagForEstimateFee::SkipValidate);

    let execution_results = exec_context.execute_transactions([], transactions, validate, true)?;

    let fee_estimates =
        execution_results.iter().map(|result| exec_context.execution_result_to_fee_estimate(result)).collect();

    Ok(fee_estimates)
}
