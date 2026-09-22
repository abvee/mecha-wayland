use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use animation::prelude::*;
use app::prelude::*;
use geometry::{Color, Insets};
use layout::prelude::*;
use paint::prelude::*;
use render::prelude::*;
use window::prelude::*;

struct Click;
impl Event for Click {}
struct HoverEnter;
impl Event for HoverEnter {}

struct TestWidget {
    seen: Option<Layout>,
}

struct TestBuilder {
    click: Option<LayoutAnimation>,
    hover: Option<LayoutAnimation>,
}

impl Build for TestBuilder {
    type Widget = TestWidget;
}

impl Widget for TestWidget {
    type Builder = TestBuilder;

    fn build(b: TestBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        if let Some(animation) = b.click {
            s.on::<Click>(me, animation.on_event())
                .on::<Click>(me, |ctx, _| {
                    let layout = *ctx.component::<Layout>().unwrap();
                    ctx.me().seen = Some(layout);
                });
        }
        if let Some(animation) = b.hover {
            s.on::<HoverEnter>(me, move |ctx, _| animation.start(ctx));
        }
        Self { seen: None }
    }
}

#[derive(Default)]
struct Requests(Vec<NodeId>);
impl Resource for Requests {}

fn record(app: &mut App, r: &FrameRequested) {
    app.resource_mut::<Requests>().0.push(r.0);
}

fn answer(app: &mut App, r: &FrameRequested) {
    app.signal(Frame(r.0));
}

fn slide(distance: f32, milliseconds: u64) -> LayoutAnimation {
    LayoutAnimation::new(Duration::from_millis(milliseconds), move |base, p| {
        let mut layout = Layout::from(*base);
        layout.rect.origin.x += distance * (1.0 - p);
        layout
    })
}

struct Fixture {
    app: App,
    start: Instant,
    window: Handle<Window>,
    node: Handle<TestWidget>,
}

impl Fixture {
    fn new(animation: LayoutAnimation) -> Self {
        let start = Instant::now();
        let mut time = Time::manual();
        time.update_at(start);
        let mut app = App::new();
        app.insert_resource(time);
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(WindowModule)
            .add_module(AnimationModule)
            .add_module(RenderModule::default())
            .init_resource::<Requests>()
            .system(record);
        let window = app.spawn(
            app.root(),
            window().layout(LayoutStyle::default().size(px(200.0), px(100.0))),
        );
        let node = app.spawn_with(
            window,
            TestBuilder {
                click: Some(animation),
                hover: Some(slide(40.0, 400)),
            },
            (
                LayoutStyle::default()
                    .size(px(60.0), px(40.0))
                    .padding(Insets::new(px(1.0), px(2.0), px(3.0), px(4.0)))
                    .border(Insets::all(px(2.0))),
                Paint::Quad(Quad::new(Color::WHITE)),
            ),
        );
        app.tick();
        let mut fixture = Self {
            app,
            start,
            window,
            node,
        };
        fixture.frame();
        fixture.take_requests();
        fixture
    }

    fn at(&mut self, ms: u64) {
        self.app
            .resource_mut::<Time>()
            .update_at(self.start + Duration::from_millis(ms));
    }

    fn trigger<E: Event>(&mut self, event: E) {
        self.app.emit(event, self.node);
        self.app.flush();
    }

    fn output(&self) -> Layout {
        *self.app.component::<Layout>(self.node).unwrap()
    }

    fn frame(&mut self) -> Queue {
        self.app.signal(Frame(self.window.id()));
        self.app.flush();
        self.app
            .resource_mut::<Scenes>()
            .queue(self.window.id(), 1)
            .unwrap()
            .clone()
    }

    fn take_requests(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.app.resource_mut::<Requests>().0)
    }
}

