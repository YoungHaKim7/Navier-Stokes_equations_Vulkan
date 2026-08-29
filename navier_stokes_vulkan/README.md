# navier_stokes_vulkan

<div>
  <!-- Rust version -->
  <a href="https://www.rust-lang.org/" rel="nofollow noopener noreferrer"><img src="https://img.shields.io/badge/Rust-1.98+-orange.svg" alt="Rust">
  </a>

  <!-- Vulkan version -->
  <a href="https://www.vulkan.org/" rel="nofollow noopener noreferrer">
    <img src="https://img.shields.io/badge/Vulkan-1.3+-red.svg" alt="Vulkan">
  </a>
</div>

- Real-time 2D fluid simulation of the **incompressible Navier–Stokes equations**
  in Rust + [Vulkan](https://vulkan.org/) compute shaders.
- The solver is Stam's ["stable fluids"](https://d2f99xq7vri1nk.cloudfront.net/legacy_app_files/bs140.pdf)
  operator splitting; every term of the equation runs as its own compute pass
  over storage images, and the window displays the dye carried by the fluid.

# The equations being solved

From the repository's main README — the incompressible Navier–Stokes momentum
equation with constant density:

$$\frac{\partial \mathbf{u}}{\partial t}+(\mathbf{u}\cdot\nabla)\mathbf{u}=-\frac{1}{\rho}\nabla p+\nu\nabla^2\mathbf{u}+\mathbf{f}$$

together with the incompressibility condition:

$$\nabla\cdot\mathbf{u}=0$$

Each term maps to one compute pass (GLSL in `assets/`):

| Term | Meaning | Compute pass |
| --- | --- | --- |
| $\mathbf{f}$ | external force | `splat.comp` — pointer drags inject a Gaussian velocity impulse + dye |
| $\nu\nabla^2\mathbf{u}$ | viscosity (kinematic viscosity ν) | `diffuse.comp` — Jacobi solve of $(I-\nu\,dt\,\nabla^2)u^{new}=u^{old}$ |
| $-\frac{1}{\rho}\nabla p$ | pressure force (ρ = 1) | `divergence.comp` + `pressure.comp` + `gradient.comp` |
| $(\mathbf{u}\cdot\nabla)\mathbf{u}$ | convection (the famous nonlinear term) | `advect.comp` — unconditionally stable semi-Lagrangian backtrace |
| $\nabla\cdot\mathbf{u}=0$ | incompressibility | the projection above: solve $\nabla^2 p = \nabla\cdot\mathbf{u}$, then subtract $\nabla p$ |

Per frame the passes run in order: **splat → diffuse → divergence → pressure
(Jacobi, warm-started) → gradient → advect velocity → advect dye**. The velocity
and pressure grids are 256 cells high (width follows the window aspect so cells
stay square); the dye grid is 2× finer in each dimension. All fields ping-pong
between storage images; vulkano inserts the barriers between dispatches.

# Run

```bash
$ cargo run --release

Navier-Stokes fluid simulation (Stam's stable fluids on Vulkan compute)
Controls: drag = stir | space = pause | up/down = time speed | V/B = viscosity
[ ] = pressure iterations | R = reset | Esc = quit
Using device: NVIDIA GeForce RTX 3060 Ti (type: DiscreteGpu)
Simulation grid: 410x256 cells (velocity/pressure), dye at 2x
```

Drag with the left mouse button to stir the fluid; the injected dye cycles
through hues as you move.

## Controls

| Input | Action |
| --- | --- |
| drag | inject force + dye (external force **f**) |
| space | pause |
| up / down | time speed ×1.5 (0.1×–16×) |
| V / B | viscosity ν down / up (the **ν∇²u** term; 0 = off) |
| [ / ] | pressure Jacobi iterations −8 / +8 (how exactly **∇·u = 0** is enforced) |
| R | reset the fields |
| Esc | quit |

# Verification without interaction

```bash
# Compute-only check, no window or display server needed:
#   - a splatted field has significant divergence,
#   - one pressure projection reduces it sharply,
#   - 120 scripted steps stay finite and carry dye,
#   - the viscosity pass runs cleanly at ν = 2.
$ NS_CHECK=1 cargo run --release
NS_CHECK using device: NVIDIA GeForce RTX 3060 Ti (type: DiscreteGpu)
divergence before projection: max=61.143 mean=0.03884
divergence after  projection: max=9.627 mean=0.01835
projection reduces divergence by 84% (max) / 53% (mean): PASS
...
NS_CHECK: PASS

# Render one frame after 3 s of scripted stirring to a PPM image:
$ NS_DUMP_FRAME=/tmp/frame.ppm cargo run --release
```

# Module layout

- `gpu` — instance/device/queue selection and the allocators,
- `fluid` — the solver's storage images, compute pipelines, and dispatch recording,
- `shaders` — `vulkano_shaders` modules wrapping `assets/*.comp|vert|frag`,
- `renderer` — swapchain and the fullscreen-triangle display pipeline,
- `app` — winit event handling, pointer forces, and the frame loop,
- `debug` — the `NS_CHECK` / `NS_DUMP_FRAME` verification modes.

# Notes

- Boundary conditions are free-slip walls: the normal velocity component is
  zeroed on border cells, and the pressure solve uses a homogeneous Neumann
  condition (∂p/∂n = 0) via clamped edge sampling.
- The pressure Poisson solve absorbs dt and ρ = 1 (as in common real-time
  implementations); the Jacobi iteration count is the accuracy knob.
- Hardware filtering of float textures is not assumed anywhere — the
  semi-Lagrangian lookups do their own bilinear interpolation with `imageLoad`.
