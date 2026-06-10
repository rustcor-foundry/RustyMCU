use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

const ICON_BYTES: &[u8] = include_bytes!("../../RustyMCU-icon.png");

// Kept alive for the process lifetime; the OS tray entry disappears when dropped.
struct TrayState {
    _icon: TrayIcon,
    show_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

pub struct TrayPlugin;

impl Plugin for TrayPlugin {
    fn build(&self, app: &mut App) {
        // Plugin::build runs on the main thread — safe for OS tray creation.
        let state = build_tray_state();
        app.world_mut().insert_non_send_resource(state);
        app.add_systems(Update, (poll_tray_events, minimize_to_tray_on_close));
    }
}

fn build_tray_state() -> TrayState {
    let img = image::load_from_memory(ICON_BYTES)
        .expect("tray icon png")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let icon =
        tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("tray icon rgba");

    let show_item = MenuItem::new("Show", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&show_item).expect("tray menu show");
    menu.append(&PredefinedMenuItem::separator()).expect("tray menu sep");
    menu.append(&quit_item).expect("tray menu quit");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("RustyMCU")
        .with_icon(icon)
        .build()
        .expect("tray icon build");

    TrayState { _icon: tray, show_id, quit_id }
}

fn poll_tray_events(
    tray: NonSend<TrayState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut app_exit: EventWriter<AppExit>,
) {
    // Left-click the tray icon → show window.
    while TrayIconEvent::receiver().try_recv().is_ok() {
        show_window(&mut windows);
    }

    // Context-menu item clicks.
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == tray.quit_id {
            app_exit.send(AppExit::Success);
        } else if event.id == tray.show_id {
            show_window(&mut windows);
        }
    }
}

fn minimize_to_tray_on_close(
    mut events: EventReader<WindowCloseRequested>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for _ in events.read() {
        if let Ok(mut win) = windows.get_single_mut() {
            win.visible = false;
        }
    }
}

fn show_window(windows: &mut Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut win) = windows.get_single_mut() {
        win.visible = true;
    }
}
