//! Public API and integration layer for the Sniffer

use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use super::config::SnifferConfig;
use super::telemetry::{SnifferMetrics, HandoffDiagnostics};
use super::analytics::PredictiveAnalytics;
use super::extractor::{PremintCandidate, PriorityLevel};
use super::prefilter;
use super::security;
use super::handoff;
use super::core;
use super::dataflow::SnifferEvent;
use super::supervisor::{Supervisor, WorkerHandle};

/// Public API trait for Sniffer operations
pub trait SnifferApi {
    /// Start the sniffer and return a receiver for candidates
    fn start(&self) -> impl std::future::Future<Output = Result<mpsc::Receiver<PremintCandidate>>> + Send;
    
    /// Stop the sniffer
    fn stop(&self);
    
    /// Pause the sniffer (stop producing candidates but keep connection alive)
    fn pause(&self);
    
    /// Resume the sniffer after pause
    fn resume(&self);
    
    /// Check health status
    fn health(&self) -> bool;
    
    /// Get current metrics snapshot
    fn stats(&self) -> String;
    
    /// Check if running
    fn is_running(&self) -> bool;
    
    /// Check if paused
    fn is_paused(&self) -> bool;
}

/// Event collector for telemetry
pub struct EventCollector {
    events: Arc<parking_lot::Mutex<Vec<SnifferEvent>>>,
    max_events: usize,
}

impl EventCollector {
    /// Create a new event collector
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(max_events))),
            max_events,
        }
    }

    /// Collect an event
    #[inline]
    pub fn collect(&self, event: SnifferEvent) {
        let mut events = self.events.lock();
        if events.len() >= self.max_events {
            events.remove(0);
        }
        events.push(event);
    }

    /// Get recent events
    pub fn get_recent(&self, count: usize) -> Vec<SnifferEvent> {
        let events = self.events.lock();
        let start = events.len().saturating_sub(count);
        events[start..].to_vec()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.lock().clear();
    }

    /// Get event count
    pub fn len(&self) -> usize {
        self.events.lock().len()
    }
}

/// Main Sniffer structure implementing the SnifferApi trait
pub struct Sniffer {
    config: SnifferConfig,
    metrics: Arc<SnifferMetrics>,
    analytics: Arc<PredictiveAnalytics>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    trace_id_counter: Arc<AtomicU64>,
    health_ok: Arc<AtomicBool>,
    event_collector: Arc<EventCollector>,
    handoff_diagnostics: Arc<HandoffDiagnostics>,
    supervisor: Arc<Supervisor>,
}

impl Sniffer {
    /// Create a new Sniffer instance
    pub fn new(config: SnifferConfig) -> Self {
        let analytics = Arc::new(PredictiveAnalytics::new(
            config.ema_alpha_short,
            config.ema_alpha_long,
            config.initial_threshold,
        ));

        Self {
            config,
            metrics: Arc::new(SnifferMetrics::new()),
            analytics,
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            trace_id_counter: Arc::new(AtomicU64::new(0)),
            health_ok: Arc::new(AtomicBool::new(true)),
            event_collector: Arc::new(EventCollector::new(10000)),
            handoff_diagnostics: Arc::new(HandoffDiagnostics::new()),
            supervisor: Arc::new(Supervisor::new()),
        }
    }

    /// Get current metrics reference
    pub fn get_metrics(&self) -> Arc<SnifferMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Get current analytics reference
    pub fn get_analytics(&self) -> Arc<PredictiveAnalytics> {
        Arc::clone(&self.analytics)
    }

    /// Get event collector reference
    pub fn get_event_collector(&self) -> Arc<EventCollector> {
        Arc::clone(&self.event_collector)
    }

    /// Get handoff diagnostics reference
    pub fn get_handoff_diagnostics(&self) -> Arc<HandoffDiagnostics> {
        Arc::clone(&self.handoff_diagnostics)
    }

    /// Get supervisor reference
    pub fn get_supervisor(&self) -> Arc<Supervisor> {
        Arc::clone(&self.supervisor)
    }

