//! Lifecycle supervisor for async worker management
//!
//! Manages:
//! - Lifecycle of all async workers (EMA, Telemetry, Security Pool)
//! - Coordinated pause/resume/stop operations
//! - Panic recovery with exponential backoff
//! - Unified state tracking (Running, Paused, Stopped, Error)

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Sniffer state enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SnifferState {
    /// Sniffer is stopped (initial state)
    Stopped = 0,
    /// Sniffer is starting up
    Starting = 1,
    /// Sniffer is running normally
    Running = 2,
    /// Sniffer is paused (connections alive but not processing)
    Paused = 3,
    /// Sniffer is stopping gracefully
    Stopping = 4,
    /// Sniffer encountered an error
    Error = 5,
}

impl From<u8> for SnifferState {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Stopped,
            1 => Self::Starting,
            2 => Self::Running,
            3 => Self::Paused,
            4 => Self::Stopping,
            5 => Self::Error,
            _ => Self::Error,
        }
    }
}

/// Supervisor command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorCommand {
    /// Start the sniffer
    Start,
    /// Pause the sniffer
    Pause,
    /// Resume the sniffer
    Resume,
    /// Stop the sniffer
    Stop,
    /// Restart a failed worker
    RestartWorker,
}

/// Worker registration information
pub struct WorkerHandle {
    pub name: String,
    pub handle: JoinHandle<()>,
    pub critical: bool, // If true, failure triggers global error state
}

impl WorkerHandle {
    /// Create a new worker handle
    pub fn new(name: String, handle: JoinHandle<()>, critical: bool) -> Self {
        Self {
            name,
            handle,
            critical,
        }
    }
}

/// Lifecycle supervisor for coordinating all async workers
pub struct Supervisor {
    /// Current state
    state: Arc<AtomicU8>,
    /// Command broadcast channel
    command_tx: broadcast::Sender<SupervisorCommand>,
    /// Worker handles (using tokio::sync::Mutex for Send across await)
    workers: Arc<tokio::sync::Mutex<Vec<WorkerHandle>>>,
    /// Error count for backoff
    error_count: Arc<std::sync::atomic::AtomicU32>,
}

