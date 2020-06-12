use glium::Display;
use glutin::event;
use glutin::event_loop;
use specs::shrev::{EventChannel, ReaderId};
use specs::prelude::*;

use crate::client::*;
use crate::game_loop::Module;
use std::rc::Rc;

pub struct EventLoopSystem {
    events_loop: EventLoop,
    events: Vec<Event>,
}

impl EventLoopSystem {
    pub fn new(events_loop: EventLoop) -> Self {
        Self {
            events_loop,
            events: Vec::with_capacity(128),
        }
    }
}

impl<'a> System<'a> for EventLoopSystem {
    type SystemData = Write<'a, EventChannel<Event>>;

    fn run(&mut self, mut event_handler: Self::SystemData) {
        let events = &mut self.events;
        let events_loop = &mut self.events_loop;
        // !! TODO: 这里的用法完全变了 event_loop 变成了最外层循环 需要调整
        // events_loop.run(|event, _, control_flow| {
        //     match event.to_static() {
        //         Some(static_event) => events.push(static_event),
        //         _ => ()
        //     };
        // });
        event_handler.drain_vec_write(events);
    }
}

struct SysClientInfo {
    event_reader: Option<ReaderId<Event>>,
}

impl SysClientInfo {
    pub fn new() -> SysClientInfo {
        SysClientInfo { event_reader: None }
    }
}

impl<'a> System<'a> for SysClientInfo {
    type SystemData = (Read<'a, EventChannel<Event>>, WriteExpect<'a, ClientInfo>);

    fn run(&mut self, (events, mut client_info): Self::SystemData) {
        for event in events.read(&mut self.event_reader.as_mut().unwrap()) {
            match event {
                Event::WindowEvent { ref event, .. } => {
                    match event {
                        event::WindowEvent::Focused(focused) => {
                            client_info.is_focused = *focused;
                        }
                        event::WindowEvent::Resized(sz) => {
                            //                            info!("Resized!");
                            client_info.width = sz.width as u32;
                            client_info.height = sz.height as u32;
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }
    }

    fn setup(&mut self, res: &mut World) {
        self.event_reader = Some(res.fetch_mut::<EventChannel<Event>>().register_reader());
    }
}

#[derive(Default, Clone)]
pub struct CursorGrab {
    pub grabbed: bool,
}

struct SysCursorGrab {
    display: Rc<Display>,
    actual_grabbed: Option<bool>,
}

impl<'a> System<'a> for SysCursorGrab {
    type SystemData = (ReadExpect<'a, ClientInfo>, ReadExpect<'a, CursorGrab>);

    fn run(&mut self, (client_info, cursor_grab): Self::SystemData) {
        let should_change = match self.actual_grabbed {
            Some(grabbed) => (*cursor_grab).grabbed != grabbed,
            None => true,
        };

        if should_change {
            let grabbed = (*cursor_grab).grabbed;
            let gl_wnd = self.display.gl_window();
            let wnd = gl_wnd.window();
            wnd.hide_cursor(grabbed);
            wnd.hide_cursor(grabbed);

            self.actual_grabbed = Some(grabbed);
        }

        if let Some(true) = self.actual_grabbed {
            if (*client_info).is_focused {
                let size = self.display.gl_window().window().get_inner_size().unwrap();
                self.display
                    .gl_window()
                    .window()
                    .set_cursor_position(glutin::dpi::LogicalPosition {
                        x: size.width / 2.0,
                        y: size.height / 2.0,
                    })
                    .unwrap();
            }
        }
    }
}

pub struct WindowModule {
    display: Rc<Display>,
    event_loop: Option<EventsLoop>,
}

impl WindowModule {
    pub fn new(display: Rc<Display>, event_loop: EventLoop) -> Self {
        Self {
            display,
            event_loop: Some(event_loop),
        }
    }
}

impl Module for WindowModule {
    fn build(&mut self, init_data: &mut crate::InitData) {
        use crate::InsertInfo;
        init_data.dispatch_thread_local(InsertInfo::new("client_info"), |f| {
            f.insert_thread_local(SysClientInfo::new())
        });

        let event_loop = self.event_loop.take().unwrap();
        init_data.dispatch_thread_local(
            InsertInfo::new("event_loop").order(-1), // event loop should be run in the very beginning
            move |f| f.insert_thread_local(EventLoopSystem::new(event_loop)),
        );

        {
            let display = self.display.clone();
            init_data.dispatch_thread_local(InsertInfo::new("cursor_grab"), move |f| {
                f.insert_thread_local(SysCursorGrab {
                    display,
                    actual_grabbed: None,
                })
            });
        }
    }

    fn on_start(&self, start_data: &mut crate::StartData) {
        let glium::glutin::dpi::LogicalSize { width, height } =
            self.display.gl_window().window().get_inner_size().unwrap();

        start_data.world.insert(ClientInfo {
            width: width as u32,
            height: height as u32,
            is_focused: false,
        });
        start_data.world.insert(CursorGrab { grabbed: false });
    }
}
