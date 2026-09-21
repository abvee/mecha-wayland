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
		// A bordered box with uneven sides.
		s.spawn_with(
			win,
			Leaf,
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
