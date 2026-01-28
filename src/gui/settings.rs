use native_windows_gui as nwg;
use native_windows_derive as nwd;

use nwd::NwgUi;
use nwg::NativeUi;
use std::rc::Rc;
use crate::api::AppConfig;

#[derive(Default, NwgUi)]
pub struct SettingsWindow {
    #[nwg_resource(family: "Segoe UI", size: 16)]
    font: nwg::Font,

    #[nwg_resource(source_file: Some("resources/icon.ico"))]
    icon: nwg::Icon,

    #[nwg_control(size: (400, 300), position: (400, 400), title: "Settings - MiNote", flags: "WINDOW", icon: Some(&data.icon))]
    #[nwg_events( OnWindowClose: [SettingsWindow::hide] )]
    window: nwg::Window,

    #[nwg_control(size: (380, 25), position: (10, 10), text: "全局热键 (默认 Alt-L):", font: Some(&data.font))]
    label1: nwg::Label,

    #[nwg_control(size: (380, 30), position: (10, 40), text: "Alt-L", font: Some(&data.font))]
    hotkey_input: nwg::TextInput,

    #[nwg_control(size: (380, 25), position: (10, 80), text: "选择后去向:", font: Some(&data.font))]
    label2: nwg::Label,

    #[nwg_control(text: "剪切板 (Clipboard)", position: (10, 110), size: (300, 25), font: Some(&data.font), check_state: nwg::RadioButtonState::Checked)]
    dest_clipboard: nwg::RadioButton,

    #[nwg_control(text: "上一个程序焦点 (Type text)", position: (10, 140), size: (300, 25), font: Some(&data.font))]
    dest_type: nwg::RadioButton,

    #[nwg_control(size: (100, 35), position: (280, 250), text: "保存", font: Some(&data.font))]
    #[nwg_events( OnButtonClick: [SettingsWindow::save] )]
    save_button: nwg::Button,
}

impl SettingsWindow {
    pub fn show(&self) {
        let config = AppConfig::load();
        self.hotkey_input.set_text(&config.hotkey);
        if config.destination == "Clipboard" {
            self.dest_clipboard.set_check_state(nwg::RadioButtonState::Checked);
        } else {
            self.dest_type.set_check_state(nwg::RadioButtonState::Checked);
        }
        self.window.set_visible(true);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    fn save(&self) {
        let mut config = AppConfig::load();
        config.hotkey = self.hotkey_input.text();
        config.destination = if self.dest_clipboard.check_state() == nwg::RadioButtonState::Checked {
            "Clipboard".to_string()
        } else {
            "PreviousProgram".to_string()
        };
        config.save().ok();
        self.hide();
    }
}

pub use settings_window_ui::SettingsWindowUi;
static mut GLOBAL_SETTINGS_WINDOW: Option<SettingsWindowUi> = None;

pub fn init_settings_window() {
    unsafe {
        GLOBAL_SETTINGS_WINDOW = Some(SettingsWindow::build_ui(Default::default()).expect("Failed to build SettingsWindow UI"));
    }
}

pub fn show_settings() {
    unsafe {
        if let Some(ref sw) = GLOBAL_SETTINGS_WINDOW {
            sw.window.set_visible(true);
        }
    }
}
