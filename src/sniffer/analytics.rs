//! Predictive analytics with EMA and atomic accumulator

use std::sync::atomic::{AtomicU64, Ordering};

/// Atomic wrapper for f64 using AtomicU64 with bit representation
/// Enables lock-free updates in the hot path
#[derive(Debug)]
pub struct AtomicF64 {
    bits: AtomicU64,
}

impl AtomicF64 {
    /// Create a new AtomicF64 with initial value
    pub fn new(value: f64) -> Self {
        Self {
            bits: AtomicU64::new(value.to_bits()),
        }
    }

    /// Load the current value
    #[inline(always)]
    pub fn load(&self, order: Ordering) -> f64 {
        f64::from_bits(self.bits.load(order))
    }

    /// Store a new value
    #[inline(always)]
    pub fn store(&self, value: f64, order: Ordering) {
        self.bits.store(value.to_bits(), order);
    }

    /// Swap the value and return the old value
    #[inline(always)]
    pub fn swap(&self, value: f64, order: Ordering) -> f64 {
        f64::from_bits(self.bits.swap(value.to_bits(), order))
    }
    
    /// Atomic fetch-add for f64 using compare-and-swap loop
    /// This is the hot-path operation for volume accumulation
    #[inline]
    pub fn fetch_add(&self, value: f64, order: Ordering) {
        let mut current = self.bits.load(order);
        loop {
            let current_val = f64::from_bits(current);
            let new_val = current_val + value;
            let new_bits = new_val.to_bits();
            
            match self.bits.compare_exchange_weak(
                current,
                new_bits,
                order,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current = x,
            }
        }
    }
}

impl Default for AtomicF64 {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// Threshold calculation factor - controls how acceleration ratio influences threshold
const THRESHOLD_ACCELERATION_FACTOR: f64 = 0.1;

/// Predictive analytics using EMA (Exponential Moving Average)
/// Lock-free implementation using atomic accumulators
#[derive(Debug)]
pub struct PredictiveAnalytics {
    /// Atomic accumulator for volume samples (hot path - lock-free)
    volume_accumulator: AtomicF64,
    /// Count of samples accumulated
    sample_count: AtomicU64,
    
    /// Short window EMA for volume (updated by background worker)
    short_window_ema: AtomicF64,
    /// Long window EMA for volume (updated by background worker)
    long_window_ema: AtomicF64,
    /// Dynamic threshold (updated atomically)
    threshold: AtomicF64,
    
    /// Smoothing factor for short window (typically 0.1-0.3)
    alpha_short: f64,
    /// Smoothing factor for long window (typically 0.01-0.05)
    alpha_long: f64,
}

impl PredictiveAnalytics {
    /// Create a new PredictiveAnalytics instance
    pub fn new(alpha_short: f64, alpha_long: f64, initial_threshold: f64) -> Self {
        Self {
            volume_accumulator: AtomicF64::new(0.0),
            sample_count: AtomicU64::new(0),
            short_window_ema: AtomicF64::new(0.0),
            long_window_ema: AtomicF64::new(0.0),
            threshold: AtomicF64::new(initial_threshold),
            alpha_short,
            alpha_long,
        }
    }
    
