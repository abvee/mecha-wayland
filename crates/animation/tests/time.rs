use std::time::{Duration, Instant};

use animation::prelude::*;
use app::prelude::*;

#[test]
fn the_clock_starts_at_zero_and_the_first_sample_sets_its_origin() {
    let mut time = Time::default();
    assert_eq!(time.elapsed(), Duration::ZERO);
    assert_eq!(time.delta(), Duration::ZERO);

    time.update_at(Instant::now() + Duration::from_secs(10));
    assert_eq!(time.elapsed(), Duration::ZERO);
    assert_eq!(time.delta(), Duration::ZERO);
}

#[test]
fn elapsed_accumulates_while_delta_is_only_the_latest_interval() {
    let mut time = Time::default();
    let start = Instant::now();
    time.update_at(start);
    time.update_at(start + Duration::from_millis(16));
    assert_eq!(time.elapsed(), Duration::from_millis(16));
    assert_eq!(time.delta(), Duration::from_millis(16));

    time.update_at(start + Duration::from_millis(40));
    for _ in 0..2 {
        assert_eq!(time.elapsed(), Duration::from_millis(40));
        assert_eq!(time.delta(), Duration::from_millis(24));
    }

    time.update_at(start + Duration::from_millis(40));
    assert_eq!(time.elapsed(), Duration::from_millis(40));
    assert_eq!(time.delta(), Duration::ZERO);
}

#[test]
fn samples_preserve_nanoseconds_and_do_not_clamp_long_gaps() {
    let mut time = Time::default();
    let start = Instant::now();
    let gap = Duration::new(3600, 123);
    time.update_at(start);
    time.update_at(start + gap);
    assert_eq!(time.elapsed(), gap);
    assert_eq!(time.delta(), gap);

    time.update_at(start + gap + Duration::from_nanos(1));
    assert_eq!(time.elapsed(), gap + Duration::from_nanos(1));
    assert_eq!(time.delta(), Duration::from_nanos(1));
}

#[test]
fn a_backwards_sample_panics_without_changing_the_clock() {
    let mut time = Time::default();
    let start = Instant::now();
    time.update_at(start);
    time.update_at(start + Duration::from_millis(20));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        time.update_at(start + Duration::from_millis(10));
    }));
    assert!(result.is_err());
    assert_eq!(time.elapsed(), Duration::from_millis(20));
    assert_eq!(time.delta(), Duration::from_millis(20));

    time.update_at(start + Duration::from_millis(30));
    assert_eq!(time.elapsed(), Duration::from_millis(30));
    assert_eq!(time.delta(), Duration::from_millis(10));
}

#[test]
fn update_samples_the_system_clock() {
    let mut time = Time::default();
    let start = Instant::now() - Duration::from_secs(1);
    time.update_at(start);
    let before = Instant::now();
    time.update();
    let after = Instant::now();
    assert!(time.delta() >= before.duration_since(start));
    assert!(time.delta() <= after.duration_since(start));
    assert_eq!(time.elapsed(), time.delta());
}

#[derive(Default)]
struct Samples(Vec<(Duration, Duration)>);
impl Resource for Samples {}

fn record<S: Signal>(app: &mut App, _: &S) {
    let time = app.resource::<Time>();
    let sample = (time.elapsed(), time.delta());
    app.resource_mut::<Samples>().0.push(sample);
}

#[test]
fn the_module_updates_before_consumers_and_all_readers_share_the_sample() {
    let mut app = App::new();
    app.add_module(AnimationModule)
        .init_resource::<Samples>()
        .system(record::<Tick>)
        .system(record::<PostTick>)
        .system(record::<OnChanged<Time>>);

    app.tick();
    app.tick();
    let samples = &app.resource::<Samples>().0;
    assert_eq!(samples.len(), 6);
    assert_eq!(&samples[..3], &[(Duration::ZERO, Duration::ZERO); 3]);
    assert_eq!(samples[3], samples[4]);
    assert_eq!(samples[4], samples[5]);
    assert_eq!(samples[3].0, samples[3].1);
    assert_eq!(app.resource::<Time>().elapsed(), samples[3].0);
}

#[test]
fn installing_the_module_preserves_an_existing_time_resource() {
    let mut time = Time::default();
    let start = Instant::now() - Duration::from_secs(1);
    time.update_at(start);
    time.update_at(start + Duration::from_millis(7));

    let mut app = App::new();
    app.insert_resource(time);
    app.add_module(AnimationModule);
    assert_eq!(app.resource::<Time>().elapsed(), Duration::from_millis(7));
    assert_eq!(app.resource::<Time>().delta(), Duration::from_millis(7));

    let before = Instant::now();
    app.tick();
    let after = Instant::now();
    let time = app.resource::<Time>();
    assert!(time.elapsed() >= before.duration_since(start));
    assert!(time.elapsed() <= after.duration_since(start));
    assert_eq!(time.delta(), time.elapsed() - Duration::from_millis(7));
}
