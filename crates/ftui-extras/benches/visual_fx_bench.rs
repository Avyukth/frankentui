//! Benchmarks for visual FX (bd-l8x9.9.4)
//!
//! Performance budgets:
//! - Single-layer backdrop render (80x24): < 1ms
//! - Two-layer stacked composition (80x24): < 2ms (layering overhead should be minimal)
//! - MetaballsFx per-cell compute: < 1μs
//! - PlasmaFx per-cell compute: < 500ns
//!
//! Run with: cargo bench -p ftui-extras --bench visual_fx_bench --features visual-fx

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

#[cfg(feature = "visual-fx")]
use ftui_core::geometry::Rect;
#[cfg(feature = "visual-fx")]
use ftui_extras::visual_fx::{
    Backdrop, BackdropFx, BlendMode, FxContext, FxLayer, FxQuality, MetaballsFx, MetaballsParams,
    PlasmaFx, StackedFx, ThemeInputs,
};
#[cfg(feature = "visual-fx")]
use ftui_render::cell::PackedRgba;
#[cfg(feature = "visual-fx")]
use ftui_render::frame::Frame;
#[cfg(feature = "visual-fx")]
use ftui_render::grapheme_pool::GraphemePool;
#[cfg(feature = "visual-fx")]
use ftui_widgets::Widget;

// =============================================================================
// Area Size Configurations
// =============================================================================

/// Common terminal sizes for benchmarking
#[cfg(feature = "visual-fx")]
const SIZES: &[(u16, u16, &str)] = &[
    (80, 24, "80x24"),   // Standard terminal
    (120, 40, "120x40"), // Large terminal
    (200, 60, "200x60"), // Extra-large
];

