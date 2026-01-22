//! Load Balancing Algorithms
//!
//! Provides multiple load balancing algorithms for distributing traffic
//! across providers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Load balancing algorithm types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancerAlgorithm {
    /// Use highest priority provider; fall back on failure.
    Failover,
    /// Distribute evenly across healthy providers.
    RoundRobin,
    /// Distribute based on weight values.
    Weighted,
    /// Route to provider with fewest active connections.
    LeastConnections,
}

impl LoadBalancerAlgorithm {
    /// Parse algorithm from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "failover" => Some(LoadBalancerAlgorithm::Failover),
            "round_robin" | "roundrobin" => Some(LoadBalancerAlgorithm::RoundRobin),
            "weighted" => Some(LoadBalancerAlgorithm::Weighted),
            "least_connections" | "leastconnections" => {
                Some(LoadBalancerAlgorithm::LeastConnections)
            }
            _ => None,
        }
    }
}

/// Provider information for load balancing.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// Provider ID.
    pub id: String,
    /// Priority (lower = higher priority).
    pub priority: u32,
    /// Weight for weighted load balancing.
    pub weight: u32,
}

/// Load balancer for distributing traffic across providers.
pub struct LoadBalancer {
    /// The algorithm to use.
    algorithm: LoadBalancerAlgorithm,
    /// Current index for round-robin.
    round_robin_index: AtomicUsize,
    /// Weighted selection state.
    weighted_state: RwLock<WeightedState>,
    /// Connection counts per provider.
    connection_counts: RwLock<HashMap<String, AtomicU32>>,
}

/// State for weighted load balancing.
struct WeightedState {
    /// Current weights (cumulative).
    cumulative_weights: Vec<(String, u32)>,
    /// Total weight.
    total_weight: u32,
    /// Current selection index.
    current_index: usize,
    /// Current weight counter.
    current_weight: u32,
}

impl Default for WeightedState {
    fn default() -> Self {
        Self {
            cumulative_weights: Vec::new(),
            total_weight: 0,
            current_index: 0,
            current_weight: 0,
        }
    }
}

impl LoadBalancer {
    /// Create a new load balancer with the given algorithm.
    pub fn new(algorithm: LoadBalancerAlgorithm) -> Self {
        Self {
            algorithm,
            round_robin_index: AtomicUsize::new(0),
            weighted_state: RwLock::new(WeightedState::default()),
            connection_counts: RwLock::new(HashMap::new()),
        }
    }

    /// Create a load balancer from a string algorithm name.
    pub fn from_algorithm_str(algorithm: &str) -> Self {
        let algo = LoadBalancerAlgorithm::from_str(algorithm)
            .unwrap_or(LoadBalancerAlgorithm::Failover);
        Self::new(algo)
    }

    /// Get the algorithm being used.
    pub fn algorithm(&self) -> LoadBalancerAlgorithm {
        self.algorithm
    }

    /// Select a provider from the given list.
    ///
    /// # Arguments
    /// * `providers` - List of provider info (must be non-empty and pre-filtered for health)
    ///
    /// # Returns
    /// The ID of the selected provider, or None if no providers available.
    pub fn select(&self, providers: &[ProviderInfo]) -> Option<String> {
        if providers.is_empty() {
            return None;
        }

        match self.algorithm {
            LoadBalancerAlgorithm::Failover => self.select_failover(providers),
            LoadBalancerAlgorithm::RoundRobin => self.select_round_robin(providers),
            LoadBalancerAlgorithm::Weighted => self.select_weighted(providers),
            LoadBalancerAlgorithm::LeastConnections => self.select_least_connections(providers),
        }
    }

    /// Failover: select highest priority (lowest number) provider.
    fn select_failover(&self, providers: &[ProviderInfo]) -> Option<String> {
        providers
            .iter()
            .min_by_key(|p| p.priority)
            .map(|p| p.id.clone())
    }

    /// Round-robin: cycle through providers.
    fn select_round_robin(&self, providers: &[ProviderInfo]) -> Option<String> {
        let index = self.round_robin_index.fetch_add(1, Ordering::SeqCst);
        let provider_index = index % providers.len();
        Some(providers[provider_index].id.clone())
    }

