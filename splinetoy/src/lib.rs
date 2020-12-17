mod edit_session;
mod editor;
mod mouse;
mod path;
mod pen;
mod select;
mod toolbar;
mod tools;

use druid::{
    commands, platform_menus, AppLauncher, FileDialogOptions, FileSpec, LocalizedString, MenuDesc,
    MenuItem, SysMods, WindowDesc,
};

use edit_session::EditSession;
use editor::Editor;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn wasm_main() {
    let request_path = web_sys::window().and_then(|win| win.location().search().ok());
    let data = request_path.and_then(EditSession::from_base64_bincode);

    // This hook is necessary to get panic messages in the console
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main(data)
}

pub fn main(data: Option<EditSession>) {
    // describe the main window
    let main_window = WindowDesc::new(|| Editor::new())
        .title("Spline Toy")
        .menu(make_menu())
        .with_min_size((200., 200.))
        .window_size((600.0, 800.0));

    // create the initial app state
    let initial_state = data.unwrap_or(EditSession::new());

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn file_menu() -> MenuDesc<EditSession> {
    pub const JSON_TYPE: FileSpec = FileSpec::new("JSON Data", &["json"]);

    MenuDesc::new(LocalizedString::new("common-menu-file-menu"))
        .append(platform_menus::mac::file::new_file().disabled())
        .append(platform_menus::mac::file::new_file().disabled())
        .append_separator()
        .append(platform_menus::mac::file::close())
        .append(
            MenuItem::new(
                LocalizedString::new("save-as-json").with_placeholder("Save JSON..."),
                commands::SHOW_SAVE_PANEL
                    .with(FileDialogOptions::new().allowed_types(vec![JSON_TYPE])),
            )
            .hotkey(SysMods::Cmd, "s"),
        )
        .append_separator()
        .append(platform_menus::mac::file::page_setup().disabled())
        .append(platform_menus::mac::file::print().disabled())
}

/// The main window/app menu.
#[allow(unused_mut)]
fn make_menu() -> MenuDesc<EditSession> {
    let mut menu = MenuDesc::empty();
    #[cfg(target_os = "macos")]
    {
        menu = menu.append(platform_menus::mac::application::default());
    }

    menu.append(file_menu())
}
