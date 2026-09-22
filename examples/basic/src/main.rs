use std::time::Duration;

use mecha_wayland::prelude::*;

struct Leaf;
impl Build for Leaf {
	type Widget = Leaf;
}
impl Widget for Leaf {
	type Builder = Leaf;
	fn build(b: Leaf, _me: Handle<Self>, _s: &mut Spawner<'_, Self>) -> Self {
		b
	}
}

struct SlidingBox;
impl Build for SlidingBox {
	type Widget = SlidingBox;
}
impl Widget for SlidingBox {
	type Builder = SlidingBox;
	fn build(b: SlidingBox, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
		let slide = LayoutAnimation::new(Duration::from_millis(1500), |base, progress| {
			let mut layout = Layout::from(*base);
			layout.rect.origin.x += 260.0 * (1.0 - progress);
			layout
		})
		.easing(|t| t * t * t);
		// .easing(|t| t * t);
		// .easing(|t| t);
		// .easing(|t| 1.0 - (1.0 - t).powi(3));
		let mut started = false;
		// Start with a resolved base, once only, rather than on every relayout.
		s.on::<OnChanged<ComputedLayout>>(me, move |ctx, _| {
			if !started {
				started = true;
				slide.start(ctx);
			}
		});
		b
	}
}

/// Spawns the window and its content; stops the app on close.
struct Shell;
struct ShellBuilder {
	root: NodeId,
}
impl Build for ShellBuilder {
	type Widget = Shell;
}
impl Widget for Shell {
	type Builder = ShellBuilder;
	fn build(b: ShellBuilder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
		let win = s.spawn(
			b.root,
			window()
				.title("showcase")
				.clear(Color::rgb(0.12, 0.12, 0.14))
				.layout(LayoutStyle::default().column().size(px(480.0), px(320.0))),
		);
		s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));

		// An opaque panel.
		let panel = s.spawn_with(
			win,
			Leaf,
			(
				LayoutStyle::default().column().size(px(440.0), px(200.0)),
				Paint::Quad(Quad::new(Color::rgb(0.2, 0.45, 0.8)).radius(12.0)),
			),
		);
		// A translucent card inside it, blended.
		s.spawn_with(
			panel,
			Leaf,
			(
				LayoutStyle::default().size(px(200.0), px(120.0)),
				Paint::Quad(Quad::new(Color::rgba(1.0, 1.0, 1.0, 0.35)).radius(16.0)),
			),
		);
		// A bordered box that slides into place on startup.
		s.spawn_with(
			win,
			SlidingBox,
			(
				LayoutStyle::default().size(px(160.0), px(60.0)),
				Paint::Quad(
					Quad::new(Color::rgb(0.95, 0.8, 0.3))
						.radius(8.0)
						.border_widths(Insets::new(2.0, 8.0, 4.0, 2.0))
						.border_color(Color::rgb(0.6, 0.2, 0.1)),
				),
			),
		);
		Shell
	}
}

fn main() {
	let mut app = App::new();
	app.add_module(LayoutModule)
		.add_module(PaintModule)
		.add_module(WindowModule)
		.add_module(AnimationModule)
		.add_module(RenderModule::default())
		.insert_resource(Atlas::new());
	app.add_module(RingModule::default())
		.add_module(
			WaylandModule::new()
				.bind::<WlCompositor>()
				.bind::<ZwpLinuxDmabufV1>()
				.bind::<XdgWmBase>(),
		)
		.add_module(PresentationModule {
			app_id: "mecha.showcase".into(),
			budget: Budget::default(),
		});
	let root = app.root();
	app.spawn(root, ShellBuilder { root });
	app.run();
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Instant;

	#[test]
	fn the_box_slides_on_first_layout_and_does_not_restart_on_resize() {
		let start = Instant::now();
		let mut time = Time::manual();
		time.update_at(start);
		let mut app = App::new();
		app.insert_resource(time);
		app.add_module(LayoutModule).add_module(AnimationModule);
		let node = app.spawn_with(
			app.root(),
			SlidingBox,
			(
				LayoutRoot(true),
				LayoutStyle::default().size(px(160.0), px(60.0)),
			),
		);
		app.tick();
		assert_eq!(app.component::<Layout>(node).unwrap().rect.x(), 260.0);
		assert_eq!(app.component::<ComputedLayout>(node).unwrap().rect.x(), 0.0);

		app.resource_mut::<Time>()
			.update_at(start + Duration::from_millis(750));
		app.tick();
		assert_eq!(app.component::<Layout>(node).unwrap().rect.x(), 32.5);

		app.resource_mut::<Time>()
			.update_at(start + Duration::from_millis(1500));
		app.tick();
		assert_eq!(app.component::<Layout>(node).unwrap().rect.x(), 0.0);

		app.component_mut::<LayoutStyle>(node).unwrap().width = px(180.0);
		app.tick();
		let layout = app.component::<Layout>(node).unwrap();
		assert_eq!(layout.rect.x(), 0.0);
		assert_eq!(layout.rect.width(), 180.0);
	}
}
