#![forbid(unsafe_code)]
//! Animation foundations. [`AnimationModule`] registers [`SelectedAnimation`]
//! and installs a [`Time`] resource, sampled on each [`Tick`]. Install it
//! before tick systems that read time so they see the current sample.
//!
//! The module does not wake the runner, request frames, or attach widget
//! handlers. Users attach [`animate_layout`] to their own trigger events.
//! Only [`Animation::None`] is implemented so far; it restores the computed
//! geometry. All time readers see the same sample until the next update.
//! A tick is not necessarily a displayed frame.
//!
//! ```
//! use app::prelude::*;
//! use animation::prelude::*;
//! use std::time::Duration;
//!
//! let mut app = App::new();
//! app.add_module(AnimationModule);
//! app.tick();
//! assert_eq!(app.resource::<Time>().elapsed(), Duration::ZERO);
//! assert_eq!(app.resource::<Time>().delta(), Duration::ZERO);
//! ```

use app::prelude::*;
use layout::{ComputedLayout, Layout};

mod time;
pub mod prelude {
    pub use crate::{Animation, AnimationModule, SelectedAnimation, animate_layout};
	 pub use crate::time::*;
}

use time::*;

/// The effect chosen by the widget author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Animation {
    /// Use the default [`ComputedLayout`] to [`Layout`] conversion.
    #[default]
    None,
}

/// Per-node animation selection. Defaults to [`Animation::None`].
/// Setting this component does not attach a handler or trigger playback;
/// the user chooses which events invoke [`animate_layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectedAnimation(pub Animation);

impl Component for SelectedAnimation {}

/// Apply the handler owner's selected layout animation on a user event.
/// With [`Animation::None`], restore the latest computed geometry, writing
/// only if the output differs. The computed base is never modified.
///
/// Requires `LayoutModule` and [`AnimationModule`]. Like [`Context::fetch`],
/// this operates on the handler's owner, not a different event target.
/// Attach it to `me` to animate the widget receiving the event, or use
/// [`Context::at`] to explicitly operate on another widget.
///
/// ```
/// use app::prelude::*;
/// use animation::prelude::*;
///
/// struct Click;
/// impl Event for Click {}
///
/// fn attach<W: Widget>(s: &mut Spawner<'_, W>, me: Handle<W>) {
///     s.on::<Click>(me, animate_layout);
/// }
/// ```
///
/// Uses the most recent `ComputedLayout`; it does not run Taffy. A change
/// is reported by the usual `OnChanged<Layout>` drain at `PostTick`.
pub fn animate_layout<W: Widget, E: Event>(ctx: &mut Context<'_, W>, _: &E) {
    let (selected, computed, mut layout) =
        ctx.fetch::<(&SelectedAnimation, &ComputedLayout, &mut Layout)>();
    match selected.0 {
        Animation::None => {
            layout.set_if_neq(Layout::from(*computed));
        }
    }
}

/// Registers [`SelectedAnimation`], initializes [`Time`] if absent and
/// updates it on [`Tick`]. Install once, before tick systems that consume
/// time. Does not attach widget handlers or install a runner.
pub struct AnimationModule;

impl Module for AnimationModule {
    fn install(self, app: &mut App) {
        app.register_component::<SelectedAnimation>()
            .init_resource::<Time>()
            .system(update_time);
    }
}

fn update_time(app: &mut App, _: &Tick) {
    app.resource_mut::<Time>().update();
}