#[test]
fn click_slide_reaches_initial_halfway_and_final_values_then_stops() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let animation = LayoutAnimation::new(Duration::from_millis(200), move |base, p| {
        count.set(count.get() + 1);
        let mut layout = Layout::from(*base);
        layout.rect.origin.x += 24.0 * (1.0 - p);
        layout
    });
    let mut f = Fixture::new(animation);
    let base = *f.app.component::<ComputedLayout>(f.node).unwrap();
    assert_eq!(calls.get(), 0, "installation does not start effects");
    assert_eq!(f.output(), Layout::from(base));
    f.app.system(answer);

    f.trigger(Click);
    assert_eq!(f.output().rect.x(), base.rect.x() + 24.0);
    assert_eq!(
        f.app.widget::<TestWidget>(f.node).unwrap().seen,
        Some(f.output())
    );
    f.app.tick();
    assert_eq!(f.take_requests(), [f.window.id()]);

    for (ms, offset) in [(100, 12.0), (200, 0.0)] {
        f.at(ms);
        f.app.tick();
        assert_eq!(f.output().rect.x(), base.rect.x() + offset);
        assert_eq!(f.output().padding, base.padding);
        assert_eq!(f.output().border, base.border);
        assert_eq!(f.app.component::<ComputedLayout>(f.node), Some(&base));
        assert_eq!(f.take_requests(), [f.window.id()]);
        let expected = f.output().rect;
        let mut scenes = f.app.resource_mut::<Scenes>();
        let queue = scenes.queue(f.window.id(), 1).unwrap();
        assert_eq!(queue.opaque.commands[0].rect, expected);
    }

    let completed_calls = calls.get();
    f.at(300);
    f.app.tick();
    f.frame();
    assert_eq!(
        calls.get(),
        completed_calls,
        "completed effects are removed"
    );
    assert!(f.take_requests().is_empty());
}

#[test]
fn retriggering_restarts_and_a_different_event_replaces_the_running_effect() {
    let mut f = Fixture::new(slide(24.0, 200));
    let x = f.output().rect.x();
    f.trigger(Click);
    f.at(100);
    f.app.tick();
    assert_eq!(f.output().rect.x(), x + 12.0);

    f.trigger(Click);
    assert_eq!(f.output().rect.x(), x + 24.0);
    f.at(200);
    f.app.tick();
    assert_eq!(f.output().rect.x(), x + 12.0);

    f.trigger(HoverEnter);
    assert_eq!(f.output().rect.x(), x + 40.0);
    f.at(400);
    f.app.tick();
    assert_eq!(f.output().rect.x(), x + 20.0);
    f.at(600);
    f.app.tick();
    assert_eq!(f.output().rect.x(), x);
}

#[test]
fn custom_captured_easing_is_used_and_overshoot_is_not_clamped() {
    let exponent = 2;
    let mut f = Fixture::new(slide(24.0, 200).easing(move |t| t.powi(exponent)));
    f.trigger(Click);
    f.at(100);
    f.app.tick();
    assert_eq!(f.output().rect.x(), 18.0);

    let mut f = Fixture::new(slide(24.0, 200).easing(|t| t * 3.0));
    f.trigger(Click);
    f.at(100);
    f.app.tick();
    assert_eq!(f.output().rect.x(), -12.0);
}

#[test]
fn playback_uses_fresh_taffy_output_in_the_same_tick() {
    let mut f = Fixture::new(slide(24.0, 200));
    f.trigger(Click);
    f.at(100);
    f.app.component_mut::<LayoutStyle>(f.node).unwrap().width = px(80.0);
    f.app.tick();
    let base = *f.app.component::<ComputedLayout>(f.node).unwrap();
    assert_eq!(base.rect.width(), 80.0);
    assert_eq!(f.output().rect.width(), 80.0);
    assert_eq!(f.output().rect.x(), base.rect.x() + 12.0);
}