    /// Weighted: select based on weight values using smooth weighted round-robin.
    fn select_weighted(&self, providers: &[ProviderInfo]) -> Option<String> {
        // Filter out providers with zero weight
        let weighted_providers: Vec<_> = providers.iter().filter(|p| p.weight > 0).collect();

        if weighted_providers.is_empty() {
            // Fall back to failover if all weights are zero
            return self.select_failover(providers);
        }

        // Simple weighted selection using cumulative weights
        let total_weight: u32 = weighted_providers.iter().map(|p| p.weight).sum();
        if total_weight == 0 {
            return self.select_failover(providers);
        }

        // Use atomic counter for smooth distribution
        let counter = self.round_robin_index.fetch_add(1, Ordering::SeqCst) as u32;
        let selection_point = counter % total_weight;

        let mut cumulative = 0u32;
        for provider in &weighted_providers {
            cumulative += provider.weight;
            if selection_point < cumulative {
                return Some(provider.id.clone());
            }
        }

        // Fallback (shouldn't reach here)
        weighted_providers.last().map(|p| p.id.clone())
    }

    /// Least connections: select provider with fewest active connections.
    fn select_least_connections(&self, providers: &[ProviderInfo]) -> Option<String> {
        let counts = self.connection_counts.read().ok()?;

        providers
            .iter()
            .min_by_key(|p| {
                counts
                    .get(&p.id)
                    .map(|c| c.load(Ordering::SeqCst))
                    .unwrap_or(0)
            })
            .map(|p| p.id.clone())
    }

    /// Record a connection starting (for least_connections algorithm).
    pub fn connection_start(&self, provider_id: &str) {
        if let Ok(mut counts) = self.connection_counts.write() {
            counts
                .entry(provider_id.to_string())
                .or_insert_with(|| AtomicU32::new(0))
                .fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Record a connection ending (for least_connections algorithm).
    pub fn connection_end(&self, provider_id: &str) {
        if let Ok(counts) = self.connection_counts.read() {
            if let Some(count) = counts.get(provider_id) {
                // Use saturating_sub to avoid underflow
                let current = count.load(Ordering::SeqCst);
                if current > 0 {
                    count.store(current - 1, Ordering::SeqCst);
                }
            }
        }
    }

    /// Get connection count for a provider.
    pub fn get_connection_count(&self, provider_id: &str) -> u32 {
        self.connection_counts
            .read()
            .ok()
            .and_then(|counts| counts.get(provider_id).map(|c| c.load(Ordering::SeqCst)))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_providers() -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                id: "provider_1".to_string(),
                priority: 1,
                weight: 100,
            },
            ProviderInfo {
                id: "provider_2".to_string(),
                priority: 2,
                weight: 50,
            },
            ProviderInfo {
                id: "provider_3".to_string(),
                priority: 3,
                weight: 25,
            },
        ]
    }

    #[test]
    fn test_algorithm_from_str() {
        assert_eq!(
            LoadBalancerAlgorithm::from_str("failover"),
            Some(LoadBalancerAlgorithm::Failover)
        );
        assert_eq!(
            LoadBalancerAlgorithm::from_str("round_robin"),
            Some(LoadBalancerAlgorithm::RoundRobin)
        );
        assert_eq!(
            LoadBalancerAlgorithm::from_str("weighted"),
            Some(LoadBalancerAlgorithm::Weighted)
        );
        assert_eq!(
            LoadBalancerAlgorithm::from_str("least_connections"),
            Some(LoadBalancerAlgorithm::LeastConnections)
        );
        assert_eq!(LoadBalancerAlgorithm::from_str("invalid"), None);
    }

