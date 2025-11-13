//! Comprehensive benchmarks for TX Builder nonce management performance
//!
//! Task 4 requirement: Measure performance overhead with p95 target < 5ms
//!
//! Benchmarks:
//! - Nonce acquisition and release
//! - Transaction building with nonce
//! - RAII guard lifecycle
//! - Instruction ordering overhead
//!
//! Note: These benchmarks use synchronous wrappers around async operations
//! to work with Criterion's benchmarking framework.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{v0::Message as MessageV0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};
use std::sync::Arc;
use std::time::Duration;

// Import from the library
use bot::nonce_manager::{LocalSigner, UniverseNonceManager};

/// Helper: Create test nonce manager with specified pool size (blocking version for benchmarks)
fn create_test_nonce_manager_blocking(pool_size: usize) -> Arc<UniverseNonceManager> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let signer = Arc::new(LocalSigner::new(Keypair::new()));
        let mut nonce_accounts = vec![];
        for _ in 0..pool_size {
            nonce_accounts.push(Pubkey::new_unique());
        }

        UniverseNonceManager::new_for_testing(signer, nonce_accounts, Duration::from_secs(300))
            .await
    })
}

/// Helper: Build a complete transaction with nonce
fn build_transaction_with_nonce(
    nonce_account: &Pubkey,
    nonce_authority: &Keypair,
    nonce_blockhash: Hash,
    payer: &Keypair,
) -> VersionedTransaction {
    let mut instructions = vec![];

    // 1. advance_nonce instruction (MUST BE FIRST)
    instructions.push(system_instruction::advance_nonce_account(
        nonce_account,
        &nonce_authority.pubkey(),
    ));

    // 2. Compute budget instructions
    instructions.push(Instruction::new_with_bytes(
        solana_sdk::compute_budget::id(),
        &[2, 0, 0, 0, 0, 200, 0, 0], // set_compute_unit_limit
        vec![],
    ));

    // 3. Simple transfer instruction
    instructions.push(system_instruction::transfer(
        &payer.pubkey(),
        &Pubkey::new_unique(),
        1_000_000,
    ));

    let message = MessageV0::try_compile(
        &payer.pubkey(),
        &instructions,
        &[],
        nonce_blockhash,
    )
    .unwrap();

    let signers: Vec<&dyn Signer> = if payer.pubkey() == nonce_authority.pubkey() {
        vec![payer]
    } else {
        vec![payer, nonce_authority]
    };

    VersionedTransaction::try_new(VersionedMessage::V0(message), &signers).unwrap()
}

/// Benchmark: Nonce acquisition overhead
fn bench_nonce_acquisition(c: &mut Criterion) {
    let nonce_manager = create_test_nonce_manager_blocking(20);

    c.bench_function("nonce_acquisition", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let lease = nonce_manager.acquire_nonce().await.unwrap();
                black_box(lease.nonce_pubkey());
                drop(lease.release().await);
            })
        });
    });
}

/// Benchmark: Nonce acquisition with varying pool sizes
fn bench_nonce_acquisition_pool_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonce_acquisition_pool_size");

    for pool_size in [5, 10, 20, 50].iter() {
        let nonce_manager = create_test_nonce_manager_blocking(*pool_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(pool_size),
            pool_size,
            |b, _| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let lease = nonce_manager.acquire_nonce().await.unwrap();
                        black_box(lease.nonce_pubkey());
                        drop(lease.release().await);
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: RAII guard lifecycle (acquire + release)
fn bench_raii_guard_lifecycle(c: &mut Criterion) {
    let nonce_manager = create_test_nonce_manager_blocking(10);

    c.bench_function("raii_guard_lifecycle", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let lease = nonce_manager.acquire_nonce().await.unwrap();
                // Simulate minimal work
                black_box(lease.nonce_pubkey());
                black_box(lease.nonce_blockhash());
                // Explicit release (best practice)
                drop(lease.release().await);
            })
        });
    });
}

