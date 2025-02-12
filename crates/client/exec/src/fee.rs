use crate::{ExecutionContext, ExecutionResult};
use blockifier::transaction::objects::FeeType;

impl<'a> ExecutionContext<'a> {
    pub fn execution_result_to_fee_estimate(
        &self,
        executions_result: &ExecutionResult,
    ) -> starknet_core::types::FeeEstimate {
        let gas_price =
            self.block_context.block_info().gas_prices.get_gas_price_by_fee_type(&executions_result.fee_type).get();
        let data_gas_price = self
            .block_context
            .block_info()
            .gas_prices
            .get_data_gas_price_by_fee_type(&executions_result.fee_type)
            .get();

        let data_gas_consumed = executions_result.execution_info.da_gas.l1_data_gas;
        let data_gas_fee = data_gas_consumed.saturating_mul(data_gas_price);
        let gas_consumed =
            executions_result.execution_info.actual_fee.0.saturating_sub(data_gas_fee) / gas_price.max(1);
        let minimal_gas_consumed = executions_result.minimal_l1_gas.unwrap_or_default().l1_gas;
        let minimal_data_gas_consumed = executions_result.minimal_l1_gas.unwrap_or_default().l1_data_gas;
        let gas_consumed = gas_consumed.max(minimal_gas_consumed);
        let data_gas_consumed = data_gas_consumed.max(minimal_data_gas_consumed);
        let overall_fee =
            gas_consumed.saturating_mul(gas_price).saturating_add(data_gas_consumed.saturating_mul(data_gas_price));

        let unit = match executions_result.fee_type {
            FeeType::Eth => starknet_core::types::PriceUnit::Wei,
            FeeType::Strk => starknet_core::types::PriceUnit::Fri,
        };
        starknet_core::types::FeeEstimate {
            gas_consumed: gas_consumed.into(),
            gas_price: gas_price.into(),
            data_gas_consumed: data_gas_consumed.into(),
            data_gas_price: data_gas_price.into(),
            overall_fee: overall_fee.into(),
            unit,
        }
    }
}
