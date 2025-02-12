use std::sync::Arc;

use blockifier::context::TransactionContext;
use blockifier::execution::entry_point::{CallEntryPoint, CallType, EntryPointExecutionContext};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{DeprecatedTransactionInfo, TransactionInfo};
use dp_convert::{ToFelt, ToStarkFelt};
use starknet_api::core::EntryPointSelector;
use starknet_api::deprecated_contract_class::EntryPointType;
use starknet_api::transaction::Calldata;
use starknet_types_core::felt::Felt;

use crate::{CallContractError, Error, ExecutionContext};

impl<'a> ExecutionContext<'a> {
    pub fn call_contract(
        &self,
        contract_address: &Felt,
        entry_point_selector: &Felt,
        calldata: &[Felt],
    ) -> Result<Vec<Felt>, Error> {
        log::debug!("calling contract {contract_address:#x}");

        let make_err = |err| CallContractError { block_n: self.db_id, contract: *contract_address, err };

        let entrypoint = CallEntryPoint {
            code_address: None,
            entry_point_type: EntryPointType::External,
            entry_point_selector: EntryPointSelector(entry_point_selector.to_stark_felt()),
            calldata: Calldata(Arc::new(calldata.iter().map(|x| x.to_stark_felt()).collect())),
            storage_address: contract_address
                .to_stark_felt()
                .try_into()
                .map_err(TransactionExecutionError::StarknetApiError)
                .map_err(make_err)?,
            call_type: CallType::Call,
            initial_gas: self.block_context.versioned_constants().tx_initial_gas(),
            ..Default::default()
        };

        let mut resources = cairo_vm::vm::runners::cairo_runner::ExecutionResources::default();
        let mut entry_point_execution_context = EntryPointExecutionContext::new_invoke(
            Arc::new(TransactionContext {
                block_context: self.block_context.clone(),
                tx_info: TransactionInfo::Deprecated(DeprecatedTransactionInfo::default()),
            }),
            false,
        )
        .map_err(make_err)?;

        let mut cached_state = self.init_cached_state();

        let res = entrypoint
            .execute(&mut cached_state, &mut resources, &mut entry_point_execution_context)
            .map_err(TransactionExecutionError::ContractConstructorExecutionFailed)
            .map_err(make_err)?;

        Ok(res.execution.retdata.0.iter().map(ToFelt::to_felt).collect())
    }
}