// =============================================================================
// Effect Compute Benchmarks (raw FX without Widget overhead)
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_metaballs_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/metaballs_compute");
    let theme = ThemeInputs::default_dark();

    for &(width, height, name) in SIZES {
        let len = width as usize * height as usize;
        group.throughput(Throughput::Elements(len as u64));

        group.bench_with_input(
            BenchmarkId::new("full_quality", name),
            &(width, height),
            |b, &(w, h)| {
                let mut fx = MetaballsFx::default();
                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    fx.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reduced_quality", name),
            &(width, height),
            |b, &(w, h)| {
                let mut fx = MetaballsFx::default();
                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Reduced,
                    theme: &theme,
                };

                b.iter(|| {
                    fx.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("minimal_quality", name),
            &(width, height),
            |b, &(w, h)| {
                let mut fx = MetaballsFx::default();
                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Minimal,
                    theme: &theme,
                };

                b.iter(|| {
                    fx.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "visual-fx")]
fn bench_plasma_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/plasma_compute");
    let theme = ThemeInputs::default_dark();

    for &(width, height, name) in SIZES {
        let len = width as usize * height as usize;
        group.throughput(Throughput::Elements(len as u64));

        group.bench_with_input(
            BenchmarkId::new("full_quality", name),
            &(width, height),
            |b, &(w, h)| {
                let mut fx = PlasmaFx::default();
                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    fx.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("minimal_quality", name),
            &(width, height),
            |b, &(w, h)| {
                let mut fx = PlasmaFx::default();
                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Minimal,
                    theme: &theme,
                };

                b.iter(|| {
                    fx.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// Single-Layer Backdrop Widget Benchmarks
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_backdrop_single_layer(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/backdrop_single");
    let theme = ThemeInputs::default_dark();

    for &(width, height, name) in SIZES {
        let len = width as usize * height as usize;
        group.throughput(Throughput::Elements(len as u64));

        // Metaballs single layer
        group.bench_with_input(
            BenchmarkId::new("metaballs", name),
            &(width, height),
            |b, &(w, h)| {
                let mut backdrop = Backdrop::new(Box::new(MetaballsFx::default()), theme);
                backdrop.set_effect_opacity(0.35);
                let mut pool = GraphemePool::new();
                let mut frame = Frame::new(w, h, &mut pool);
                let area = Rect::new(0, 0, w, h);

                b.iter(|| {
                    backdrop.render(black_box(area), &mut frame);
                    black_box(&frame.buffer);
                });
            },
        );

        // Plasma single layer
        group.bench_with_input(
            BenchmarkId::new("plasma", name),
            &(width, height),
            |b, &(w, h)| {
                let mut backdrop = Backdrop::new(Box::new(PlasmaFx::default()), theme);
                backdrop.set_effect_opacity(0.35);
                let mut pool = GraphemePool::new();
                let mut frame = Frame::new(w, h, &mut pool);
                let area = Rect::new(0, 0, w, h);

                b.iter(|| {
                    backdrop.render(black_box(area), &mut frame);
                    black_box(&frame.buffer);
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// Multi-Layer Stacked Backdrop Benchmarks (bd-l8x9.9.4)
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_stacked_fx_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/stacked_layers");
    let theme = ThemeInputs::default_dark();

    for &(width, height, name) in SIZES {
        let len = width as usize * height as usize;
        group.throughput(Throughput::Elements(len as u64));

        // 1 layer (baseline for comparison)
        group.bench_with_input(
            BenchmarkId::new("1_layer_plasma", name),
            &(width, height),
            |b, &(w, h)| {
                let mut stack = StackedFx::new();
                stack.push(FxLayer::new(Box::new(PlasmaFx::default())));

                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    stack.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        // 2 layers (Over blend)
        group.bench_with_input(
            BenchmarkId::new("2_layers_over", name),
            &(width, height),
            |b, &(w, h)| {
                let mut stack = StackedFx::new();
                stack.push(FxLayer::new(Box::new(PlasmaFx::ocean())));
                stack.push(FxLayer::with_opacity(Box::new(MetaballsFx::default()), 0.5));

                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    stack.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        // 2 layers (Additive blend)
        group.bench_with_input(
            BenchmarkId::new("2_layers_additive", name),
            &(width, height),
            |b, &(w, h)| {
                let mut stack = StackedFx::new();
                stack.push(FxLayer::new(Box::new(PlasmaFx::fire())));
                stack.push(FxLayer::with_opacity_and_blend(
                    Box::new(PlasmaFx::cyberpunk()),
                    0.3,
                    BlendMode::Additive,
                ));

                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    stack.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );

        // 3 layers
        group.bench_with_input(
            BenchmarkId::new("3_layers_mixed", name),
            &(width, height),
            |b, &(w, h)| {
                let mut stack = StackedFx::new();
                stack.push(FxLayer::new(Box::new(PlasmaFx::ocean())));
                stack.push(FxLayer::with_opacity_and_blend(
                    Box::new(MetaballsFx::new(MetaballsParams::aurora())),
                    0.4,
                    BlendMode::Screen,
                ));
                stack.push(FxLayer::with_opacity(Box::new(PlasmaFx::fire()), 0.2));

                let mut out = vec![PackedRgba::TRANSPARENT; len];
                let ctx = FxContext {
                    width: w,
                    height: h,
                    frame: 0,
                    time_seconds: 0.5,
                    quality: FxQuality::Full,
                    theme: &theme,
                };

                b.iter(|| {
                    stack.render(ctx, black_box(&mut out));
                    black_box(&out);
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// Layering Overhead Analysis
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_layering_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/layering_overhead");
    let theme = ThemeInputs::default_dark();

    // Focus on 80x24 to measure pure overhead
    let (width, height) = (80, 24);
    let len = width as usize * height as usize;
    group.throughput(Throughput::Elements(len as u64));

    // Baseline: Raw effect compute (no Backdrop widget overhead)
    group.bench_function("raw_plasma", |b| {
        let mut fx = PlasmaFx::default();
        let mut out = vec![PackedRgba::TRANSPARENT; len];
        let ctx = FxContext {
            width,
            height,
            frame: 0,
            time_seconds: 0.5,
            quality: FxQuality::Full,
            theme: &theme,
        };

        b.iter(|| {
            fx.render(ctx, black_box(&mut out));
            black_box(&out);
        });
    });

    // Single layer in StackedFx (measures compositor overhead)
    group.bench_function("stacked_1_layer", |b| {
        let mut stack = StackedFx::new();
        stack.push(FxLayer::new(Box::new(PlasmaFx::default())));

        let mut out = vec![PackedRgba::TRANSPARENT; len];
        let ctx = FxContext {
            width,
            height,
            frame: 0,
            time_seconds: 0.5,
            quality: FxQuality::Full,
            theme: &theme,
        };

        b.iter(|| {
            stack.render(ctx, black_box(&mut out));
            black_box(&out);
        });
    });

    // Two identical layers (measures per-layer overhead)
    group.bench_function("stacked_2_identical", |b| {
        let mut stack = StackedFx::new();
        stack.push(FxLayer::new(Box::new(PlasmaFx::default())));
        stack.push(FxLayer::with_opacity(Box::new(PlasmaFx::default()), 0.5));

        let mut out = vec![PackedRgba::TRANSPARENT; len];
        let ctx = FxContext {
            width,
            height,
            frame: 0,
            time_seconds: 0.5,
            quality: FxQuality::Full,
            theme: &theme,
        };

        b.iter(|| {
            stack.render(ctx, black_box(&mut out));
            black_box(&out);
        });
    });

    // Three identical layers
    group.bench_function("stacked_3_identical", |b| {
        let mut stack = StackedFx::new();
        stack.push(FxLayer::new(Box::new(PlasmaFx::default())));
        stack.push(FxLayer::with_opacity(Box::new(PlasmaFx::default()), 0.5));
        stack.push(FxLayer::with_opacity(Box::new(PlasmaFx::default()), 0.3));

        let mut out = vec![PackedRgba::TRANSPARENT; len];
        let ctx = FxContext {
            width,
            height,
            frame: 0,
            time_seconds: 0.5,
            quality: FxQuality::Full,
            theme: &theme,
        };

        b.iter(|| {
            stack.render(ctx, black_box(&mut out));
            black_box(&out);
        });
    });

    group.finish();
}

// =============================================================================
// Blend Mode Benchmarks
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_blend_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/blend_modes");
    let theme = ThemeInputs::default_dark();

    let (width, height) = (80, 24);
    let len = width as usize * height as usize;
    group.throughput(Throughput::Elements(len as u64));

    for blend_mode in [
        BlendMode::Over,
        BlendMode::Additive,
        BlendMode::Multiply,
        BlendMode::Screen,
    ] {
        let mode_name = format!("{:?}", blend_mode).to_lowercase();

        group.bench_function(&mode_name, |b| {
            let mut stack = StackedFx::new();
            stack.push(FxLayer::new(Box::new(PlasmaFx::ocean())));
            stack.push(FxLayer::with_opacity_and_blend(
                Box::new(PlasmaFx::fire()),
                0.5,
                blend_mode,
            ));

            let mut out = vec![PackedRgba::TRANSPARENT; len];
            let ctx = FxContext {
                width,
                height,
                frame: 0,
                time_seconds: 0.5,
                quality: FxQuality::Full,
                theme: &theme,
            };

            b.iter(|| {
                stack.render(ctx, black_box(&mut out));
                black_box(&out);
            });
        });
    }

    group.finish();
}

// =============================================================================
// Buffer Caching Verification (bd-l8x9.2.1)
// =============================================================================

#[cfg(feature = "visual-fx")]
fn bench_buffer_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_fx/buffer_reuse");
    let theme = ThemeInputs::default_dark();

    let (width, height) = (80, 24);
    group.throughput(Throughput::Elements((width * height) as u64));

    // First render (cold, allocates buffers) - just measure raw create+render
    group.bench_function("first_render", |b| {
        b.iter(|| {
            let backdrop = Backdrop::new(Box::new(MetaballsFx::default()), theme);
            let mut pool = GraphemePool::new();
            let mut frame = Frame::new(width, height, &mut pool);
            let area = Rect::new(0, 0, width, height);
            backdrop.render(black_box(area), &mut frame);
            black_box(&frame.buffer);
        });
    });

    // Subsequent renders (warm, reuses buffers)
    group.bench_function("steady_state", |b| {
        let backdrop = Backdrop::new(Box::new(MetaballsFx::default()), theme);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(width, height, &mut pool);
        let area = Rect::new(0, 0, width, height);

        // Warm up - allocate buffers
        backdrop.render(area, &mut frame);

        b.iter(|| {
            backdrop.render(black_box(area), &mut frame);
            black_box(&frame.buffer);
        });
    });

    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

#[cfg(feature = "visual-fx")]
criterion_group!(
    benches,
    bench_metaballs_compute,
    bench_plasma_compute,
    bench_backdrop_single_layer,
    bench_stacked_fx_layers,
    bench_layering_overhead,
    bench_blend_modes,
    bench_buffer_reuse,
);

#[cfg(not(feature = "visual-fx"))]
fn bench_placeholder(_c: &mut Criterion) {
    // Placeholder when visual-fx feature is not enabled
}

#[cfg(not(feature = "visual-fx"))]
criterion_group!(benches, bench_placeholder);

criterion_main!(benches);