/// Benchmark: Transaction building with nonce (full workflow)
fn bench_transaction_building_with_nonce(c: &mut Criterion) {
    let nonce_manager = create_test_nonce_manager_blocking(10);
    let payer = Keypair::new();

    c.bench_function("transaction_building_with_nonce", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let lease = nonce_manager.acquire_nonce().await.unwrap();
                let nonce_pubkey = *lease.nonce_pubkey();
                let nonce_blockhash = lease.nonce_blockhash();

                // Build transaction
                let tx = build_transaction_with_nonce(
                    &nonce_pubkey,
                    &payer,
                    nonce_blockhash,
                    &payer,
                );

                black_box(&tx);
                drop(lease.release().await);
            })
        });
    });
}

/// Benchmark: Transaction building WITHOUT nonce (baseline)
fn bench_transaction_building_without_nonce(c: &mut Criterion) {
    let payer = Keypair::new();

    c.bench_function("transaction_building_without_nonce", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let instructions = vec![system_instruction::transfer(
                    &payer.pubkey(),
                    &Pubkey::new_unique(),
                    1_000_000,
                )];

                let message = MessageV0::try_compile(
                    &payer.pubkey(),
                    &instructions,
                    &[],
                    Hash::default(),
                )
                .unwrap();

                let tx =
                    VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer]).unwrap();

                black_box(&tx);
            })
        });
    });
}

/// Benchmark: Instruction ordering overhead
fn bench_instruction_ordering_overhead(c: &mut Criterion) {
    let payer = Keypair::new();
    let nonce_account = Pubkey::new_unique();
    let nonce_blockhash = Hash::default();

    let mut group = c.benchmark_group("instruction_ordering");

    // With nonce (advance_nonce first)
    group.bench_function("with_nonce", |b| {
        b.iter(|| {
            let mut instructions = vec![];
            instructions.push(system_instruction::advance_nonce_account(
                &nonce_account,
                &payer.pubkey(),
            ));
            instructions.push(Instruction::new_with_bytes(
                solana_sdk::compute_budget::id(),
                &[2, 0, 0, 0, 0, 200, 0, 0],
                vec![],
            ));
            instructions.push(system_instruction::transfer(
                &payer.pubkey(),
                &Pubkey::new_unique(),
                1_000_000,
            ));

            let message = MessageV0::try_compile(
                &payer.pubkey(),
                &instructions,
                &[],
                nonce_blockhash,
            )
            .unwrap();

            black_box(&message);
        });
    });

    // Without nonce (baseline)
    group.bench_function("without_nonce", |b| {
        b.iter(|| {
            let mut instructions = vec![];
            instructions.push(Instruction::new_with_bytes(
                solana_sdk::compute_budget::id(),
                &[2, 0, 0, 0, 0, 200, 0, 0],
                vec![],
            ));
            instructions.push(system_instruction::transfer(
                &payer.pubkey(),
                &Pubkey::new_unique(),
                1_000_000,
            ));

            let message = MessageV0::try_compile(
                &payer.pubkey(),
                &instructions,
                &[],
                Hash::default(),
            )
            .unwrap();

            black_box(&message);
        });
    });

    group.finish();
}

/// Benchmark: Concurrent acquisition (measures contention)
fn bench_concurrent_acquisition(c: &mut Criterion) {
    let nonce_manager = create_test_nonce_manager_blocking(10);

    let mut group = c.benchmark_group("concurrent_acquisition");

    for concurrency in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &conc| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = vec![];

                        for _ in 0..conc {
                            let manager = nonce_manager.clone();
                            let handle = tokio::spawn(async move {
                                if let Ok(lease) = manager.acquire_nonce().await {
                                    black_box(lease.nonce_pubkey());
                                    drop(lease.release().await);
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            let _ = handle.await;
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory allocation overhead
fn bench_memory_allocation(c: &mut Criterion) {
    let nonce_manager = create_test_nonce_manager_blocking(10);

    c.bench_function("memory_allocation_per_cycle", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                // Full cycle: acquire, use, release
                let lease = nonce_manager.acquire_nonce().await.unwrap();
                let _pubkey = *lease.nonce_pubkey();
                let _blockhash = lease.nonce_blockhash();
                drop(lease.release().await);
            })
        });
    });
}

criterion_group!(
    benches,
    bench_nonce_acquisition,
    bench_nonce_acquisition_pool_sizes,
    bench_raii_guard_lifecycle,
    bench_transaction_building_with_nonce,
    bench_transaction_building_without_nonce,
    bench_instruction_ordering_overhead,
    bench_concurrent_acquisition,
    bench_memory_allocation,
);
criterion_main!(benches);
