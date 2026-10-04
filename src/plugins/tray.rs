use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use bevy::winit::{UpdateMode, WinitSettings};
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const ICON_BYTES: &[u8] = include_bytes!("../../RustyMCU-icon.png");

// Kept alive for the process lifetime; the OS tray entry disappears when dropped.
struct TrayState {
    _icon: TrayIcon,
    show_id: MenuId,
    quit_id: MenuId,
}

pub struct TrayPlugin;

impl Plugin for TrayPlugin {
    fn build(&self, app: &mut App) {
        // Plugin::build runs on the main thread — safe for OS tray creation.
        let state = build_tray_state();
        app.world_mut().insert_non_send_resource(state);
        app.add_systems(Update, (poll_tray_events, minimize_to_tray_on_close))
            .add_systems(Last, drop_tray_on_exit);
    }
}

fn build_tray_state() -> TrayState {
    let img = image::load_from_memory(ICON_BYTES)
        .expect("tray icon png")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let icon = tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("tray icon rgba");

    let show_item = MenuItem::new("Show", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&show_item).expect("tray menu show");
    menu.append(&PredefinedMenuItem::separator())
        .expect("tray menu sep");
    menu.append(&quit_item).expect("tray menu quit");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("RustyMCU")
        .with_icon(icon)
        .build()
        .expect("tray icon build");

    TrayState {
        _icon: tray,
        show_id,
        quit_id,
    }
}

fn poll_tray_events(
    tray: Option<NonSend<TrayState>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut winit_settings: ResMut<WinitSettings>,
    mut app_exit: EventWriter<AppExit>,
) {
    let Some(tray) = tray else { return };

    // Only an actual left-click restores the window — the receiver also
    // delivers Enter/Move/Leave hover events which must not un-hide it.
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        let clicked = matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        );
        if clicked {
            show_window(&mut windows, &mut winit_settings);
        }
    }

    // Context-menu item clicks.
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == tray.quit_id {
            // Hide immediately so quitting feels instant even if teardown
            // takes a moment.
            if let Ok(mut win) = windows.get_single_mut() {
                win.visible = false;
            }
            app_exit.send(AppExit::Success);
        } else if event.id == tray.show_id {
            show_window(&mut windows, &mut winit_settings);
        }
    }
}

fn minimize_to_tray_on_close(
    mut events: EventReader<WindowCloseRequested>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut winit_settings: ResMut<WinitSettings>,
) {
    for _ in events.read() {
        if let Ok(mut win) = windows.get_single_mut() {
            win.visible = false;
        }
        // Throttle to ~4 updates/sec while hidden: the serial buffer keeps
        // draining and the tray stays responsive, but we stop rendering at
        // full tilt in the background.
        let low_power = UpdateMode::reactive_low_power(Duration::from_millis(250));
        winit_settings.focused_mode = low_power;
        winit_settings.unfocused_mode = low_power;
    }
}

fn show_window(
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    winit_settings: &mut WinitSettings,
) {
    if let Ok(mut win) = windows.get_single_mut() {
        win.visible = true;
        win.focused = true;
    }
    winit_settings.focused_mode = UpdateMode::Continuous;
    winit_settings.unfocused_mode = UpdateMode::Continuous;
}

/// Drop the tray icon the moment an exit is requested; otherwise Windows
/// leaves a dead "ghost" icon in the notification area after the process
/// dies, until the user mouses over it.
fn drop_tray_on_exit(world: &mut World) {
    let exiting = world
        .get_resource::<Events<AppExit>>()
        .is_some_and(|ev| !ev.is_empty());
    if exiting {
        world.remove_non_send_resource::<TrayState>();
    }
}
