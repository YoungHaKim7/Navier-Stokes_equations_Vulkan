
Done — `docs/README.md` is now in English. The original file was the Chinese summary (same content as `docs/README_cn.md`), so this was a straight in-place translation with nothing lost — the Chinese version is preserved in `README_cn.md` and in git history.

The translation covers:
- The intro describing `navier_stokes_vulkan/` as a GPU-compute Navier–Stokes sim following the reference architecture
- The **math-term → compute-pass table** (f → `splat.comp`, ν∇²u → `diffuse.comp`, pressure projection → `divergence/pressure/gradient.comp`, advection → `advect.comp`)
- Frame order, grid resolution, and display path
- Verification results (`NS_CHECK=1`, `NS_DUMP_FRAME`, clippy)
- Controls and the Jacobi low-frequency convergence caveat

I left the root `README.md` (the NS-equations notes, which contain Korean rather than Chinese) untouched. If you'd like that one translated to English too, just say so.
