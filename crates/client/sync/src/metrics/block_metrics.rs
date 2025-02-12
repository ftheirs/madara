use dc_metrics::{Gauge, MetricsRegistry, PrometheusError, F64};

#[derive(Clone, Debug)]
pub struct BlockMetrics {
    // L2 network metrics
    pub l2_block_number: Gauge<F64>,
    pub l2_sync_time: Gauge<F64>,
    pub l2_avg_sync_time: Gauge<F64>,
    pub l2_latest_sync_time: Gauge<F64>,
    pub l2_state_size: Gauge<F64>,
    pub transaction_count: Gauge<F64>,
    pub event_count: Gauge<F64>,
    // L1 network metrics
    pub l1_block_number: Gauge<F64>,
    pub l1_gas_price_wei: Gauge<F64>,
    pub l1_gas_price_strk: Gauge<F64>,
}

impl BlockMetrics {
    pub fn register(registry: &MetricsRegistry) -> Result<Self, PrometheusError> {
        Ok(Self {
            l2_block_number: registry
                .register(Gauge::new("deoxys_l2_block_number", "Gauge for deoxys L2 block number")?)?,
            l2_sync_time: registry.register(Gauge::new("deoxys_l2_sync_time", "Gauge for deoxys L2 sync time")?)?,
            l2_avg_sync_time: registry
                .register(Gauge::new("deoxys_l2_avg_sync_time", "Gauge for deoxys L2 average sync time")?)?,
            l2_latest_sync_time: registry
                .register(Gauge::new("deoxys_l2_latest_sync_time", "Gauge for deoxys L2 latest sync time")?)?,
            l2_state_size: registry
                .register(Gauge::new("deoxys_l2_state_size", "Gauge for node storage usage in GB")?)?,
            l1_block_number: registry
                .register(Gauge::new("deoxys_l1_block_number", "Gauge for deoxys L1 block number")?)?,
            transaction_count: registry
                .register(Gauge::new("deoxys_transaction_count", "Gauge for deoxys transaction count")?)?,
            event_count: registry.register(Gauge::new("deoxys_event_count", "Gauge for deoxys event count")?)?,
            l1_gas_price_wei: registry.register(Gauge::new("deoxys_l1_gas_price", "Gauge for deoxys L1 gas price")?)?,
            l1_gas_price_strk: registry
                .register(Gauge::new("deoxys_l1_gas_price_strk", "Gauge for deoxys L1 gas price in strk")?)?,
        })
    }
}