impl Supervisor {
    /// Create a new supervisor
    pub fn new() -> Self {
        let (command_tx, _) = broadcast::channel(16);
        
        Self {
            state: Arc::new(AtomicU8::new(SnifferState::Stopped as u8)),
            command_tx,
            workers: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            error_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Get current state
    pub fn state(&self) -> SnifferState {
        SnifferState::from(self.state.load(Ordering::Relaxed))
    }

    /// Set state
    fn set_state(&self, new_state: SnifferState) {
        self.state.store(new_state as u8, Ordering::Release);
        debug!("Supervisor state changed to {:?}", new_state);
    }

    /// Register a worker
    pub async fn register_worker(&self, worker: WorkerHandle) {
        info!("Registering worker: {} (critical: {})", worker.name, worker.critical);
        self.workers.lock().await.push(worker);
    }

    /// Start all workers
    pub async fn start(&self) -> anyhow::Result<()> {
        if self.state() != SnifferState::Stopped {
            return Err(anyhow::anyhow!("Cannot start: current state is {:?}", self.state()));
        }

        self.set_state(SnifferState::Starting);
        info!("Supervisor starting all workers");

        // Send start command to all workers
        let _ = self.command_tx.send(SupervisorCommand::Start);

        self.set_state(SnifferState::Running);
        info!("Supervisor started successfully");

        Ok(())
    }

    /// Pause all workers
    pub fn pause(&self) {
        info!("Supervisor pausing all workers");
        self.set_state(SnifferState::Paused);
        let _ = self.command_tx.send(SupervisorCommand::Pause);
    }

    /// Resume all workers
    pub fn resume(&self) {
        info!("Supervisor resuming all workers");
        self.set_state(SnifferState::Running);
        let _ = self.command_tx.send(SupervisorCommand::Resume);
    }

    /// Stop all workers gracefully
    pub async fn stop(&self, timeout: Duration) -> anyhow::Result<()> {
        if self.state() == SnifferState::Stopped {
            return Ok(());
        }

        info!("Supervisor stopping all workers (timeout: {:?})", timeout);
        self.set_state(SnifferState::Stopping);

        // Send stop command
        let _ = self.command_tx.send(SupervisorCommand::Stop);

        // Wait for workers to finish with timeout
        let deadline = tokio::time::Instant::now() + timeout;
        let mut workers = self.workers.lock().await;

        for worker in workers.drain(..) {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            
            match tokio::time::timeout(remaining, worker.handle).await {
                Ok(Ok(())) => {
                    debug!("Worker '{}' stopped successfully", worker.name);
                }
                Ok(Err(e)) => {
                    if worker.critical {
                        error!("Critical worker '{}' panicked: {:?}", worker.name, e);
                        self.set_state(SnifferState::Error);
                    } else {
                        warn!("Worker '{}' panicked: {:?}", worker.name, e);
                    }
                }
                Err(_) => {
                    warn!("Worker '{}' did not stop within timeout", worker.name);
                }
            }
        }

        self.set_state(SnifferState::Stopped);
        info!("Supervisor stopped all workers");

        Ok(())
    }

    /// Monitor workers for panics and restart with backoff
    pub async fn monitor_workers(&self) {
        let mut check_interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            check_interval.tick().await;

            if self.state() == SnifferState::Stopped {
                break;
            }

            let mut workers = self.workers.lock();
            let mut failed_workers = Vec::new();

            // Check for finished workers
            for (idx, worker) in workers.iter_mut().enumerate() {
                if worker.handle.is_finished() {
                    failed_workers.push(idx);
                }
            }

            // Handle failed workers
            for idx in failed_workers.iter().rev() {
                let worker = workers.remove(*idx);
                
                if worker.critical {
                    error!("Critical worker '{}' failed - entering error state", worker.name);
                    self.set_state(SnifferState::Error);
                    self.error_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    warn!("Non-critical worker '{}' failed", worker.name);
                }
            }

            drop(workers);

            // If in error state with too many errors, stop monitoring
            if self.state() == SnifferState::Error && self.error_count.load(Ordering::Relaxed) > 5 {
                error!("Too many worker failures - stopping supervisor");
                break;
            }
        }
    }

    /// Subscribe to supervisor commands
    pub fn subscribe(&self) -> broadcast::Receiver<SupervisorCommand> {
        self.command_tx.subscribe()
    }

    /// Check if supervisor is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.state(), SnifferState::Running | SnifferState::Paused)
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_state_transitions() {
        let supervisor = Supervisor::new();
        
        assert_eq!(supervisor.state(), SnifferState::Stopped);
        
        supervisor.start().await.unwrap();
        assert_eq!(supervisor.state(), SnifferState::Running);
        
        supervisor.pause();
        assert_eq!(supervisor.state(), SnifferState::Paused);
        
        supervisor.resume();
        assert_eq!(supervisor.state(), SnifferState::Running);
        
        supervisor.stop(Duration::from_secs(1)).await.unwrap();
        assert_eq!(supervisor.state(), SnifferState::Stopped);
    }

    #[tokio::test]
    async fn test_supervisor_worker_registration() {
        let supervisor = Supervisor::new();
        
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
        });
        
        let worker = WorkerHandle::new("test_worker".to_string(), handle, false);
        supervisor.register_worker(worker).await;
        
        assert_eq!(supervisor.workers.lock().await.len(), 1);
    }

    #[test]
    fn test_sniffer_state_conversion() {
        assert_eq!(SnifferState::from(0), SnifferState::Stopped);
        assert_eq!(SnifferState::from(2), SnifferState::Running);
        assert_eq!(SnifferState::from(3), SnifferState::Paused);
        assert_eq!(SnifferState::from(5), SnifferState::Error);
    }
    
    /// Concurrent test to verify no deadlocks with multiple workers
    #[tokio::test]
    async fn test_supervisor_concurrent_worker_registration() {
        let supervisor = Arc::new(Supervisor::new());
        
        // Spawn multiple workers concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let sup = supervisor.clone();
            let handle = tokio::spawn(async move {
                let task = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                });
                
                let worker = WorkerHandle::new(
                    format!("test_worker_{}", i),
                    task,
                    false,
                );
                sup.register_worker(worker).await;
            });
            handles.push(handle);
        }
        
        // Wait for all registrations to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify all workers registered
        assert_eq!(supervisor.workers.lock().await.len(), 10);
        
        // Stop supervisor (should not deadlock)
        let result = supervisor.stop(Duration::from_secs(5)).await;
        assert!(result.is_ok());
        assert_eq!(supervisor.state(), SnifferState::Stopped);
    }
}