    /// Main processing loop (hot-path)
    async fn process_loop(
        config: SnifferConfig,
        tx: mpsc::Sender<PremintCandidate>,
        metrics: Arc<SnifferMetrics>,
        analytics: Arc<PredictiveAnalytics>,
        running: Arc<AtomicBool>,
        paused: Arc<AtomicBool>,
        trace_id_counter: Arc<AtomicU64>,
        health_ok: Arc<AtomicBool>,
        event_collector: Arc<EventCollector>,
        handoff_diagnostics: Arc<HandoffDiagnostics>,
    ) -> Result<()> {
        info!("Starting sniffer process loop");

        // Subscribe to stream with retry
        let mut stream = core::subscribe_with_retry(
            &config,
            Arc::clone(&running),
            Arc::clone(&metrics),
        ).await?;

        // Batch sender for efficient handoff with diagnostics
        let mut batch_sender = handoff::BatchSender::with_diagnostics(
            tx.clone(),
            config.batch_size,
            Duration::from_millis(config.batch_timeout_ms),
            Arc::clone(&metrics),
            Arc::clone(&handoff_diagnostics),
        );

        let mut last_batch_send = Instant::now();
        let batch_timeout = Duration::from_millis(config.batch_timeout_ms);
        
        // Create shutdown channel for deterministic shutdown
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
        
        // Spawn shutdown watcher
        let running_clone = Arc::clone(&running);
        tokio::spawn(async move {
            while running_clone.load(Ordering::Relaxed) {
                sleep(Duration::from_millis(100)).await;
            }
            let _ = shutdown_tx.send(()).await;
        });

        loop {
            // Use biased select! for deterministic shutdown priority
            tokio::select! {
                biased;
                
                // Highest priority: shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                
                // Check if paused
                _ = async {}, if paused.load(Ordering::Relaxed) => {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
                
                // Normal processing: receive transaction bytes
                tx_bytes_opt = stream.recv() => {
                    let tx_bytes = match tx_bytes_opt {
                        Some(bytes) => bytes,
                        None => {
                            // Stream ended, try to reconnect
                            warn!("Stream ended, attempting reconnection");
                            health_ok.store(false, Ordering::Relaxed);
                            stream = core::handle_reconnect(
                                &config,
                                Arc::clone(&running),
                                Arc::clone(&metrics),
                            ).await?;
                            health_ok.store(true, Ordering::Relaxed);
                            continue;
                        }
                    };
                    // HOT-PATH: Track processing latency
                    let processing_start = Instant::now();

                    // HOT-PATH: Increment counter
                    metrics.tx_seen.fetch_add(1, Ordering::Relaxed);

                    // Assign trace_id
                    let trace_id = trace_id_counter.fetch_add(1, Ordering::Relaxed);
                    
                    // Emit: BytesReceived event
                    event_collector.collect(SnifferEvent::BytesReceived {
                        trace_id,
                        timestamp: processing_start,
                        size: tx_bytes.len(),
                    });

                    // HOT-PATH: Quick security check
                    if !security::quick_sanity_check(&tx_bytes) {
                        metrics.security_drop_count.fetch_add(1, Ordering::Relaxed);
                        event_collector.collect(SnifferEvent::SecurityRejected {
                            trace_id,
                            timestamp: Instant::now(),
                            reason: "quick_sanity_check_failed",
                        });
                        continue;
                    }

                    // HOT-PATH: Prefilter (zero-copy)
                    let prefilter_start = Instant::now();
                    if !prefilter::should_process(&tx_bytes) {
                        metrics.tx_filtered.fetch_add(1, Ordering::Relaxed);
                        event_collector.collect(SnifferEvent::PrefilterRejected {
                            trace_id,
                            timestamp: Instant::now(),
                            reason: "filtered_out",
                        });
                        continue;
                    }
                    
                    // Emit: PrefilterPassed event
                    event_collector.collect(SnifferEvent::PrefilterPassed {
                        trace_id,
                        timestamp: Instant::now(),
                        latency_us: prefilter_start.elapsed().as_micros() as u64,
                    });
                    
                    // Accumulate volume for analytics (atomic operation)
                    let volume_hint = tx_bytes.len() as f64;
                    analytics.accumulate_volume(volume_hint);

                    // Determine priority based on analytics
                    let priority = if analytics.is_high_priority(volume_hint) {
                        PriorityLevel::High
                    } else {
                        PriorityLevel::Low
                    };

                    // Extract candidate
                    let extraction_start = Instant::now();
                    let candidate = match PremintCandidate::try_extract_candidate(
                        &tx_bytes,
                        trace_id,
                        volume_hint,
                        priority,
                        config.safe_offsets,
                    ) {
                        Ok(c) => {
                            // Emit: CandidateExtracted event
                            event_collector.collect(SnifferEvent::CandidateExtracted {
                                trace_id,
                                timestamp: Instant::now(),
                                latency_us: extraction_start.elapsed().as_micros() as u64,
                                priority,
                            });
                            c
                        },
                        Err(e) => {
                            debug!("Failed to extract candidate: {}", e);
                            event_collector.collect(SnifferEvent::ExtractionFailed {
                                trace_id,
                                timestamp: Instant::now(),
                                error: e.to_string(),
                            });
                            if e.to_string().contains("Mint") {
                                metrics.mint_extract_errors.fetch_add(1, Ordering::Relaxed);
                            } else {
                                metrics.account_extract_errors.fetch_add(1, Ordering::Relaxed);
                            }
                            continue;
                        }
                    };

                    // Inline security validation
                    let security_start = Instant::now();
                    if !security::is_valid_candidate(&candidate) {
                        metrics.security_drop_count.fetch_add(1, Ordering::Relaxed);
                        event_collector.collect(SnifferEvent::SecurityRejected {
                            trace_id,
                            timestamp: Instant::now(),
                            reason: "validation_failed",
                        });
                        continue;
                    }
                    
                    // Emit: SecurityPassed event
                    event_collector.collect(SnifferEvent::SecurityPassed {
                        trace_id,
                        timestamp: Instant::now(),
                        latency_us: security_start.elapsed().as_micros() as u64,
                    });

                    // HOT-PATH: Add to batch (note: candidate will be moved, not cloned)
                    batch_sender.add(candidate);

                    // Record latency
                    let latency_us = processing_start.elapsed().as_micros() as u64;
                    metrics.record_latency(latency_us);

                    // Check if we should flush batch due to timeout
                    if batch_sender.should_flush_timeout() {
                        batch_sender.flush_sync();
                    }
                }
            }
        }

        // Flush any remaining candidates
        batch_sender.flush_sync();
        info!("Sniffer process loop stopped");
        Ok(())
    }

    /// Telemetry export loop
    async fn telemetry_loop(
        metrics: Arc<SnifferMetrics>,
        running: Arc<AtomicBool>,
        interval_secs: u64,
    ) {
        let mut ticker = interval(Duration::from_secs(interval_secs));
        
        while running.load(Ordering::Relaxed) {
            ticker.tick().await;
            
            let snapshot = metrics.snapshot();
            info!("Sniffer metrics: {}", snapshot);
            
            // Calculate and log percentile latencies
            if let Some(p50) = metrics.get_percentile_latency(0.50) {
                debug!("P50 latency: {}μs", p50);
            }
            if let Some(p95) = metrics.get_percentile_latency(0.95) {
                debug!("P95 latency: {}μs", p95);
            }
            if let Some(p99) = metrics.get_percentile_latency(0.99) {
                debug!("P99 latency: {}μs", p99);
            }
        }
    }

    /// Analytics updater loop (background task)
    async fn analytics_updater_loop(
        analytics: Arc<PredictiveAnalytics>,
        running: Arc<AtomicBool>,
        interval_ms: u64,
    ) {
        let mut ticker = interval(Duration::from_millis(interval_ms));
        
        while running.load(Ordering::Relaxed) {
            ticker.tick().await;
            analytics.update_ema();
        }
    }

    /// Threshold updater loop (background task)
    async fn threshold_update_loop(
        analytics: Arc<PredictiveAnalytics>,
        running: Arc<AtomicBool>,
        config: SnifferConfig,
    ) {
        let mut ticker = interval(Duration::from_millis(config.ema_update_interval_ms * 2));
        
        while running.load(Ordering::Relaxed) {
            ticker.tick().await;
            analytics.update_threshold(config.threshold_update_rate);
            
            debug!(
                "EMA values: short={:.2}, long={:.2}, threshold={:.2}, ratio={:.2}",
                analytics.get_ema_values().0,
                analytics.get_ema_values().1,
                analytics.get_threshold(),
                analytics.get_acceleration_ratio(),
            );
        }
    }

    /// Config reload loop - watches for config file changes and applies updates
    async fn config_reload_loop(
        config_path: String,
        analytics: Arc<PredictiveAnalytics>,
        handoff_diagnostics: Arc<HandoffDiagnostics>,
        running: Arc<AtomicBool>,
    ) {
        use tokio::time::interval;
        
        let mut check_interval = interval(Duration::from_secs(5));
        let mut last_modified = std::fs::metadata(&config_path)
            .ok()
            .and_then(|m| m.modified().ok());

        while running.load(Ordering::Relaxed) {
            check_interval.tick().await;

            // Check if file was modified
            if let Ok(metadata) = std::fs::metadata(&config_path) {
                if let Ok(modified) = metadata.modified() {
                    if last_modified.map_or(true, |last| modified > last) {
                        last_modified = Some(modified);

                        // Try to reload config
                        if let Ok(new_config) = SnifferConfig::from_file(&config_path) {
                            info!("Configuration reloaded from {}", config_path);
                            
                            // Apply updates to analytics - update threshold
                            analytics.update_threshold(new_config.threshold_update_rate);
                            
                            // Note: In a real implementation, you'd want to update
                            // other runtime parameters here as well through a config
                            // update channel or similar mechanism
                            
                            debug!(
                                "Applied config updates: threshold_rate={:.2}, batch_size={}, drop_policy={:?}",
                                new_config.threshold_update_rate,
                                new_config.batch_size,
                                new_config.drop_policy,
                            );
                        } else {
                            warn!("Failed to reload configuration from {}", config_path);
                        }
                    }
                }
            }
        }
    }
}

impl SnifferApi for Sniffer {
    async fn start(&self) -> Result<mpsc::Receiver<PremintCandidate>> {
        // Validate configuration
        self.config.validate()?;
        
        self.running.store(true, Ordering::Release);
        self.health_ok.store(true, Ordering::Release);

        // Start supervisor
        self.supervisor.start().await?;

        let (tx, rx) = mpsc::channel(self.config.channel_capacity);

        // Clone Arc references for tasks
        let config = self.config.clone();
        let metrics = Arc::clone(&self.metrics);
        let analytics = Arc::clone(&self.analytics);
        let running = Arc::clone(&self.running);
        let paused = Arc::clone(&self.paused);
        let trace_id_counter = Arc::clone(&self.trace_id_counter);
        let health_ok = Arc::clone(&self.health_ok);
        let event_collector = Arc::clone(&self.event_collector);
        let handoff_diagnostics = Arc::clone(&self.handoff_diagnostics);
        let supervisor = Arc::clone(&self.supervisor);

        // Spawn main processing loop and register with supervisor
        let process_handle = tokio::spawn(async move {
            if let Err(e) = Self::process_loop(
                config.clone(),
                tx,
                metrics.clone(),
                analytics.clone(),
                running.clone(),
                paused.clone(),
                trace_id_counter.clone(),
                health_ok.clone(),
                event_collector.clone(),
                handoff_diagnostics.clone(),
            ).await {
                error!("Sniffer process loop error: {}", e);
            }
        });
        self.supervisor.register_worker(WorkerHandle::new(
            "process_loop".to_string(),
            process_handle,
            true, // critical worker
        )).await;

        // Spawn telemetry exporter and register with supervisor
        let metrics_clone = Arc::clone(&self.metrics);
        let running_clone = Arc::clone(&self.running);
        let telemetry_interval = self.config.telemetry_interval_secs;
        let telemetry_handle = tokio::spawn(async move {
            Self::telemetry_loop(metrics_clone, running_clone, telemetry_interval).await;
        });
        self.supervisor.register_worker(WorkerHandle::new(
            "telemetry_loop".to_string(),
            telemetry_handle,
            false, // non-critical worker
        )).await;

        // Spawn analytics_updater task and register with supervisor
        let analytics_clone = Arc::clone(&self.analytics);
        let running_clone = Arc::clone(&self.running);
        let ema_update_interval = self.config.ema_update_interval_ms;
        let analytics_handle = tokio::spawn(async move {
            Self::analytics_updater_loop(analytics_clone, running_clone, ema_update_interval).await;
        });
        self.supervisor.register_worker(WorkerHandle::new(
            "analytics_updater".to_string(),
            analytics_handle,
            false, // non-critical worker
        )).await;

        // Spawn threshold updater and register with supervisor
        let analytics_clone = Arc::clone(&self.analytics);
        let running_clone = Arc::clone(&self.running);
        let config_clone = self.config.clone();
        let threshold_handle = tokio::spawn(async move {
            Self::threshold_update_loop(analytics_clone, running_clone, config_clone).await;
        });
        self.supervisor.register_worker(WorkerHandle::new(
            "threshold_updater".to_string(),
            threshold_handle,
            false, // non-critical worker
        )).await;

        // Spawn config reload handler
        let config_path = self.config.config_file_path.clone();
        let analytics_clone = Arc::clone(&self.analytics);
        let handoff_diagnostics_clone = Arc::clone(&self.handoff_diagnostics);
        let running_clone = Arc::clone(&self.running);
        let config_reload_handle = tokio::spawn(async move {
            Self::config_reload_loop(
                config_path,
                analytics_clone,
                handoff_diagnostics_clone,
                running_clone,
            ).await;
        });
        self.supervisor.register_worker(WorkerHandle::new(
            "config_reload".to_string(),
            config_reload_handle,
            false, // non-critical worker
        )).await;

        Ok(rx)
    }

