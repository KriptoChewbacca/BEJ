//! Benchmark for extractor performance

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use smallvec::SmallVec;
use solana_sdk::pubkey::Pubkey;
use ultra::sniffer::extractor::{PremintCandidate, PriorityLevel};

fn bench_candidate_creation(c: &mut Criterion) {
    let mint = Pubkey::new_unique();
    let mut accounts = SmallVec::new();
    for _ in 0..4 {
        accounts.push(Pubkey::new_unique());
    }

    c.bench_function("candidate_creation", |b| {
        b.iter(|| {
            black_box(PremintCandidate::new(
                black_box(mint),
                black_box(accounts.clone()),
                black_box(1.5),
                black_box(123),
                black_box(PriorityLevel::High),
            ))
        });
    });
}

fn bench_priority_check(c: &mut Criterion) {
    let mint = Pubkey::new_unique();
    let accounts = SmallVec::new();
    let candidate = PremintCandidate::new(mint, accounts, 1.5, 123, PriorityLevel::High);

    c.bench_function("is_high_priority", |b| {
        b.iter(|| black_box(candidate.is_high_priority()));
    });
}

criterion_group!(benches, bench_candidate_creation, bench_priority_check);
criterion_main!(benches);
