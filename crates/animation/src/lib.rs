#![forbid(unsafe_code)]
//! Duration-based layout effects started by user-selected events.
//!
//! Install [`AnimationModule`] after `LayoutModule` and `WindowModule`, but
//! before `RenderModule`. Playback runs after Taffy on `PostTick`, and is
//! sampled again before rendering each `Frame`. Widgets choose their own
//! trigger events through [`LayoutAnimation::on_event`].
//!
//! Effects read the latest `ComputedLayout` without changing it and produce
//! a `Layout`. Changed output uses the existing `OnChanged<Layout>`/frame
//! request path. This module does not install a runner or an independent
//! wakeup source: effects that hold unchanged output may need an external
//! wakeup to progress through that hold.

use std::rc::Rc;
use std::time::Duration;

use app::prelude::*;
use layout::{ComputedLayout, Layout};
use window::{Frame, Window};

mod time;
pub use time::Time;

pub mod prelude {
    pub use crate::{AnimationModule, LayoutAnimation, Time};
}

type LayoutEffect = dyn Fn(&ComputedLayout, f32) -> Layout;

/// A reusable duration, easing curve, and layout-effect callable.
///
/// Easing receives progress in `0..=1`. Its output is passed to the effect
/// without clamping, allowing overshoot. The effect should return finite,
/// valid geometry based on the supplied computed layout, not accumulate
/// changes from a previous visual layout. Both callables can capture data;
/// cloning this description shares them, but each start has its own timer.
///
/// ```
/// use app::prelude::*;
/// use animation::prelude::*;
/// use layout::Layout;
/// use std::time::Duration;
///
/// struct Click;
/// impl Event for Click {}
///
/// fn attach<W: Widget>(s: &mut Spawner<'_, W>, me: Handle<W>) {
///     let distance = 24.0;
///     let slide = LayoutAnimation::new(Duration::from_millis(200), move |base, p| {
///         let mut layout = Layout::from(*base);
///         layout.rect.origin.x += distance * (1.0 - p);
///         layout
///     })
///     .easing(|t| t * t);
///     s.on::<Click>(me, slide.on_event());
/// }
/// ```
#[derive(Clone)]
pub struct LayoutAnimation {
    duration: Duration,
    easing: Rc<dyn Fn(f32) -> f32>,
    effect: Rc<LayoutEffect>,
}

impl LayoutAnimation {
    /// Construct an effect with linear easing. A zero duration applies its
    /// final sample immediately and leaves no active playback.
    pub fn new(
        duration: Duration,
        effect: impl Fn(&ComputedLayout, f32) -> Layout + 'static,
    ) -> Self {
        Self {
            duration,
            easing: Rc::new(|t| t),
            effect: Rc::new(effect),
        }
    }

    /// Replace the easing curve. A captured custom curve works just like
    /// a built-in function; no registration or enum variant is needed.
    pub fn easing(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
        self.easing = Rc::new(easing);
        self
    }

    /// Create a trigger handler for any event type. Like `Context::fetch`,
    /// it operates on the handler's owner; attach it to `me`, or use
    /// [`Self::start`] with `Context::at` to animate another widget.
    pub fn on_event<W: Widget, E: Event>(self) -> impl FnMut(&mut Context<'_, W>, &E) {
        move |ctx, _| self.start(ctx)
    }

    /// Start at progress zero, replacing any running layout animation on
    /// this widget. Retriggering restarts the effect, not a blend from the
    /// current visual value. The initial sample is visible to later handlers
    /// of the same event. Requires `LayoutModule` and [`AnimationModule`].
    ///
    /// Uses the latest computed base, so trigger after initial layout when
    /// an effect needs resolved dimensions. On completion, the final sample
    /// remains until another writer or a subsequent Taffy pass changes it.
    pub fn start<W: Widget>(&self, ctx: &mut Context<'_, W>) {
        let node = ctx.handle().id();
        let window = std::iter::once(node)
            .chain(ctx.tree().ancestors(node))
            .find(|&id| ctx.widget::<Window>(id).is_some());
        let (computed, mut layout, mut time, mut playing) =
            ctx.fetch::<(&ComputedLayout, &mut Layout, ResMut<Time>, ResMut<Playing>)>();

        // A trigger can arrive after a long idle wait, before the next Tick.
		  // TODO: make it so somehow, this isn't needed here
        time.update();
        let (value, finished) = self.sample(computed, Duration::ZERO);
        layout.set_if_neq(value);
        playing.0.retain(|p| p.node != node);
        if !finished {
            playing.0.push(Playback {
                node,
                window,
                started: time.elapsed(),
                animation: self.clone(),
            });
        }
    }

    fn sample(&self, base: &ComputedLayout, elapsed: Duration) -> (Layout, bool) {
        let finished = elapsed >= self.duration;
        let progress = if finished {
            1.0
        } else {
            (elapsed.as_secs_f64() / self.duration.as_secs_f64()) as f32
        };
        ((self.effect)(base, (self.easing)(progress)), finished)
    }
}

struct Playback {
    node: NodeId,
    window: Option<NodeId>,
    started: Duration, // TODO: see if you should change this to Instant
    animation: LayoutAnimation,
}

#[derive(Default)]
struct Playing(Vec<Playback>);
impl Resource for Playing {}

/// Installs timing and playback systems, never widget handlers. Install
/// once, after layout/window and before render and other time consumers.
/// A headless app can omit window/render and drive playback with ticks.
pub struct AnimationModule;

impl Module for AnimationModule {
    fn install(self, app: &mut App) {
        app.init_resource::<Time>()
            .init_resource::<Playing>()
            .system(update_time)
            .system(on_post_tick)
            .system(on_frame)
            .system(on_removed);
    }
}

fn update_time(app: &mut App, _: &Tick) {
    app.resource_mut::<Time>().update();
}

fn on_post_tick(app: &mut App, _: &PostTick) {
    if app.resource::<Playing>().0.is_empty() {
        return;
    }
    evaluate(app, None);
    // Layout's automatic drain ran earlier in PostTick. Report the writes
    // from playback now, through the same event path, without waiting a tick.
    let changed: Vec<_> = app.take_changed::<Layout>().collect();
    if !changed.is_empty() {
        app.emit(OnChanged::<Layout>::new(), changed);
    }
}

fn on_frame(app: &mut App, frame: &Frame) {
    if !app
        .resource::<Playing>()
        .0
        .iter()
        .any(|p| p.window == Some(frame.0))
    {
        return;
    }
    // Frame callbacks are flushed before the runner's next Tick. Resample
    // here to avoid drawing with the timestamp from before the I/O wait.
    app.resource_mut::<Time>().update();
    evaluate(app, Some(frame.0));
}

fn evaluate(app: &mut App, window: Option<NodeId>) {
    let (tree, mut data) = app.split();
    let (computed, mut layouts, time, mut playing) =
        data.query::<(&ComputedLayout, &mut Layout, Res<Time>, ResMut<Playing>)>();
    playing.0.retain(|p| {
        if !tree.is_live(p.node) {
            return false;
        }
        if window.is_some() && p.window != window {
            return true;
        }
        let elapsed = time.elapsed().saturating_sub(p.started);
        let (value, finished) = p.animation.sample(&computed[p.node], elapsed);
        layouts.get_mut(p.node).unwrap().set_if_neq(value);
        !finished
    });
}

fn on_removed(app: &mut App, _: &Removed) {
    let (tree, mut data) = app.split();
    let mut playing = data.query::<ResMut<Playing>>();
    // Removed names only the subtree root; prune its dead descendants too.
    playing.0.retain(|p| tree.is_live(p.node));
}