#[test]
fn a_frame_samples_playback_before_render_without_waiting_for_another_tick() {
    let mut f = Fixture::new(slide(24.0, 200));
    f.trigger(Click);
    f.app.tick();
    f.frame();
    f.at(100);
    let queue = f.frame();
    assert_eq!(queue.opaque.commands[0].rect.x(), 12.0);
    assert_eq!(f.output().rect.x(), 12.0);
    assert!(!queue.scissor.is_empty());
}

#[test]
fn pending_frames_coalesce_and_the_eventual_frame_uses_current_progress() {
    let mut f = Fixture::new(slide(24.0, 200));
    f.trigger(Click);
    f.app.tick();
    for ms in [50, 100] {
        f.at(ms);
        f.app.tick();
    }
    assert_eq!(f.take_requests(), [f.window.id()]);
    assert_eq!(f.output().rect.x(), 12.0);

    f.at(150);
    let queue = f.frame();
    assert_eq!(f.output().rect.x(), 6.0);
    assert_eq!(queue.opaque.commands[0].rect.x(), 6.0);
}

#[test]
fn an_unchanged_sample_does_not_request_an_extra_frame() {
    let mut f = Fixture::new(slide(24.0, 200));
    f.trigger(Click);
    f.app.tick();
    f.frame();
    f.take_requests();
    f.app.tick();
    assert!(f.take_requests().is_empty());
    assert_eq!(f.output().rect.x(), 24.0);
}

#[test]
fn zero_duration_and_large_time_jumps_finish_without_nan_or_repeated_playback() {
    let mut f = Fixture::new(slide(24.0, 0));
    let base = f.output();
    f.trigger(Click);
    assert_eq!(f.output(), base);
    f.app.tick();
    assert!(f.take_requests().is_empty());

    let mut f = Fixture::new(slide(24.0, 200));
    let base = f.output();
    f.trigger(Click);
    f.at(10_000);
    f.app.tick();
    assert_eq!(f.output(), base);
}

#[test]
fn removed_subtrees_are_cleaned_up_and_reused_slots_do_not_inherit_playback() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let mut f = Fixture::new(LayoutAnimation::new(
        Duration::from_secs(1),
        move |base, p| {
            count.set(count.get() + 1);
            let mut layout = Layout::from(*base);
            layout.rect.origin.x += 24.0 * (1.0 - p);
            layout
        },
    ));
    f.trigger(Click);
    let before = calls.get();
    let old = f.node.id();
    f.app.remove(f.window);
    f.app.flush();
    let parent = f.app.spawn(
        f.app.root(),
        TestBuilder {
            click: None,
            hover: None,
        },
    );
    let replacement = f.app.spawn(
        parent,
        TestBuilder {
            click: None,
            hover: None,
        },
    );
    assert_eq!(replacement.id().slot(), old.slot());
    assert_ne!(replacement.id(), old);
    f.at(100);
    f.app.tick();
    assert_eq!(calls.get(), before);
    assert_eq!(
        f.app.component::<Layout>(replacement),
        Some(&Layout::default())
    );
}

#[test]
fn one_description_can_run_on_two_nodes_with_independent_start_times() {
    let animation = slide(24.0, 200);
    let mut f = Fixture::new(animation.clone());
    let second = f.app.spawn_with(
        f.window,
        TestBuilder {
            click: Some(animation),
            hover: None,
        },
        (LayoutStyle::default().size(px(20.0), px(20.0)),),
    );
    f.app.tick();
    let second_base = f.app.component::<ComputedLayout>(second).unwrap().rect.x();
    f.trigger(Click);
    f.at(100);
    f.app.emit(Click, second);
    f.app.tick();
    assert_eq!(f.output().rect.x(), 12.0);
    assert_eq!(
        f.app.component::<Layout>(second).unwrap().rect.x(),
        second_base + 24.0
    );
    f.at(200);
    f.app.tick();
    assert_eq!(f.output().rect.x(), 0.0);
    assert_eq!(
        f.app.component::<Layout>(second).unwrap().rect.x(),
        second_base + 12.0
    );
}
