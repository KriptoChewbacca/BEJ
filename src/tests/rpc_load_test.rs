/// Load testing and benchmarking utilities for RPC Manager
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use tokio::sync::Semaphore;

/// Results from a load test
#[derive(Debug, Clone)]
pub struct LoadTestResults {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub duration_secs: f64,
    pub requests_per_second: f64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub errors_by_type: std::collections::HashMap<String, u64>,
}

/// Configuration for a load test
#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    /// Total number of requests to send
    pub total_requests: u64,
    
    /// Maximum concurrent requests
    pub max_concurrency: usize,
    
    /// Duration to run the test (if set, overrides total_requests)
    pub duration: Option<Duration>,
    
    /// Rate limit (requests per second, 0 = unlimited)
    pub rate_limit_rps: u64,
    
    /// Warmup period (not counted in results)
    pub warmup_duration: Duration,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            total_requests: 1000,
            max_concurrency: 100,
            duration: None,
            rate_limit_rps: 0,
            warmup_duration: Duration::from_secs(0),
        }
    }
}

/// Load test runner
pub struct LoadTestRunner {
    config: LoadTestConfig,
    latencies: Arc<parking_lot::Mutex<Vec<f64>>>,
    errors: Arc<parking_lot::Mutex<std::collections::HashMap<String, u64>>>,
    successful: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
}