    fn stop(&self) {
        self.running.store(false, Ordering::Release);
        // Stop supervisor gracefully
        let supervisor = Arc::clone(&self.supervisor);
        tokio::spawn(async move {
            let _ = supervisor.stop(Duration::from_millis(5000)).await;
        });
        info!("Sniffer shutdown requested");
    }

    fn pause(&self) {
        self.paused.store(true, Ordering::Release);
        info!("Sniffer paused");
    }

    fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        info!("Sniffer resumed");
    }

    fn health(&self) -> bool {
        let is_running = self.running.load(Ordering::Relaxed);
        let is_healthy = self.health_ok.load(Ordering::Relaxed);
        let reconnects = self.metrics.reconnect_count.load(Ordering::Relaxed);
        
        is_running && is_healthy && reconnects < 10
    }

    fn stats(&self) -> String {
        self.metrics.snapshot()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sniffer_creation() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        assert!(!sniffer.is_running());
        assert!(!sniffer.is_paused());
    }

    #[tokio::test]
    async fn test_sniffer_start_stop() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        let rx = sniffer.start().await.unwrap();
        assert!(sniffer.is_running());
        
        sniffer.stop();
        // Give it a moment to stop
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!sniffer.is_running());
        
        drop(rx);
    }

    #[tokio::test]
    async fn test_sniffer_pause_resume() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        let _rx = sniffer.start().await.unwrap();
        
        sniffer.pause();
        assert!(sniffer.is_paused());
        
        sniffer.resume();
        assert!(!sniffer.is_paused());
        
        sniffer.stop();
    }
}
