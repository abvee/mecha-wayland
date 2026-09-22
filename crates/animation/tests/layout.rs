use animation::prelude::*;
use app::prelude::*;
use geometry::{Insets, Rect};
use layout::prelude::*;

struct Click;
impl Event for Click {}

struct HoverEnter;
impl Event for HoverEnter {}

struct TestWidget {
    seen: Option<Layout>,
}

struct TestBuilder {
    attach_handlers: bool,
}

impl Build for TestBuilder {
    type Widget = TestWidget;
}

impl Widget for TestWidget {
    type Builder = TestBuilder;

    fn build(b: TestBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        if b.attach_handlers {
            s.on::<Click>(me, animate_layout)
                .on::<Click>(me, |ctx, _| {
                    let layout = *ctx.component::<Layout>().unwrap();
                    ctx.me().seen = Some(layout);
                })
                .on::<HoverEnter>(me, animate_layout);
        }
        Self { seen: None }
    }
}

fn setup(attach_handlers: bool) -> (App, Handle<TestWidget>) {
    let mut app = App::new();
    app.add_module(LayoutModule);
    let node = app.spawn_with(
        app.root(),
        TestBuilder { attach_handlers },
        (
            LayoutRoot(true),
            LayoutStyle::default()
                .size(px(100.0), px(60.0))
                .padding(Insets::new(px(1.0), px(2.0), px(3.0), px(4.0)))
                .border(Insets::all(px(2.0))),
        ),
    );
    app.tick();
    (app, node)
}

#[test]
fn animation_defaults_to_none() {
    assert_eq!(Animation::default(), Animation::None);
}

#[test]
fn a_user_trigger_restores_the_base_before_the_next_handler_runs() {
    let (mut app, node) = setup(true);
    let computed = *app.component::<ComputedLayout>(node).unwrap();
    let expected = Layout::from(computed);
    *app.component_mut::<Layout>(node).unwrap() = Layout::default();

    app.emit(Click, node);
    assert_eq!(app.component::<Layout>(node), Some(&Layout::default()));
    app.flush();

    assert_eq!(app.component::<Layout>(node), Some(&expected));
    assert_eq!(app.component::<ComputedLayout>(node), Some(&computed));
    assert_eq!(app.widget::<TestWidget>(node).unwrap().seen, Some(expected));
}

#[test]
fn the_same_handler_accepts_another_event_and_equal_output_is_not_a_change() {
    let (mut app, node) = setup(true);
    let expected = Layout::from(*app.component::<ComputedLayout>(node).unwrap());
    app.component_mut::<Layout>(node).unwrap().rect = Rect::ZERO;
    app.take_changed::<Layout>().for_each(drop);

    app.emit(HoverEnter, node);
    app.flush();
    assert_eq!(app.component::<Layout>(node), Some(&expected));
    assert_eq!(
        app.take_changed::<Layout>().collect::<Vec<_>>(),
        [node.id()]
    );

    app.emit(HoverEnter, node);
    app.flush();
    assert!(app.take_changed::<Layout>().next().is_none());
}

#[test]
fn the_module_does_not_attach_handlers_or_convert_layout_on_its_own() {
    let (mut app, node) = setup(false);
    app.add_module(AnimationModule);
    app.component_mut::<Layout>(node).unwrap().rect = Rect::ZERO;

    app.emit(Click, node);
    app.emit(HoverEnter, node);
    app.tick();
    assert_eq!(app.component::<Layout>(node).unwrap().rect, Rect::ZERO);
    assert_ne!(
        app.component::<ComputedLayout>(node).unwrap().rect,
        Rect::ZERO
    );
}