    /// HOT-PATH: Accumulate volume sample (lock-free atomic operation)
    /// This is called in the hot path for every transaction
    #[inline(always)]
    pub fn accumulate_volume(&self, volume: f64) {
        self.volume_accumulator.fetch_add(volume, Ordering::Relaxed);
        self.sample_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// BACKGROUND WORKER: Drain accumulator and update EMAs
    /// This is called periodically by a background task, NOT in the hot path
    pub fn update_ema(&self) {
        // Atomically drain the accumulator
        let accumulated_volume = self.volume_accumulator.swap(0.0, Ordering::Relaxed);
        let sample_count = self.sample_count.swap(0, Ordering::Relaxed);
        
        if sample_count == 0 {
            return;
        }
        
        // Calculate average volume per sample
        let avg_volume = accumulated_volume / sample_count as f64;
        
        // Update EMAs
        let short_ema = self.short_window_ema.load(Ordering::Relaxed);
        let long_ema = self.long_window_ema.load(Ordering::Relaxed);
        
        let new_short_ema = if short_ema == 0.0 {
            avg_volume
        } else {
            self.alpha_short * avg_volume + (1.0 - self.alpha_short) * short_ema
        };
        
        let new_long_ema = if long_ema == 0.0 {
            avg_volume
        } else {
            self.alpha_long * avg_volume + (1.0 - self.alpha_long) * long_ema
        };
        
        self.short_window_ema.store(new_short_ema, Ordering::Relaxed);
        self.long_window_ema.store(new_long_ema, Ordering::Relaxed);
    }
    
    /// BACKGROUND WORKER: Update threshold based on acceleration ratio
    /// This is called periodically by a background task, NOT in the hot path
    pub fn update_threshold(&self, threshold_update_rate: f64) {
        let short_ema = self.short_window_ema.load(Ordering::Relaxed);
        let long_ema = self.long_window_ema.load(Ordering::Relaxed);
        
        if long_ema == 0.0 {
            return;
        }
        
        // Calculate acceleration ratio
        let acceleration_ratio = short_ema / long_ema;
        
        // Adjust threshold based on acceleration
        let current_threshold = self.threshold.load(Ordering::Relaxed);
        let adjustment = (acceleration_ratio - 1.0) * THRESHOLD_ACCELERATION_FACTOR;
        let new_threshold = current_threshold + (adjustment * threshold_update_rate);
        
        // Clamp threshold to reasonable bounds (0.5 to 5.0)
        let clamped_threshold = new_threshold.max(0.5).min(5.0);
        
        self.threshold.store(clamped_threshold, Ordering::Relaxed);
    }
    
    /// HOT-PATH: Check if current metrics indicate high priority
    /// This is called in the hot path for classification
    #[inline(always)]
    pub fn is_high_priority(&self, volume_hint: f64) -> bool {
        let threshold = self.threshold.load(Ordering::Relaxed);
        let long_ema = self.long_window_ema.load(Ordering::Relaxed);
        
        if long_ema == 0.0 {
            return false;
        }
        
        volume_hint > (long_ema * threshold)
    }
    
    /// Get current EMA values (for monitoring)
    pub fn get_ema_values(&self) -> (f64, f64) {
        (
            self.short_window_ema.load(Ordering::Relaxed),
            self.long_window_ema.load(Ordering::Relaxed),
        )
    }
    
    /// Get current threshold value
    pub fn get_threshold(&self) -> f64 {
        self.threshold.load(Ordering::Relaxed)
    }
    
    /// Get acceleration ratio
    pub fn get_acceleration_ratio(&self) -> f64 {
        let short_ema = self.short_window_ema.load(Ordering::Relaxed);
        let long_ema = self.long_window_ema.load(Ordering::Relaxed);
        
        if long_ema == 0.0 {
            1.0
        } else {
            short_ema / long_ema
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_f64() {
        let atomic = AtomicF64::new(10.0);
        assert_eq!(atomic.load(Ordering::Relaxed), 10.0);
        
        atomic.store(20.0, Ordering::Relaxed);
        assert_eq!(atomic.load(Ordering::Relaxed), 20.0);
        
        atomic.fetch_add(5.0, Ordering::Relaxed);
        assert_eq!(atomic.load(Ordering::Relaxed), 25.0);
    }

    #[test]
    fn test_predictive_analytics() {
        let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);
        
        // Accumulate some volume
        analytics.accumulate_volume(100.0);
        analytics.accumulate_volume(150.0);
        analytics.accumulate_volume(200.0);
        
        // Update EMAs
        analytics.update_ema();
        
        let (short, long) = analytics.get_ema_values();
        assert!(short > 0.0);
        assert!(long > 0.0);
    }

    #[test]
    fn test_priority_classification() {
        let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);
        
        // Initialize with some baseline
        analytics.accumulate_volume(100.0);
        analytics.update_ema();
        
        // High volume should be high priority
        assert!(analytics.is_high_priority(200.0));
        
        // Low volume should be low priority
        assert!(!analytics.is_high_priority(50.0));
    }
}
