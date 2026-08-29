Done. **`navier_stokes_vulkan/`** is now a complete, verified Navier–Stokes fluid simulation. I followed the architecture of the two reference projects (`Gpu` → `RenderContext` → winit `App`, `assets/` GLSL, push constants, same dependencies/versions), but with the solver running on **GPU compute shaders** instead of a CPU simulation — because the Jacobi pressure solve is exactly the kind of massively parallel workload Vulkan compute is meant for.

**Mapping from the README's math to compute passes:**

| Term                      | Meaning         | Kernel                                                                  |
| ------------------------- | --------------- | ----------------------------------------------------------------------- |
| **f**                     | External force  | `splat.comp` — injects a Gaussian velocity impulse + hue-cycling dye while dragging |
| **ν∇²u**                  | Viscosity       | `diffuse.comp` — Jacobi iteration of $(I-\nu dt\nabla^2)u^{new}=u^{old}$ |
| **−(1/ρ)∇p**, **∇·u = 0** | Pressure projection | `divergence.comp` → `pressure.comp` (Jacobi, warm-started) → `gradient.comp` |
| **(u·∇)u**                | Nonlinear advection | `advect.comp` — Stam's semi-Lagrangian backtrace (velocity + dye)   |

Frame order: splat → diffuse → divergence → pressure ×N → gradient → advect velocity → advect dye, all scheduled as ping-pong dispatches over storage images (vulkano inserts the barriers automatically). The velocity/pressure grid is 410×256 (square cells, matching the aspect ratio), with dye at 2× the resolution; display goes through a fullscreen triangle + manual bilinear `imageLoad` (no reliance on hardware filtering of float textures).

**Verified on your RTX 3060 Ti:**
- `NS_CHECK=1` (compute only, no display) — **passed**: a single projection reduces the divergence of a drag-sized source by **84% (max) / 53% (mean)**, 120 scripted steps stay finite and transport dye, and the viscosity pass behaves correctly at ν=2.0
- `NS_DUMP_FRAME=/tmp/ns_frame.ppm` — the rendered frame shows real vortices carrying hue-cycling dye (verified numerically and via ASCII rendering)
- The windowed app runs, and clippy reports no warnings

**Controls:** drag = stir · space = pause · ↑/↓ = time speed · V/B = viscosity ν · [ ] = pressure iterations · R = reset · Esc = quit (shown live in the window title).

One honest caveat: at the default iteration count, Jacobi converges slowly on *low-frequency* divergence (common to all real-time fluid demos) — the `[` `]` keys adjust that accuracy. Everything is documented in the project README. Nothing has been committed yet; just say the word if you'd like that.
