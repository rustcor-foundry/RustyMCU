// Hide the console window in release builds; keep it in debug for easy log access.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WinitWindows;
use bevy_egui::EguiPlugin;

mod plugins;
mod state;
mod ui;

const ICON_BYTES: &[u8] = include_bytes!("../RustyMCU-icon.png");

fn main() {
    let exit = App::new()
        // Matches theme::BG so resize/startup frames don't flash a mismatched color.
        .insert_resource(ClearColor(Color::srgb_u8(0x06, 0x0b, 0x12)))
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "RustyMCU".into(),
                    resolution: (1100.0_f32, 580.0_f32).into(),
                    ..default()
                }),
                // The tray plugin intercepts close requests; we handle quitting
                // ourselves via the tray "Quit" item or AppExit event.
                close_when_requested: false,
                ..default()
            }),
        )
        .add_plugins(EguiPlugin)
        .add_plugins(plugins::BoardPlugins)
        .add_plugins(ui::UiPlugin)
        .add_systems(Startup, set_window_icon)
        .run();

    // Belt and braces: background IO threads (serial, TCP, probe, disk) run
    // infinite loops, and wgpu teardown has been known to stall on Windows.
    // Force the process down once the app loop has finished.
    std::process::exit(match exit {
        AppExit::Success => 0,
        AppExit::Error(code) => code.get() as i32,
    });
}

/// Sets the window icon from the embedded PNG.  Must run on the main thread,
/// which Bevy guarantees for NonSend systems.
fn set_window_icon(
    winit_windows: NonSend<WinitWindows>,
    primary: Query<Entity, With<PrimaryWindow>>,
) {
    let Ok(entity) = primary.get_single() else { return };
    let Some(win) = winit_windows.get_window(entity) else { return };

    let img = image::load_from_memory(ICON_BYTES)
        .expect("window icon png")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let icon = winit::window::Icon::from_rgba(img.into_raw(), w, h)
        .expect("winit window icon");
    win.set_window_icon(Some(icon));
}
