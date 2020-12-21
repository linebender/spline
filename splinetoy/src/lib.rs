mod edit_session;
mod editor;
mod mouse;
mod path;
mod pen;
mod save;
mod select;
mod toolbar;
mod tools;

use druid::{
    commands, platform_menus, AppLauncher, FileDialogOptions, FileInfo, FileSpec, LocalizedString,
    MenuDesc, MenuItem, Selector, SysMods, WindowDesc,
};

use edit_session::EditSession;
use editor::Editor;
use save::SessionState;
use tools::ToolId;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub const SAVE_BINARY: Selector<FileInfo> = Selector::new("splinetoy.save-binary");
pub const TOGGLE_PREVIEW_LOCK: Selector = Selector::new("splinetoy.toggle-lock");

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn wasm_main() {
    // This hook is necessary to get panic messages in the console
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let saved_session = match SessionState::init_from_current_url() {
        Ok(session) => Some(session),
        Err(err) => {
            web_sys::console::log_1(&format!("{}", err).into());
            None
        }
    };
    main(saved_session)
}

pub fn main(saved: Option<SessionState>) {
    // describe the main window
    let saved = saved.unwrap_or_default();
    let tool = saved.tool;
    let select_only = saved.select_only;
    let main_window = WindowDesc::new(move || make_editor(tool, select_only))
        .title("Spline Toy")
        .menu(make_menu())
        .with_min_size((200., 200.))
        .window_size((600.0, 800.0));

    // create the initial app state
    let initial_data = saved.into_edit_session();

    // start the application
    AppLauncher::with_window(main_window)
        .launch(initial_data)
        .expect("Failed to launch application");
}

fn file_menu() -> MenuDesc<EditSession> {
    pub const JSON_TYPE: FileSpec = FileSpec::new("JSON Data", &["json"]);
    pub const BINARY_TYPE: FileSpec = FileSpec::new("Binary Data", &["splinetoy"]);

    MenuDesc::new(LocalizedString::new("common-menu-file-menu"))
        .append(platform_menus::mac::file::new_file().disabled())
        .append(
            MenuItem::new(
                LocalizedString::new("common-menu-file-open"),
                commands::SHOW_OPEN_PANEL
                    .with(FileDialogOptions::new().allowed_types(vec![BINARY_TYPE])),
            )
            .hotkey(SysMods::Cmd, "o"),
        )
        .append_separator()
        .append(platform_menus::mac::file::close())
        .append(
            MenuItem::new(
                LocalizedString::new("save-as-binary").with_placeholder("Save Binary..."),
                commands::SHOW_SAVE_PANEL.with(
                    FileDialogOptions::new()
                        .allowed_types(vec![BINARY_TYPE])
                        .accept_command(SAVE_BINARY),
                ),
            )
            .hotkey(SysMods::Cmd, "s"),
        )
        .append(
            MenuItem::new(
                LocalizedString::new("save-as-json").with_placeholder("Save JSON..."),
                commands::SHOW_SAVE_PANEL
                    .with(FileDialogOptions::new().allowed_types(vec![JSON_TYPE])),
            )
            .hotkey(SysMods::CmdShift, "s"),
        )
        .append_separator()
        .append(platform_menus::mac::file::page_setup().disabled())
        .append(platform_menus::mac::file::print().disabled())
}

fn make_editor(tool: ToolId, preview_only: bool) -> Editor {
    Editor::from_saved(tool, preview_only)
}

fn make_debug_menu() -> MenuDesc<EditSession> {
    MenuDesc::new(LocalizedString::new("debug-menu-file-name").with_placeholder("Debug")).append(
        MenuItem::new(
            LocalizedString::new("toggle-preview").with_placeholder("Toggle Preview Lock"),
            TOGGLE_PREVIEW_LOCK,
        ),
    )
}

/// The main window/app menu.
#[allow(unused_mut)]
fn make_menu() -> MenuDesc<EditSession> {
    let mut menu = MenuDesc::empty();
    #[cfg(target_os = "macos")]
    {
        menu = menu.append(platform_menus::mac::application::default());
    }

    menu.append(file_menu()).append(make_debug_menu())
}
