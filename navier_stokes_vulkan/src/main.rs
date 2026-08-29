// Navier-Stokes fluid simulation on Vulkan compute shaders.
//
// Solves the incompressible Navier-Stokes equations
//     du/dt = -(u·∇)u - (1/ρ)∇p + ν∇²u + f    with    ∇·u = 0
// with Stam's "stable fluids" operator splitting. Every term runs as its own
// compute pass over storage images (see assets/*.comp for the per-term
// mapping); the window shows the dye field carried by the fluid.
//
// Module layout:
// - `gpu`     : instance/device/queue selection and the allocators,
// - `fluid`   : the solver's images, compute pipelines, and dispatch recording,
// - `shaders` : the vulkano_shaders modules wrapping assets/*.comp|vert|frag,
// - `renderer`: swapchain and the fullscreen display pipeline,
// - `app`     : winit event handling, pointer forces, and the frame loop,
// - `debug`   : headless verification (NS_CHECK) and frame dumps
//               (NS_DUMP_FRAME=filename).

mod app;
mod debug;
mod fluid;
mod gpu;
mod renderer;
mod shaders;

use std::error::Error;

use winit::event_loop::EventLoop;

use crate::app::App;

fn main() -> Result<(), impl Error> {
    if std::env::var_os("NS_CHECK").is_some() {
        if !debug::headless_check() {
            std::process::exit(1);
        }
        return Ok(());
    }

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(&event_loop);

    event_loop.run_app(&mut app)
}
