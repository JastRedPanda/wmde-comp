// SPDX-License-Identifier: GPL-3.0-only

//! WMDE: which panels drop a shadow. The panel draws nothing outside its own surface, so the
//! shadow is ours, drawn like a window's. Whether a panel wants one is its own setting: the
//! `shadow` key of `fun.wmde.Panel.<name>`, faded by the panel's `opacity` so a see-through
//! panel casts a faint shadow. A panel's layer surface carries the panel name as its namespace,
//! which is how the renderer matches a surface to these settings (`Shell::panel_shadows`).

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_config::ConfigGet;
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use tracing::warn;

use crate::state::State;

/// Config id of the panel container, whose `entries` key lists the panel names.
const PANEL_CONFIG: &str = "fun.wmde.Panel";
const PANEL_CONFIG_VERSION: u64 = 1;

/// What the renderer needs to know about one panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelShadow {
    pub enabled: bool,
    /// The panel's background opacity, 0.0 to 1.0; scales the shadow's alpha.
    pub opacity: f32,
}

fn panel_config(name: &str) -> Option<cosmic_config::Config> {
    cosmic_config::Config::new(&format!("{PANEL_CONFIG}.{name}"), PANEL_CONFIG_VERSION)
        .map_err(|err| warn!(?err, name, "Failed to open the panel config"))
        .ok()
}

fn read(config: &cosmic_config::Config) -> PanelShadow {
    // Both keys fall back the way the panel itself does: `shadow` is on unless switched off,
    // and a config without `opacity` is a broken one, drawn as opaque.
    PanelShadow {
        enabled: config.get::<bool>("shadow").unwrap_or(true),
        opacity: config.get::<f32>("opacity").unwrap_or(1.0).clamp(0.0, 1.0),
    }
}

fn redraw(state: &mut State) {
    let outputs = state
        .common
        .shell
        .read()
        .outputs()
        .cloned()
        .collect::<Vec<_>>();
    for output in outputs {
        state.backend.schedule_render(&output);
    }
}

/// Watches the panel list and every listed panel's config, and keeps `Shell::panel_shadows`
/// in step with them.
pub fn watch<'l>(loop_handle: &LoopHandle<'l, State>) {
    let container = match cosmic_config::Config::new(PANEL_CONFIG, PANEL_CONFIG_VERSION) {
        Ok(config) => config,
        Err(err) => {
            warn!(
                ?err,
                "Failed to open the panel container config, panels get no shadow"
            );
            return;
        }
    };

    // One watcher per panel, dropped when the panel leaves the list.
    let watchers: Rc<RefCell<HashMap<String, RegistrationToken>>> = Rc::default();

    let sync = {
        let loop_handle = loop_handle.clone();
        let watchers = watchers.clone();
        move |container: &cosmic_config::Config, state: &mut State| {
            let names = container.get::<Vec<String>>("entries").unwrap_or_default();
            let mut watchers = watchers.borrow_mut();

            watchers.retain(|name, token| {
                let keep = names.contains(name);
                if !keep {
                    loop_handle.remove(*token);
                }
                keep
            });

            let mut shadows = HashMap::with_capacity(names.len());
            for name in names {
                let Some(config) = panel_config(&name) else {
                    continue;
                };
                shadows.insert(name.clone(), read(&config));

                if watchers.contains_key(&name) {
                    continue;
                }
                let source = match cosmic_config::calloop::ConfigWatchSource::new(&config) {
                    Ok(source) => source,
                    Err(err) => {
                        warn!(?err, name, "Failed to watch the panel config");
                        continue;
                    }
                };
                let panel = name.clone();
                match loop_handle.insert_source(source, move |(config, keys), (), state| {
                    if !keys.iter().any(|key| key == "shadow" || key == "opacity") {
                        return;
                    }
                    let shadow = read(&config);
                    let changed = state
                        .common
                        .shell
                        .write()
                        .panel_shadows
                        .insert(panel.clone(), shadow)
                        != Some(shadow);
                    if changed {
                        redraw(state);
                    }
                }) {
                    Ok(token) => {
                        watchers.insert(name, token);
                    }
                    Err(err) => warn!(?err, name, "Failed to watch the panel config"),
                }
            }

            state.common.shell.write().panel_shadows = shadows;
            redraw(state);
        }
    };

    let initial = container.clone();
    let initial_sync = sync.clone();
    let _ = loop_handle.insert_idle(move |state| initial_sync(&initial, state));

    match cosmic_config::calloop::ConfigWatchSource::new(&container) {
        Ok(source) => {
            if let Err(err) = loop_handle.insert_source(source, move |(config, keys), (), state| {
                if keys.iter().any(|key| key == "entries") {
                    sync(&config, state);
                }
            }) {
                warn!(?err, "Failed to watch the panel container config");
            }
        }
        Err(err) => warn!(?err, "Failed to watch the panel container config"),
    }
}