    #[test]
    fn test_failover_selects_highest_priority() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::Failover);
        let providers = create_providers();

        // Should always select provider_1 (priority 1)
        for _ in 0..10 {
            assert_eq!(lb.select(&providers), Some("provider_1".to_string()));
        }
    }

    #[test]
    fn test_failover_with_different_priorities() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::Failover);
        let providers = vec![
            ProviderInfo {
                id: "high".to_string(),
                priority: 2,
                weight: 100,
            },
            ProviderInfo {
                id: "low".to_string(),
                priority: 1,
                weight: 100,
            },
        ];

        // Should select "low" because it has priority 1
        assert_eq!(lb.select(&providers), Some("low".to_string()));
    }

    #[test]
    fn test_round_robin_cycles() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::RoundRobin);
        let providers = create_providers();

        let selected: Vec<_> = (0..6).map(|_| lb.select(&providers).unwrap()).collect();

        // Should cycle through all providers
        assert_eq!(selected[0], "provider_1");
        assert_eq!(selected[1], "provider_2");
        assert_eq!(selected[2], "provider_3");
        assert_eq!(selected[3], "provider_1");
        assert_eq!(selected[4], "provider_2");
        assert_eq!(selected[5], "provider_3");
    }

    #[test]
    fn test_weighted_distribution() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::Weighted);
        let providers = vec![
            ProviderInfo {
                id: "heavy".to_string(),
                priority: 1,
                weight: 75,
            },
            ProviderInfo {
                id: "light".to_string(),
                priority: 1,
                weight: 25,
            },
        ];

        // Run many selections
        let mut counts: HashMap<String, usize> = HashMap::new();
        for _ in 0..1000 {
            let selected = lb.select(&providers).unwrap();
            *counts.entry(selected).or_insert(0) += 1;
        }

        // heavy should be selected roughly 3x more than light
        let heavy_count = *counts.get("heavy").unwrap_or(&0);
        let light_count = *counts.get("light").unwrap_or(&0);

        // Allow for some variance (heavy should be between 2x and 4x light)
        let ratio = heavy_count as f64 / light_count as f64;
        assert!(ratio > 2.0 && ratio < 4.0, "Ratio was {}", ratio);
    }

    #[test]
    fn test_weighted_with_zero_weights_falls_back() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::Weighted);
        let providers = vec![
            ProviderInfo {
                id: "zero".to_string(),
                priority: 2,
                weight: 0,
            },
            ProviderInfo {
                id: "also_zero".to_string(),
                priority: 1,
                weight: 0,
            },
        ];

        // Should fall back to failover
        let selected = lb.select(&providers).unwrap();
        assert_eq!(selected, "also_zero"); // Lower priority number wins
    }

    #[test]
    fn test_least_connections() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::LeastConnections);
        let providers = create_providers();

        // All start at 0 connections, should select first
        let first = lb.select(&providers).unwrap();

        // Add connections to first provider
        lb.connection_start(&first);
        lb.connection_start(&first);

        // Now should select a different provider
        let second = lb.select(&providers).unwrap();
        assert_ne!(first, second);

        // End connections
        lb.connection_end(&first);
        lb.connection_end(&first);

        // Should go back to first
        let third = lb.select(&providers).unwrap();
        assert_eq!(first, third);
    }

    #[test]
    fn test_empty_providers() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::Failover);
        let providers: Vec<ProviderInfo> = vec![];

        assert_eq!(lb.select(&providers), None);
    }

    #[test]
    fn test_connection_count() {
        let lb = LoadBalancer::new(LoadBalancerAlgorithm::LeastConnections);

        lb.connection_start("provider_1");
        lb.connection_start("provider_1");
        lb.connection_start("provider_2");

        assert_eq!(lb.get_connection_count("provider_1"), 2);
        assert_eq!(lb.get_connection_count("provider_2"), 1);
        assert_eq!(lb.get_connection_count("provider_3"), 0);

        lb.connection_end("provider_1");
        assert_eq!(lb.get_connection_count("provider_1"), 1);
    }

    #[test]
    fn test_from_algorithm_str() {
        let lb = LoadBalancer::from_algorithm_str("round_robin");
        assert_eq!(lb.algorithm(), LoadBalancerAlgorithm::RoundRobin);

        let lb = LoadBalancer::from_algorithm_str("invalid");
        assert_eq!(lb.algorithm(), LoadBalancerAlgorithm::Failover); // Default
    }
}
