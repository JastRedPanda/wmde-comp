// SPDX-License-Identifier: GPL-3.0-only

//! WMDE: the greeter's picture under everything until the wallpaper arrives.
//!
//! The greeter and the session are two compositors, and between them the screen shows the
//! console. The session's own first frames would show `CLEAR_COLOR` until `wmde-bg` maps its
//! wallpaper; instead they show the picture the greeter was showing, scaled the same way
//! (cover), so the session reads as a continuation of the login screen. Once any output has a
//! wallpaper the picture is dropped for good and never decoded again.

use std::{
    borrow::Borrow,
    cell::RefCell,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            ImportMem, Renderer, Texture,
            element::{Id, Kind, texture::TextureRenderElement},
            gles::{GlesRenderer, GlesTexture},
            glow::GlowRenderer,
            utils::with_renderer_surface_state,
        },
    },
    desktop::layer_map_for_output,
    output::Output,
    utils::{Buffer, Logical, Rectangle, Size, Transform},
    wayland::shell::wlr_layer::Layer,
};
use tracing::warn;

use super::element::AsGlowRenderer;
use crate::utils::prelude::*;

/// The greeter's background, `BACKGROUND_PATH` in wmde-greeter.
const GREETER_BACKGROUND: &str = "/usr/share/wmde-greeter/background.jpg";
/// The layer namespace of wmde-bg's wallpaper surfaces.
const WALLPAPER_NAMESPACE: &str = "wallpaper";

/// Set once a wallpaper has been seen; from then on nothing here does any work.
static DONE: AtomicBool = AtomicBool::new(false);

/// Decoded RGBA pixels and their size, decoded off the render path by `preload`.
static PIXELS: Mutex<Option<(Vec<u8>, Size<i32, Buffer>)>> = Mutex::new(None);
static PRELOAD: OnceLock<()> = OnceLock::new();

/// The texture, per GL context: uploaded on first use, dropped when `DONE` is set.
struct BackdropTexture(RefCell<Option<(Id, GlesTexture)>>);

/// Starts decoding the picture on a thread of its own. Called once at startup; until the
/// pixels are ready, frames show the clear colour as before.
pub fn preload() {
    PRELOAD.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("startup-backdrop".into())
            .spawn(|| match image::open(GREETER_BACKGROUND) {
                Ok(picture) => {
                    let picture = picture.into_rgba8();
                    let size = Size::from((picture.width() as i32, picture.height() as i32));
                    if !DONE.load(Ordering::Acquire) {
                        *PIXELS.lock().unwrap() = Some((picture.into_raw(), size));
                    }
                }
                Err(err) => warn!(?err, GREETER_BACKGROUND, "No startup backdrop"),
            });
        if let Err(err) = spawned {
            warn!(?err, "Failed to start decoding the startup backdrop");
        }
    });
}

fn has_wallpaper(output: &Output) -> bool {
    let map = layer_map_for_output(output);
    map.layers_on(Layer::Background).any(|layer| {
        layer.namespace() == WALLPAPER_NAMESPACE
            && with_renderer_surface_state(layer.wl_surface(), |state| state.buffer().is_some())
                .unwrap_or(false)
    })
}

/// The picture as an element covering `output`, or `None` once the output has its wallpaper,
/// before the picture is decoded, or if there is none.
pub fn element<R: AsGlowRenderer>(
    renderer: &mut R,
    output: &Output,
) -> Option<TextureRenderElement<GlesTexture>> {
    if DONE.load(Ordering::Acquire) {
        return None;
    }

    let glow = renderer.glow_renderer_mut();
    let cached = |glow: &GlowRenderer| -> Option<(Id, GlesTexture)> {
        let gles: &GlesRenderer = glow.borrow();
        let user_data = gles.egl_context().user_data();
        user_data.insert_if_missing(|| BackdropTexture(RefCell::new(None)));
        user_data.get::<BackdropTexture>()?.0.borrow().clone()
    };
    let store = |glow: &GlowRenderer, value: Option<(Id, GlesTexture)>| {
        let gles: &GlesRenderer = glow.borrow();
        if let Some(cache) = gles.egl_context().user_data().get::<BackdropTexture>() {
            *cache.0.borrow_mut() = value;
        }
    };

    if has_wallpaper(output) {
        DONE.store(true, Ordering::Release);
        PIXELS.lock().unwrap().take();
        store(glow, None);
        return None;
    }

    let (id, texture) = match cached(glow) {
        Some(cached) => cached,
        None => {
            let texture = {
                let pixels = PIXELS.lock().unwrap();
                let (data, size) = pixels.as_ref()?;
                match glow.import_memory(data, Fourcc::Abgr8888, *size, false) {
                    Ok(texture) => texture,
                    Err(err) => {
                        warn!(?err, "Failed to upload the startup backdrop");
                        return None;
                    }
                }
            };
            let cached = (Id::new(), texture);
            store(glow, Some(cached.clone()));
            cached
        }
    };

    // Cover: fill the output and crop the overflow evenly, as the greeter draws it.
    let output_size = output.geometry().size.as_logical();
    let picture = texture.size();
    let zoom = (f64::from(output_size.w) / f64::from(picture.w))
        .max(f64::from(output_size.h) / f64::from(picture.h));
    let visible = Size::<f64, Logical>::from((
        f64::from(output_size.w) / zoom,
        f64::from(output_size.h) / zoom,
    ));
    let src = Rectangle::new(
        (
            (f64::from(picture.w) - visible.w) / 2.0,
            (f64::from(picture.h) - visible.h) / 2.0,
        )
            .into(),
        visible,
    );

    Some(TextureRenderElement::from_static_texture(
        id,
        glow.context_id(),
        (0.0, 0.0),
        texture,
        1,
        Transform::Normal,
        None,
        Some(src),
        Some(output_size),
        None,
        Kind::Unspecified,
    ))
}
