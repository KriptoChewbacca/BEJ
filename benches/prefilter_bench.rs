//! Benchmark for prefilter performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn create_mock_tx(size: usize) -> Vec<u8> {
    let mut tx = vec![0x01; size];
    // Add some variation to prevent optimizations
    for i in 0..size.min(100) {
        tx[i] = (i % 256) as u8;
    }
    tx
}

fn bench_prefilter_should_process(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefilter");

    for size in [128, 256, 512, 1024].iter() {
        let tx = create_mock_tx(*size);

        group.bench_with_input(BenchmarkId::new("should_process", size), &tx, |b, tx| {
            b.iter(|| {
                // Note: This will likely always return false for mock data
                // In real benchmarks, use realistic transaction data
                black_box(ultra::sniffer::prefilter::should_process(black_box(tx)))
            });
        });
    }

    group.finish();
}

fn bench_prefilter_vote_check(c: &mut Criterion) {
    let tx = create_mock_tx(256);

    c.bench_function("is_vote_tx", |b| {
        b.iter(|| black_box(ultra::sniffer::prefilter::is_vote_tx(black_box(&tx))));
    });
}

criterion_group!(
    benches,
    bench_prefilter_should_process,
    bench_prefilter_vote_check
);
criterion_main!(benches);
