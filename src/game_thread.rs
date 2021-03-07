use crate::components::*;
use crate::rendering_system::RenderingSystem;
use crate::resources::*;
use crate::systems::*;
use glutin::event::VirtualKeyCode;
use glutin::window::Window;
use glutin::{ContextWrapper, NotCurrent, PossiblyCurrent, WindowedContext};
use skia_safe::gpu::Context as GpuContext;
use specs::{Dispatcher, DispatcherBuilder, World, WorldExt};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum EventLoopMsg {
    Quit,
    KeyPressed(VirtualKeyCode),
    KeyReleased(VirtualKeyCode),
    Resized(u32, u32),
    MouseMovedBy(f64, f64),
}

pub enum GameThreadMsg {
    Quit,
}

pub fn game_thread(
    window_ctx: WindowedContext<NotCurrent>,
    event_loop_msg_rec: Receiver<EventLoopMsg>,
    game_thread_sender: Sender<GameThreadMsg>,
) {
    {
        let window_ctx = {
            let gl_window = unsafe { window_ctx.make_current() }.unwrap();
            gl::load_with(|addr| gl_window.get_proc_address(&addr));
            Rc::new(gl_window)
        };
        let gpu_context = {
            let ctx = skia_safe::gpu::Context::new_gl(None).unwrap();
            Rc::new(RefCell::new(ctx))
        };

        let (mut world, mut game_dispatcher) =
            make_gameplay_world(window_ctx.clone(), gpu_context.clone());

        let render_each = Duration::from_micros(1380); // 720 fps
        let mut started_at;
        let mut elapsed = render_each;
        let mut exit = false;

        game_dispatcher.setup(&mut world);

        loop {
            started_at = Instant::now();
            update_time(&mut world, elapsed);
            game_dispatcher.dispatch(&mut world);
            world.maintain();
            window_ctx.swap_buffers().unwrap();

            {
                let mut event_loop_msgs = world.fetch_mut::<Vec<EventLoopMsg>>();
                let mut game_events = world.fetch_mut::<GameEvents>();
                event_loop_msgs.clear();
                game_events.clear();
            }

            for msg in event_loop_msg_rec.try_iter() {
                match msg {
                    EventLoopMsg::Quit => {
                        exit = true;
                    }
                    EventLoopMsg::Resized(w, h) => {
                        let mut events = world.fetch_mut::<GameEvents>();
                        events.emit(GameEvent::WindowResized((w,h)));
                    }
                    _ => {}
                }

                let mut ev_loop_msgs = world.fetch_mut::<Vec<EventLoopMsg>>();
                ev_loop_msgs.push(msg);
            }

            let render_time = started_at.elapsed();

            if render_each > render_time {
                std::thread::sleep(render_each - render_time);
            }

            elapsed = started_at.elapsed();

            if exit {
                break;
            }
        }
    }

    // ^
    // new scope to make sure to clean up everything before
    // exiting the application

    // closing the game thread should make the main thread exit as well.
    game_thread_sender.send(GameThreadMsg::Quit).unwrap();
}

fn make_gameplay_world<'a>(
    window_ctx: Rc<ContextWrapper<PossiblyCurrent, Window>>,
    gpu_context: Rc<RefCell<GpuContext>>,
) -> (World, Dispatcher<'a, 'a>) {
    let mut world = World::new();

    // components
    world.register::<GamePos>();
    world.register::<Circle>();
    world.register::<Slider>();
    world.register::<Lifetime>();
    world.register::<CircleHitRating>();
    world.register::<DespawnObject>();
    world.register::<HitSound>();

    // resources
    world.insert(GameCursor {
        window_x: 0.0,
        window_y: 0.0,
    });
    world.insert(Time::default());
    world.insert(Trail::default());
    world.insert(TrailTimer::default());
    world.insert(Hp::default());
    world.insert(GameArea::default());
    world.insert(Vec::<EventLoopMsg>::with_capacity(8));
    world.insert(GameInputState::default());
    world.insert(Combo::default());
    world.insert(Score::default());
    world.insert(GameEvents::default());

    let game_dispatcher = DispatcherBuilder::new()
        .with(InputSystem, "input-system", &[])
        .with(TrailSystem, "trail-system", &["input-system"])
        .with(ObjectSpawnerSystem::default(), "object-spawner-system", &[])
        .with(HitSystem, "hit-system", &["object-spawner-system", "input-system"])
        .with(CircleLifetimeSystem, "circle-lifetime-system", &["hit-system"])
        .with(SliderLifetimeSystem, "slider-lifetime-system", &["hit-system"])
        .with(LifetimeSystem, "lifetime-system" , &["circle-lifetime-system", "slider-lifetime-system"])
        .with(ScoringSystem, "scoring-system", &["lifetime-system"])
        .with_thread_local(AudioSystem::default())
        .with_thread_local(RenderingSystem::new(window_ctx, gpu_context))
        .build();

    return (world, game_dispatcher);
}

fn update_time(world: &mut World, elapsed: Duration) {
    let mut time = world.write_resource::<Time>();
    let elapsed_in_secs = elapsed.as_secs_f64();
    time.delta = elapsed;
    time.delta_seconds = elapsed_in_secs;
    time.secs_since_start += elapsed_in_secs;
    time.now = Instant::now();
}
