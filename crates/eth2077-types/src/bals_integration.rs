use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BalsLayer {
    ExecutionLayer,
    ConsensusLayer,
    DataAvailabilityLayer,
    ToolingLayer,
    ValidatorServices,
    CrossLayerBridge,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IntegrationComplexity {
    Trivial,
    Low,
    Moderate,
    High,
    VeryHigh,
    Breaking,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProtocolImpact {
    NoChange,
    MinorAdjustment,
    NewEndpoint,
    ProtocolExtension,
    HardFork,
    FullRewrite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BalsFeature {
    ValidatorTriggeredExits,
    ExecutionLayerRequests,
    ConsolidationRequests,
    DepositProcessing,
    WithdrawalQueue,
    MaxEffectiveBalance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BalsIntegrationConfig {
    pub target_features: Vec<BalsFeature>,
    pub max_complexity: IntegrationComplexity,
    pub protocol_impact_budget: ProtocolImpact,
    pub validator_set_size: usize,
    pub max_requests_per_block: usize,
    pub queue_capacity: usize,
    pub activation_epoch_delay: u64,
    pub cross_layer_latency_budget_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BalsValidationError {
    EmptyFeatures,
    ValidatorSetTooSmall { size: usize },
    RequestsPerBlockZero,
    QueueCapacityZero,
    LatencyBudgetNonPositive { value: f64 },
    IncompatibleComplexityAndImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BalsIntegrationStats {
    pub total_implementation_effort: f64,
    pub cross_layer_latency_ms: f64,
    pub validator_throughput_impact: f64,
    pub protocol_upgrade_risk: f64,
    pub feature_coverage: f64,
    pub queue_utilization: f64,
    pub bottleneck: String,
    pub migration_caveats: Vec<String>,
}

pub fn default_bals_integration_config() -> BalsIntegrationConfig {
    BalsIntegrationConfig {
        target_features: all_bals_features().to_vec(),
        max_complexity: IntegrationComplexity::High,
        protocol_impact_budget: ProtocolImpact::ProtocolExtension,
        validator_set_size: 2_048,
        max_requests_per_block: 64,
        queue_capacity: 8_192,
        activation_epoch_delay: 2,
        cross_layer_latency_budget_ms: 220.0,
    }
}

pub fn validate_bals_config(
    config: &BalsIntegrationConfig,
) -> Result<(), Vec<BalsValidationError>> {
    let mut errors = Vec::new();

    if config.target_features.is_empty() {
        errors.push(BalsValidationError::EmptyFeatures);
    }

    if config.validator_set_size < 1_024 {
        errors.push(BalsValidationError::ValidatorSetTooSmall {
            size: config.validator_set_size,
        });
    }

    if config.max_requests_per_block == 0 {
        errors.push(BalsValidationError::RequestsPerBlockZero);
    }

    if config.queue_capacity == 0 {
        errors.push(BalsValidationError::QueueCapacityZero);
    }

    if !config.cross_layer_latency_budget_ms.is_finite()
        || config.cross_layer_latency_budget_ms <= 0.0
    {
        errors.push(BalsValidationError::LatencyBudgetNonPositive {
            value: config.cross_layer_latency_budget_ms,
        });
    }

    if !is_complexity_impact_compatible(config.max_complexity, config.protocol_impact_budget) {
        errors.push(BalsValidationError::IncompatibleComplexityAndImpact);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_bals_stats(config: &BalsIntegrationConfig) -> BalsIntegrationStats {
    let selected_features = unique_features(&config.target_features);
    if selected_features.is_empty() {
        return BalsIntegrationStats {
            total_implementation_effort: 0.0,
            cross_layer_latency_ms: 0.0,
            validator_throughput_impact: 0.0,
            protocol_upgrade_risk: 0.0,
            feature_coverage: 0.0,
            queue_utilization: 0.0,
            bottleneck: "None".to_string(),
            migration_caveats: vec!["No BALs features selected.".to_string()],
        };
    }

    let mut layer_totals = [0.0_f64; 6];
    for feature in &selected_features {
        let impact = feature_layer_impact(*feature);
        layer_totals[0] += impact[0];
        layer_totals[1] += impact[1];
        layer_totals[2] += impact[2];
        layer_totals[3] += impact[3];
        layer_totals[4] += impact[4];
        layer_totals[5] += impact[5];
    }

    let total_layer_pressure = layer_totals.iter().sum::<f64>();
    let complexity_multiplier = complexity_multiplier(config.max_complexity);
    let impact_multiplier = impact_multiplier(config.protocol_impact_budget);
    let validator_scale = (config.validator_set_size as f64 / 1_024.0).sqrt().max(1.0);

    let total_implementation_effort =
        total_layer_pressure * complexity_multiplier * impact_multiplier * validator_scale;

    let bridge_pressure = layer_totals[layer_index(BalsLayer::CrossLayerBridge)];
    let cross_layer_latency_ms = config.cross_layer_latency_budget_ms
        * (1.0 + 0.04 * bridge_pressure + 0.01 * config.activation_epoch_delay as f64);

    let validator_pressure = layer_totals[layer_index(BalsLayer::ValidatorServices)]
        + (0.5 * layer_totals[layer_index(BalsLayer::ConsensusLayer)]);
    let request_density = if config.validator_set_size > 0 {
        (config.max_requests_per_block as f64 / config.validator_set_size as f64) * 1_024.0
    } else {
        0.0
    };
    let validator_throughput_impact = (0.015 * validator_pressure * complexity_multiplier
        + 0.001 * request_density)
        .clamp(0.0, 1.0);

    let protocol_upgrade_risk = (0.12 * complexity_rank(config.max_complexity) as f64
        + 0.14 * protocol_impact_rank(config.protocol_impact_budget) as f64
        + 0.05 * selected_features.len() as f64)
        .clamp(0.0, 1.0);

    let feature_coverage =
        (selected_features.len() as f64 / all_bals_features().len() as f64).clamp(0.0, 1.0);

    let queue_demand =
        config.max_requests_per_block as f64 * (1.0 + config.activation_epoch_delay as f64 / 32.0);
    let queue_utilization = (queue_demand / config.queue_capacity.max(1) as f64).clamp(0.0, 1.0);

    let bottleneck = layer_totals
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| layer_name(layer_from_index(index)).to_string())
        .unwrap_or_else(|| "None".to_string());

    let mut migration_caveats = Vec::new();
    if protocol_impact_rank(config.protocol_impact_budget)
        >= protocol_impact_rank(ProtocolImpact::HardFork)
    {
        migration_caveats.push("Coordination-heavy network upgrade path is required.".to_string());
    }
    if validator_throughput_impact > 0.35 {
        migration_caveats.push(
            "Validator services may require scaling during peak BALs request load.".to_string(),
        );
    }
    if queue_utilization > 0.80 {
        migration_caveats.push(
            "Queue pressure is high; withdrawals and consolidations can backlog.".to_string(),
        );
    }
    if cross_layer_latency_ms > config.cross_layer_latency_budget_ms * 1.25 {
        migration_caveats.push(
            "Cross-layer message flow exceeds nominal latency budget under modeled pressure."
                .to_string(),
        );
    }
    if feature_coverage < 0.5 {
        migration_caveats
            .push("Partial BALs coverage may delay ecosystem-wide interoperability.".to_string());
    }
    if migration_caveats.is_empty() {
        migration_caveats.push("No major migration caveats detected.".to_string());
    }

    BalsIntegrationStats {
        total_implementation_effort,
        cross_layer_latency_ms,
        validator_throughput_impact,
        protocol_upgrade_risk,
        feature_coverage,
        queue_utilization,
        bottleneck,
        migration_caveats,
    }
}

pub fn compare_bals_features(
    config: &BalsIntegrationConfig,
) -> Vec<(String, BalsIntegrationStats)> {
    let mut compared = Vec::new();

    for feature in all_bals_features() {
        if config.target_features.contains(feature) {
            let mut variant = config.clone();
            variant.target_features = vec![*feature];
            compared.push((
                feature_name(*feature).to_string(),
                compute_bals_stats(&variant),
            ));
        }
    }

    compared
}

pub fn compute_bals_commitment(config: &BalsIntegrationConfig) -> [u8; 32] {
    let mut normalized_features = config.target_features.clone();
    normalized_features.sort_by_key(|feature| feature_byte(*feature));

    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::BALS_INTEGRATION::V1");
    hasher.update((normalized_features.len() as u64).to_le_bytes());
    for feature in normalized_features {
        hasher.update([feature_byte(feature)]);
    }
    hasher.update([complexity_byte(config.max_complexity)]);
    hasher.update([protocol_impact_byte(config.protocol_impact_budget)]);
    hasher.update((config.validator_set_size as u64).to_le_bytes());
    hasher.update((config.max_requests_per_block as u64).to_le_bytes());
    hasher.update((config.queue_capacity as u64).to_le_bytes());
    hasher.update(config.activation_epoch_delay.to_le_bytes());
    hasher.update(config.cross_layer_latency_budget_ms.to_le_bytes());

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn all_bals_features() -> &'static [BalsFeature] {
    &[
        BalsFeature::ValidatorTriggeredExits,
        BalsFeature::ExecutionLayerRequests,
        BalsFeature::ConsolidationRequests,
        BalsFeature::DepositProcessing,
        BalsFeature::WithdrawalQueue,
        BalsFeature::MaxEffectiveBalance,
    ]
}

fn unique_features(features: &[BalsFeature]) -> Vec<BalsFeature> {
    let mut unique = Vec::new();
    for feature in features {
        if !unique.contains(feature) {
            unique.push(*feature);
        }
    }
    unique
}

fn is_complexity_impact_compatible(
    complexity: IntegrationComplexity,
    impact: ProtocolImpact,
) -> bool {
    complexity_rank(complexity) >= complexity_rank(min_complexity_for_impact(impact))
}

fn min_complexity_for_impact(impact: ProtocolImpact) -> IntegrationComplexity {
    match impact {
        ProtocolImpact::NoChange => IntegrationComplexity::Trivial,
        ProtocolImpact::MinorAdjustment => IntegrationComplexity::Low,
        ProtocolImpact::NewEndpoint => IntegrationComplexity::Moderate,
        ProtocolImpact::ProtocolExtension => IntegrationComplexity::High,
        ProtocolImpact::HardFork => IntegrationComplexity::VeryHigh,
        ProtocolImpact::FullRewrite => IntegrationComplexity::Breaking,
    }
}

fn feature_layer_impact(feature: BalsFeature) -> [f64; 6] {
    match feature {
        BalsFeature::ValidatorTriggeredExits => [1.6, 1.8, 0.2, 1.1, 2.4, 1.3],
        BalsFeature::ExecutionLayerRequests => [2.7, 1.1, 0.5, 1.0, 1.0, 1.9],
        BalsFeature::ConsolidationRequests => [2.2, 1.6, 0.4, 1.2, 2.0, 2.1],
        BalsFeature::DepositProcessing => [1.5, 2.4, 0.8, 1.5, 2.2, 1.6],
        BalsFeature::WithdrawalQueue => [1.7, 2.0, 0.9, 1.4, 2.5, 1.8],
        BalsFeature::MaxEffectiveBalance => [1.2, 2.6, 0.3, 0.9, 2.3, 1.5],
    }
}

fn layer_index(layer: BalsLayer) -> usize {
    match layer {
        BalsLayer::ExecutionLayer => 0,
        BalsLayer::ConsensusLayer => 1,
        BalsLayer::DataAvailabilityLayer => 2,
        BalsLayer::ToolingLayer => 3,
        BalsLayer::ValidatorServices => 4,
        BalsLayer::CrossLayerBridge => 5,
    }
}

fn layer_from_index(index: usize) -> BalsLayer {
    match index {
        0 => BalsLayer::ExecutionLayer,
        1 => BalsLayer::ConsensusLayer,
        2 => BalsLayer::DataAvailabilityLayer,
        3 => BalsLayer::ToolingLayer,
        4 => BalsLayer::ValidatorServices,
        _ => BalsLayer::CrossLayerBridge,
    }
}

fn complexity_multiplier(complexity: IntegrationComplexity) -> f64 {
    match complexity {
        IntegrationComplexity::Trivial => 0.8,
        IntegrationComplexity::Low => 1.0,
        IntegrationComplexity::Moderate => 1.25,
        IntegrationComplexity::High => 1.6,
        IntegrationComplexity::VeryHigh => 2.0,
        IntegrationComplexity::Breaking => 2.6,
    }
}

fn impact_multiplier(impact: ProtocolImpact) -> f64 {
    match impact {
        ProtocolImpact::NoChange => 0.9,
        ProtocolImpact::MinorAdjustment => 1.0,
        ProtocolImpact::NewEndpoint => 1.2,
        ProtocolImpact::ProtocolExtension => 1.5,
        ProtocolImpact::HardFork => 1.9,
        ProtocolImpact::FullRewrite => 2.4,
    }
}

fn layer_name(layer: BalsLayer) -> &'static str {
    match layer {
        BalsLayer::ExecutionLayer => "ExecutionLayer",
        BalsLayer::ConsensusLayer => "ConsensusLayer",
        BalsLayer::DataAvailabilityLayer => "DataAvailabilityLayer",
        BalsLayer::ToolingLayer => "ToolingLayer",
        BalsLayer::ValidatorServices => "ValidatorServices",
        BalsLayer::CrossLayerBridge => "CrossLayerBridge",
    }
}

fn feature_name(feature: BalsFeature) -> &'static str {
    match feature {
        BalsFeature::ValidatorTriggeredExits => "ValidatorTriggeredExits",
        BalsFeature::ExecutionLayerRequests => "ExecutionLayerRequests",
        BalsFeature::ConsolidationRequests => "ConsolidationRequests",
        BalsFeature::DepositProcessing => "DepositProcessing",
        BalsFeature::WithdrawalQueue => "WithdrawalQueue",
        BalsFeature::MaxEffectiveBalance => "MaxEffectiveBalance",
    }
}

fn complexity_rank(complexity: IntegrationComplexity) -> usize {
    match complexity {
        IntegrationComplexity::Trivial => 0,
        IntegrationComplexity::Low => 1,
        IntegrationComplexity::Moderate => 2,
        IntegrationComplexity::High => 3,
        IntegrationComplexity::VeryHigh => 4,
        IntegrationComplexity::Breaking => 5,
    }
}

fn protocol_impact_rank(impact: ProtocolImpact) -> usize {
    match impact {
        ProtocolImpact::NoChange => 0,
        ProtocolImpact::MinorAdjustment => 1,
        ProtocolImpact::NewEndpoint => 2,
        ProtocolImpact::ProtocolExtension => 3,
        ProtocolImpact::HardFork => 4,
        ProtocolImpact::FullRewrite => 5,
    }
}

fn feature_byte(feature: BalsFeature) -> u8 {
    match feature {
        BalsFeature::ValidatorTriggeredExits => 0,
        BalsFeature::ExecutionLayerRequests => 1,
        BalsFeature::ConsolidationRequests => 2,
        BalsFeature::DepositProcessing => 3,
        BalsFeature::WithdrawalQueue => 4,
        BalsFeature::MaxEffectiveBalance => 5,
    }
}

fn complexity_byte(complexity: IntegrationComplexity) -> u8 {
    match complexity {
        IntegrationComplexity::Trivial => 0,
        IntegrationComplexity::Low => 1,
        IntegrationComplexity::Moderate => 2,
        IntegrationComplexity::High => 3,
        IntegrationComplexity::VeryHigh => 4,
        IntegrationComplexity::Breaking => 5,
    }
}

fn protocol_impact_byte(impact: ProtocolImpact) -> u8 {
    match impact {
        ProtocolImpact::NoChange => 0,
        ProtocolImpact::MinorAdjustment => 1,
        ProtocolImpact::NewEndpoint => 2,
        ProtocolImpact::ProtocolExtension => 3,
        ProtocolImpact::HardFork => 4,
        ProtocolImpact::FullRewrite => 5,
    }
}