impl LoadTestRunner {
    pub fn new(config: LoadTestConfig) -> Self {
        Self {
            config,
            latencies: Arc::new(parking_lot::Mutex::new(Vec::new())),
            errors: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
            successful: Arc::new(AtomicU64::new(0)),
            failed: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Run the load test with a given request function
    pub async fn run<F, Fut, T, E>(
        &self,
        mut request_fn: F,
    ) -> LoadTestResults
    where
        F: FnMut() -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = Result<T, E>> + Send,
        E: std::fmt::Display,
    {
        // Warmup phase
        if self.config.warmup_duration > Duration::from_secs(0) {
            println!("🔥 Warming up for {:?}...", self.config.warmup_duration);
            let warmup_start = Instant::now();
            while warmup_start.elapsed() < self.config.warmup_duration {
                let _ = request_fn().await;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        
        // Reset counters after warmup
        *self.latencies.lock() = Vec::new();
        *self.errors.lock() = std::collections::HashMap::new();
        self.successful.store(0, Ordering::Relaxed);
        self.failed.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.running.store(true, Ordering::Relaxed);
        
        println!("🚀 Starting load test...");
        println!("   Total requests: {}", self.config.total_requests);
        println!("   Max concurrency: {}", self.config.max_concurrency);
        if self.config.rate_limit_rps > 0 {
            println!("   Rate limit: {} req/s", self.config.rate_limit_rps);
        }
        
        let test_start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));
        let mut tasks = Vec::new();
        
        // Determine how many requests to send
        let num_requests = if let Some(duration) = self.config.duration {
            // Duration-based test
            u64::MAX // Will stop based on time
        } else {
            self.config.total_requests
        };
        
        for i in 0..num_requests {
            // Check if we should stop (duration-based test)
            if let Some(duration) = self.config.duration {
                if test_start.elapsed() >= duration {
                    break;
                }
            }
            
            // Rate limiting
            if self.config.rate_limit_rps > 0 {
                let interval = Duration::from_secs(1) / self.config.rate_limit_rps as u32;
                tokio::time::sleep(interval).await;
            }
            
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let latencies = self.latencies.clone();
            let errors = self.errors.clone();
            let successful = self.successful.clone();
            let failed = self.failed.clone();
            let total = self.total.clone();
            let running = self.running.clone();
            let mut request_fn_clone = request_fn.clone();
            
            let task = tokio::spawn(async move {
                if !running.load(Ordering::Relaxed) {
                    return;
                }
                
                let start = Instant::now();
                match request_fn_clone().await {
                    Ok(_) => {
                        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                        latencies.lock().push(latency_ms);
                        successful.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        let error_type = e.to_string();
                        let mut errors_map = errors.lock();
                        *errors_map.entry(error_type).or_insert(0) += 1;
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
                total.fetch_add(1, Ordering::Relaxed);
                drop(permit);
            });
            
            tasks.push(task);
            
            // Progress reporting
            if (i + 1) % 100 == 0 {
                let elapsed = test_start.elapsed().as_secs_f64();
                let current_total = self.total.load(Ordering::Relaxed);
                let current_rps = current_total as f64 / elapsed;
                println!("   Progress: {} requests, {:.1} req/s", current_total, current_rps);
            }
        }
        
        // Wait for all tasks to complete
        for task in tasks {
            let _ = task.await;
        }
        
        self.running.store(false, Ordering::Relaxed);
        let duration_secs = test_start.elapsed().as_secs_f64();
        
        // Calculate results
        self.calculate_results(duration_secs)
    }
    
    fn calculate_results(&self, duration_secs: f64) -> LoadTestResults {
        let total_requests = self.total.load(Ordering::Relaxed);
        let successful = self.successful.load(Ordering::Relaxed);
        let failed = self.failed.load(Ordering::Relaxed);
        
        let mut latencies = self.latencies.lock().clone();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let avg_latency = if !latencies.is_empty() {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        } else {
            0.0
        };
        
        let min_latency = latencies.first().copied().unwrap_or(0.0);
        let max_latency = latencies.last().copied().unwrap_or(0.0);
        
        let p50 = percentile(&latencies, 0.50);
        let p95 = percentile(&latencies, 0.95);
        let p99 = percentile(&latencies, 0.99);
        
        let results = LoadTestResults {
            total_requests,
            successful_requests: successful,
            failed_requests: failed,
            duration_secs,
            requests_per_second: total_requests as f64 / duration_secs,
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            errors_by_type: self.errors.lock().clone(),
        };
        
        self.print_results(&results);
        results
    }
    
    fn print_results(&self, results: &LoadTestResults) {
        println!("\n📊 Load Test Results");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Total Requests:     {}", results.total_requests);
        println!("Successful:         {} ({:.1}%)", 
            results.successful_requests,
            (results.successful_requests as f64 / results.total_requests as f64) * 100.0
        );
        println!("Failed:             {} ({:.1}%)", 
            results.failed_requests,
            (results.failed_requests as f64 / results.total_requests as f64) * 100.0
        );
        println!("Duration:           {:.2}s", results.duration_secs);
        println!("Throughput:         {:.1} req/s", results.requests_per_second);
        println!("\n📈 Latency Statistics (ms)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Average:            {:.2}", results.avg_latency_ms);
        println!("Minimum:            {:.2}", results.min_latency_ms);
        println!("Maximum:            {:.2}", results.max_latency_ms);
        println!("P50 (median):       {:.2}", results.p50_latency_ms);
        println!("P95:                {:.2}", results.p95_latency_ms);
        println!("P99:                {:.2}", results.p99_latency_ms);
        
        if !results.errors_by_type.is_empty() {
            println!("\n❌ Errors by Type");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            for (error_type, count) in &results.errors_by_type {
                println!("{:30} {}", error_type, count);
            }
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }
}

fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    
    let index = ((sorted_values.len() as f64 - 1.0) * p) as usize;
    sorted_values.get(index).copied().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_load_runner_basic() {
        let config = LoadTestConfig {
            total_requests: 100,
            max_concurrency: 10,
            duration: None,
            rate_limit_rps: 0,
            warmup_duration: Duration::from_secs(0),
        };
        
        let runner = LoadTestRunner::new(config);
        
        let results = runner.run(|| async {
            // Simulate a fast successful request
            tokio::time::sleep(Duration::from_millis(1)).await;
            Ok::<(), String>(())
        }).await;
        
        assert_eq!(results.total_requests, 100);
        assert_eq!(results.successful_requests, 100);
        assert_eq!(results.failed_requests, 0);
        assert!(results.requests_per_second > 0.0);
    }
    
    #[tokio::test]
    async fn test_load_runner_with_failures() {
        let config = LoadTestConfig {
            total_requests: 100,
            max_concurrency: 10,
            duration: None,
            rate_limit_rps: 0,
            warmup_duration: Duration::from_secs(0),
        };
        
        let runner = LoadTestRunner::new(config);
        let counter = Arc::new(AtomicU64::new(0));
        
        let results = runner.run(|| {
            let counter = counter.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::Relaxed);
                // Fail every 10th request
                if count % 10 == 0 {
                    Err("simulated error")
                } else {
                    Ok(())
                }
            }
        }).await;
        
        assert_eq!(results.total_requests, 100);
        assert!(results.failed_requests > 0);
        assert!(results.successful_requests > 0);
    }
    
    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        assert_eq!(percentile(&values, 0.50), 5.0);
        assert_eq!(percentile(&values, 0.95), 9.0);
        assert_eq!(percentile(&values, 0.99), 9.0);
        
        // Empty case
        assert_eq!(percentile(&[], 0.50), 0.0);
    }
}
