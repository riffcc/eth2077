use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BuilderRole {
    TrustedBuilder,
    PermissionlessBuilder,
    SharedSequencer,
    MEVAuctioneer,
    BlockAuctionBuilder,
    InclusionListBuilder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SeparationModel {
    ProposerBuilderSplit,
    ExecutionTickets,
    AttesterProposerSplit,
    SlotAuction,
    CombinedPBS,
    HybridModel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MEVPolicy {
    MEVBurn,
    MEVSmoothing,
    MEVRedistribution,
    MEVMinimization,
    NoMEVIntervention,
    PartialMEVCapture,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OrderingGuarantee {
    FCFS,
    PriorityFee,
    InclusionListEnforced,
    FairOrdering,
    MEVAware,
    Randomized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpbsIntegrationConfig {
    pub separation_model: SeparationModel,
    pub builder_role: BuilderRole,
    pub mev_policy: MEVPolicy,
    pub ordering_guarantee: OrderingGuarantee,
    pub max_builder_slots: usize,
    pub slot_auction_reserve_gwei: u64,
    pub inclusion_list_size: usize,
    pub builder_collateral_eth: f64,
    pub censorship_resistance_target: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpbsValidationError {
    InvalidSlotCount,
    CollateralTooLow { value: f64 },
    CensorshipTargetOutOfRange { value: f64 },
    InclusionListTooLarge { size: usize, max: usize },
    IncompatibleModelAndRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpbsIntegrationStats {
    pub effective_block_value_gwei: f64,
    pub censorship_resistance_score: f64,
    pub builder_diversity_index: f64,
    pub mev_leakage_fraction: f64,
    pub ordering_fairness_score: f64,
    pub inclusion_guarantee_rate: f64,
    pub bottleneck: String,
    pub caveats: Vec<String>,
}

pub fn default_epbs_integration_config() -> EpbsIntegrationConfig {
    EpbsIntegrationConfig {
        separation_model: SeparationModel::CombinedPBS,
        builder_role: BuilderRole::PermissionlessBuilder,
        mev_policy: MEVPolicy::PartialMEVCapture,
        ordering_guarantee: OrderingGuarantee::InclusionListEnforced,
        max_builder_slots: 8,
        slot_auction_reserve_gwei: 25_000,
        inclusion_list_size: 256,
        builder_collateral_eth: 32.0,
        censorship_resistance_target: 0.85,
    }
}

pub fn validate_epbs_config(
    config: &EpbsIntegrationConfig,
) -> Result<(), Vec<EpbsValidationError>> {
    let mut errors = Vec::new();

    if config.max_builder_slots == 0 {
        errors.push(EpbsValidationError::InvalidSlotCount);
    }

    if config.builder_collateral_eth < 1.0 {
        errors.push(EpbsValidationError::CollateralTooLow {
            value: config.builder_collateral_eth,
        });
    }

    if config.censorship_resistance_target < 0.0 || config.censorship_resistance_target > 1.0 {
        errors.push(EpbsValidationError::CensorshipTargetOutOfRange {
            value: config.censorship_resistance_target,
        });
    }

    const MAX_INCLUSION_LIST: usize = 2048;
    if config.inclusion_list_size > MAX_INCLUSION_LIST {
        errors.push(EpbsValidationError::InclusionListTooLarge {
            size: config.inclusion_list_size,
            max: MAX_INCLUSION_LIST,
        });
    }

    if !is_model_role_compatible(config.separation_model, config.builder_role) {
        errors.push(EpbsValidationError::IncompatibleModelAndRole);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_epbs_stats(config: &EpbsIntegrationConfig) -> EpbsIntegrationStats {
    let (
        mut effective_block_value_gwei,
        mut censorship_resistance_score,
        mut builder_diversity_index,
        mut ordering_fairness_score,
        mut mev_leakage_fraction,
        model_bottleneck,
        model_caveat,
    ) = match config.separation_model {
        SeparationModel::ProposerBuilderSplit => (
            1_150_000.0,
            0.72,
            0.66,
            0.58,
            0.35,
            "relay centralization pressure",
            "Header/body split increases relay dependency during peak demand.",
        ),
        SeparationModel::ExecutionTickets => (
            1_020_000.0,
            0.84,
            0.79,
            0.69,
            0.28,
            "ticket liquidity fragmentation",
            "Ticket inventory can fragment if secondary markets are thin.",
        ),
        SeparationModel::AttesterProposerSplit => (
            980_000.0,
            0.88,
            0.74,
            0.77,
            0.24,
            "attester coordination overhead",
            "Attester/proposer duty separation raises coordination overhead.",
        ),
        SeparationModel::SlotAuction => (
            1_260_000.0,
            0.61,
            0.55,
            0.47,
            0.42,
            "auction winner concentration",
            "Repeated slot winners can concentrate ordering influence.",
        ),
        SeparationModel::CombinedPBS => (
            1_180_000.0,
            0.80,
            0.76,
            0.70,
            0.26,
            "cross-role coordination complexity",
            "Joint proposer-builder logic creates operational coupling.",
        ),
        SeparationModel::HybridModel => (
            1_120_000.0,
            0.86,
            0.82,
            0.75,
            0.22,
            "policy tuning complexity",
            "Hybrid composition needs careful policy tuning across modes.",
        ),
    };

    let mut caveats = vec![model_caveat.to_string()];

    match config.builder_role {
        BuilderRole::TrustedBuilder => {
            effective_block_value_gwei *= 1.05;
            censorship_resistance_score -= 0.12;
            builder_diversity_index *= 0.78;
            caveats
                .push("Trusted builder assumptions can weaken neutrality guarantees.".to_string());
        }
        BuilderRole::PermissionlessBuilder => {
            effective_block_value_gwei *= 0.98;
            censorship_resistance_score += 0.06;
            builder_diversity_index *= 1.12;
        }
        BuilderRole::SharedSequencer => {
            effective_block_value_gwei *= 0.94;
            censorship_resistance_score += 0.08;
            builder_diversity_index *= 1.07;
            ordering_fairness_score += 0.05;
            mev_leakage_fraction *= 0.93;
        }
        BuilderRole::MEVAuctioneer => {
            effective_block_value_gwei *= 1.12;
            censorship_resistance_score -= 0.08;
            builder_diversity_index *= 0.90;
            mev_leakage_fraction *= 1.08;
            caveats.push("Auctioneer dominance can amplify winner-take-all dynamics.".to_string());
        }
        BuilderRole::BlockAuctionBuilder => {
            effective_block_value_gwei *= 1.08;
            censorship_resistance_score -= 0.06;
            builder_diversity_index *= 0.87;
            caveats.push(
                "Block-level auctions can reduce long-tail builder participation.".to_string(),
            );
        }
        BuilderRole::InclusionListBuilder => {
            effective_block_value_gwei *= 0.97;
            censorship_resistance_score += 0.10;
            ordering_fairness_score += 0.09;
            mev_leakage_fraction *= 0.88;
        }
    }

    match config.mev_policy {
        MEVPolicy::MEVBurn => {
            effective_block_value_gwei *= 0.93;
            mev_leakage_fraction *= 0.60;
            ordering_fairness_score += 0.12;
            censorship_resistance_score += 0.03;
        }
        MEVPolicy::MEVSmoothing => {
            effective_block_value_gwei *= 0.96;
            mev_leakage_fraction *= 0.72;
            ordering_fairness_score += 0.10;
        }
        MEVPolicy::MEVRedistribution => {
            effective_block_value_gwei *= 0.95;
            mev_leakage_fraction *= 0.75;
            ordering_fairness_score += 0.11;
            builder_diversity_index *= 1.04;
        }
        MEVPolicy::MEVMinimization => {
            effective_block_value_gwei *= 0.90;
            mev_leakage_fraction *= 0.50;
            ordering_fairness_score += 0.14;
            censorship_resistance_score += 0.04;
        }
        MEVPolicy::NoMEVIntervention => {
            effective_block_value_gwei *= 1.04;
            mev_leakage_fraction *= 1.20;
            ordering_fairness_score -= 0.08;
            caveats.push(
                "No MEV intervention leaves leakage and extraction largely market-driven."
                    .to_string(),
            );
        }
        MEVPolicy::PartialMEVCapture => {
            mev_leakage_fraction *= 0.82;
            ordering_fairness_score += 0.05;
        }
    }

    match config.ordering_guarantee {
        OrderingGuarantee::FCFS => {
            effective_block_value_gwei *= 0.94;
            ordering_fairness_score += 0.07;
            censorship_resistance_score += 0.04;
            mev_leakage_fraction *= 0.92;
        }
        OrderingGuarantee::PriorityFee => {
            effective_block_value_gwei *= 1.06;
            ordering_fairness_score -= 0.10;
            censorship_resistance_score -= 0.05;
            mev_leakage_fraction *= 1.05;
        }
        OrderingGuarantee::InclusionListEnforced => {
            effective_block_value_gwei *= 0.97;
            ordering_fairness_score += 0.13;
            censorship_resistance_score += 0.10;
            mev_leakage_fraction *= 0.86;
        }
        OrderingGuarantee::FairOrdering => {
            effective_block_value_gwei *= 0.95;
            ordering_fairness_score += 0.15;
            censorship_resistance_score += 0.06;
            mev_leakage_fraction *= 0.90;
        }
        OrderingGuarantee::MEVAware => {
            effective_block_value_gwei *= 1.03;
            ordering_fairness_score += 0.03;
            mev_leakage_fraction *= 0.95;
        }
        OrderingGuarantee::Randomized => {
            effective_block_value_gwei *= 0.92;
            ordering_fairness_score += 0.10;
            censorship_resistance_score += 0.05;
            builder_diversity_index *= 1.02;
            mev_leakage_fraction *= 0.94;
        }
    }

    let reserve_signal = (config.slot_auction_reserve_gwei as f64 / 100_000.0).clamp(0.0, 1.0);
    effective_block_value_gwei *= 1.0 - (0.08 * reserve_signal);
    censorship_resistance_score += 0.03 * reserve_signal;

    let slot_signal = (config.max_builder_slots as f64).max(1.0).ln_1p();
    effective_block_value_gwei *= 1.0 + (slot_signal / 24.0);
    builder_diversity_index *= 0.90 + ((config.max_builder_slots as f64).sqrt().min(8.0) / 12.0);

    let collateral_signal = (config.builder_collateral_eth / 32.0).clamp(0.0, 3.0);
    censorship_resistance_score += 0.04 * collateral_signal;
    builder_diversity_index *= 1.0 - (0.05 * (collateral_signal - 1.0).max(0.0));

    let inclusion_ratio = (config.inclusion_list_size as f64 / 2048.0).clamp(0.0, 1.0);
    ordering_fairness_score +=
        (0.08 * inclusion_ratio) + ((config.censorship_resistance_target - 0.5) * 0.10);

    censorship_resistance_score =
        (0.75 * censorship_resistance_score) + (0.25 * config.censorship_resistance_target);
    let inclusion_guarantee_rate = clamp01(
        0.35 + (0.45 * inclusion_ratio)
            + (0.20 * ordering_fairness_score)
            + (0.15 * censorship_resistance_score),
    );

    if config.inclusion_list_size == 0 {
        caveats.push(
            "No inclusion list configured; liveness-sensitive transactions may be excluded."
                .to_string(),
        );
    }

    if config.max_builder_slots < 4 {
        caveats.push("Low max_builder_slots can concentrate order flow.".to_string());
    }

    if config.builder_collateral_eth > 64.0 {
        caveats
            .push("High builder collateral can reduce permissionless participation.".to_string());
    }

    if config.censorship_resistance_target > 0.90
        && matches!(config.ordering_guarantee, OrderingGuarantee::PriorityFee)
    {
        caveats.push(
            "High censorship resistance target is hard to meet under pure priority-fee ordering."
                .to_string(),
        );
    }

    let bottleneck = if config.max_builder_slots <= 2 {
        "limited builder slots".to_string()
    } else if config.builder_collateral_eth >= 128.0 {
        "collateral barrier to entry".to_string()
    } else {
        model_bottleneck.to_string()
    };

    EpbsIntegrationStats {
        effective_block_value_gwei: effective_block_value_gwei.max(1.0),
        censorship_resistance_score: clamp01(censorship_resistance_score),
        builder_diversity_index: clamp01(builder_diversity_index),
        mev_leakage_fraction: mev_leakage_fraction.clamp(0.01, 0.99),
        ordering_fairness_score: clamp01(ordering_fairness_score),
        inclusion_guarantee_rate,
        bottleneck,
        caveats,
    }
}

pub fn compare_separation_models(
    config: &EpbsIntegrationConfig,
) -> Vec<(String, EpbsIntegrationStats)> {
    all_separation_models()
        .iter()
        .map(|model| {
            let mut modeled = config.clone();
            modeled.separation_model = *model;
            (
                separation_model_name(*model).to_string(),
                compute_epbs_stats(&modeled),
            )
        })
        .collect()
}

pub fn compute_epbs_commitment(config: &EpbsIntegrationConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::EPBS_INTEGRATION::V1");
    hasher.update([separation_model_tag(config.separation_model)]);
    hasher.update([builder_role_tag(config.builder_role)]);
    hasher.update([mev_policy_tag(config.mev_policy)]);
    hasher.update([ordering_guarantee_tag(config.ordering_guarantee)]);
    hasher.update((config.max_builder_slots as u64).to_le_bytes());
    hasher.update(config.slot_auction_reserve_gwei.to_le_bytes());
    hasher.update((config.inclusion_list_size as u64).to_le_bytes());
    hasher.update(config.builder_collateral_eth.to_le_bytes());
    hasher.update(config.censorship_resistance_target.to_le_bytes());

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn is_model_role_compatible(model: SeparationModel, role: BuilderRole) -> bool {
    match (model, role) {
        (
            SeparationModel::ProposerBuilderSplit,
            BuilderRole::MEVAuctioneer | BuilderRole::BlockAuctionBuilder,
        ) => false,
        (SeparationModel::ExecutionTickets, BuilderRole::InclusionListBuilder) => false,
        (
            SeparationModel::AttesterProposerSplit,
            BuilderRole::MEVAuctioneer | BuilderRole::BlockAuctionBuilder,
        ) => false,
        (
            SeparationModel::SlotAuction,
            BuilderRole::InclusionListBuilder | BuilderRole::SharedSequencer,
        ) => false,
        _ => true,
    }
}

fn all_separation_models() -> [SeparationModel; 6] {
    [
        SeparationModel::ProposerBuilderSplit,
        SeparationModel::ExecutionTickets,
        SeparationModel::AttesterProposerSplit,
        SeparationModel::SlotAuction,
        SeparationModel::CombinedPBS,
        SeparationModel::HybridModel,
    ]
}

fn separation_model_name(model: SeparationModel) -> &'static str {
    match model {
        SeparationModel::ProposerBuilderSplit => "ProposerBuilderSplit",
        SeparationModel::ExecutionTickets => "ExecutionTickets",
        SeparationModel::AttesterProposerSplit => "AttesterProposerSplit",
        SeparationModel::SlotAuction => "SlotAuction",
        SeparationModel::CombinedPBS => "CombinedPBS",
        SeparationModel::HybridModel => "HybridModel",
    }
}

fn separation_model_tag(model: SeparationModel) -> u8 {
    match model {
        SeparationModel::ProposerBuilderSplit => 0,
        SeparationModel::ExecutionTickets => 1,
        SeparationModel::AttesterProposerSplit => 2,
        SeparationModel::SlotAuction => 3,
        SeparationModel::CombinedPBS => 4,
        SeparationModel::HybridModel => 5,
    }
}

fn builder_role_tag(role: BuilderRole) -> u8 {
    match role {
        BuilderRole::TrustedBuilder => 0,
        BuilderRole::PermissionlessBuilder => 1,
        BuilderRole::SharedSequencer => 2,
        BuilderRole::MEVAuctioneer => 3,
        BuilderRole::BlockAuctionBuilder => 4,
        BuilderRole::InclusionListBuilder => 5,
    }
}

fn mev_policy_tag(policy: MEVPolicy) -> u8 {
    match policy {
        MEVPolicy::MEVBurn => 0,
        MEVPolicy::MEVSmoothing => 1,
        MEVPolicy::MEVRedistribution => 2,
        MEVPolicy::MEVMinimization => 3,
        MEVPolicy::NoMEVIntervention => 4,
        MEVPolicy::PartialMEVCapture => 5,
    }
}

fn ordering_guarantee_tag(ordering: OrderingGuarantee) -> u8 {
    match ordering {
        OrderingGuarantee::FCFS => 0,
        OrderingGuarantee::PriorityFee => 1,
        OrderingGuarantee::InclusionListEnforced => 2,
        OrderingGuarantee::FairOrdering => 3,
        OrderingGuarantee::MEVAware => 4,
        OrderingGuarantee::Randomized => 5,
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}
