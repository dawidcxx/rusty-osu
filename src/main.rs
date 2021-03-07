mod components;
mod systems;
mod resources;
mod game_thread;
mod consts;
mod rendering_system;
mod utils;
mod osu_parser;

use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::{WindowedContext, NotCurrent, ContextBuilder};
use glutin::window::WindowBuilder;
use glutin::event::{Event, WindowEvent, ElementState};

use crate::game_thread::{game_thread, EventLoopMsg, GameThreadMsg};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use simple_logger::SimpleLogger;
use log::LevelFilter;


fn main() {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init().unwrap();

    let event_loop = EventLoop::new();

    let window_ctx: WindowedContext<NotCurrent> = {
        let window_builder = WindowBuilder::new()
            .with_title("rusty-osu");
        ContextBuilder::new()
            .build_windowed(window_builder, &event_loop)
            .expect("Failed to build window context")
    };

    window_ctx.window().set_cursor_visible(false);

    let (ev_loop_sender, ev_loop_receiver) = mpsc::channel();
    let (game_thread_sender, game_thread_receiver) = mpsc::channel();

    std::thread::Builder::new()
        .name("GameThread".to_string())
        .spawn(move || game_thread(window_ctx, ev_loop_receiver, game_thread_sender))
        .unwrap();

    let mut game_thread_exited = false;

    event_loop.run(move |event, _, ctrl_flow| {
        *ctrl_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        for msg in game_thread_receiver.try_iter() {
            match msg {
                GameThreadMsg::Quit => {
                    // Initialize application shutdown
                    // Some more events might be emitted still
                    *ctrl_flow = ControlFlow::Exit;
                    game_thread_exited = true;
                    log::info!("Closing application..");
                }
            }
        }

        match event {
            Event::WindowEvent {
                event: window_event,
                ..
            } => {
                if game_thread_exited {
                    // don't process outdated window events
                    log::trace!("Application shutting down, skipping window event {:?}", window_event);
                    return;
                }
                match window_event {
                    WindowEvent::Resized(size) => {
                        ev_loop_sender.send(EventLoopMsg::Resized(size.width, size.height))
                            .unwrap();
                    }
                    WindowEvent::KeyboardInput {
                        input,
                        ..
                    } => {
                        if input.state == ElementState::Pressed {
                            if let Some(virtual_key_code) = input.virtual_keycode {
                                ev_loop_sender.send(EventLoopMsg::KeyPressed(virtual_key_code))
                                    .unwrap();
                            }
                        }
                        if input.state == ElementState::Released {
                            if let Some(virtual_key_code) = input.virtual_keycode {
                                ev_loop_sender.send(EventLoopMsg::KeyReleased(virtual_key_code))
                                    .unwrap();
                            }
                        }
                    }
                    WindowEvent::CursorMoved {
                        position,
                        ..
                    } => {
                        ev_loop_sender.send(EventLoopMsg::MouseMovedBy(position.x, position.y))
                            .unwrap();
                    }
                    WindowEvent::CloseRequested => {
                        log::info!("User application shutdown requested");
                        ev_loop_sender.send(EventLoopMsg::Quit).unwrap();
                    }
                    _ => {
                        // unused event
                    }
                }
            }
            _ => {
                // unused event
            }
        }
    });
}