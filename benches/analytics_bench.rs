//! Benchmark for analytics performance

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ultra::sniffer::analytics::PredictiveAnalytics;

fn bench_accumulate_volume(c: &mut Criterion) {
    let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);

    c.bench_function("accumulate_volume", |b| {
        b.iter(|| {
            analytics.accumulate_volume(black_box(100.0));
        });
    });
}

fn bench_update_ema(c: &mut Criterion) {
    let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);

    // Pre-populate with some data
    for _ in 0..100 {
        analytics.accumulate_volume(100.0);
    }

    c.bench_function("update_ema", |b| {
        b.iter(|| {
            analytics.update_ema();
        });
    });
}

fn bench_is_high_priority(c: &mut Criterion) {
    let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);

    // Initialize with baseline
    analytics.accumulate_volume(100.0);
    analytics.update_ema();

    c.bench_function("is_high_priority", |b| {
        b.iter(|| black_box(analytics.is_high_priority(black_box(150.0))));
    });
}

criterion_group!(
    benches,
    bench_accumulate_volume,
    bench_update_ema,
    bench_is_high_priority
);
criterion_main!(benches);
