pub(crate) mod advect_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/advect.comp",
    }
}

pub(crate) mod diffuse_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/diffuse.comp",
    }
}

pub(crate) mod divergence_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/divergence.comp",
    }
}

pub(crate) mod gradient_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/gradient.comp",
    }
}

pub(crate) mod pressure_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/pressure.comp",
    }
}

pub(crate) mod splat_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/splat.comp",
    }
}

pub(crate) mod display_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../assets/display.vert",
    }
}

pub(crate) mod display_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../assets/display.frag",
    }
}
