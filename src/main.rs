mod app;
mod render;
mod scene;
mod tile;
mod tools;
mod ui;
mod input;
mod history;
mod io;
mod anim;
mod util;

use app::App;

fn main() {
    env_logger::init();
    log::info!("Starting Cracktile 3D");

    let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).expect("event loop error");
}
