use native_windows_gui as nwg;
use native_windows_derive as nwd;

use nwd::NwgUi;
use nwg::NativeUi;
use std::cell::RefCell;
use crate::state;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use crate::api::models::{Note, strip_tags_multiline};
use winapi::um::winuser::*;

#[derive(Default, NwgUi)]
pub struct LaunchBar {
    #[nwg_resource(family: "Segoe UI", size: 18)]
    font: nwg::Font,

    #[nwg_resource(source_file: Some("resources/icon.ico"))]
    icon: nwg::Icon,

    #[nwg_control(size: (600, 400), position: (300, 300), title: "MiNote Launch Bar", flags: "WINDOW", ex_flags: 0x00000008, icon: Some(&data.icon))]
    #[nwg_events( OnWindowClose: [LaunchBar::hide], OnKeyPress: [LaunchBar::handle_key(SELF, EVT_DATA)] )]
    window: nwg::Window,

    #[nwg_control(size: (580, 24), position: (10, 10), font: Some(&data.font), focus: true)]
    #[nwg_events( OnTextInput: [LaunchBar::on_input_changed], OnKeyPress: [LaunchBar::handle_key(SELF, EVT_DATA)] )]
    input: nwg::TextInput,

    #[nwg_control(size: (580, 340), position: (10, 55), font: Some(&data.font), flags: "VISIBLE")]
    #[nwg_events( OnListBoxSelect: [LaunchBar::on_select], OnListBoxDoubleClick: [LaunchBar::on_confirm] )]
    results_list: nwg::ListBox<String>,

    matcher: SkimMatcherV2,
    current_results: RefCell<Vec<Note>>,
}

impl LaunchBar {
    pub fn show(&self) {
        self.center_window();
        self.window.set_visible(true);
        self.input.set_focus();
        // Force refresh on show
        self.on_input_changed();
    }

    fn center_window(&self) {
        let (screen_w, screen_h) = unsafe {
            (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
        };
        let (win_w, win_h) = self.window.size();
        let x = (screen_w - win_w as i32) / 2;
        let y = (screen_h - win_h as i32) / 2;
        self.window.set_position(x, y);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    fn on_input_changed(&self) {
        let query = self.input.text();
        let notes = state::get_notes();

        dprintln!("[LaunchBar] Input changed: query='{}', notes_in_cache={}", query, notes.len());
        if notes.len() > 0 {
            dprintln!("[LaunchBar] Example note in cache: {}", notes[0].display_title());
        }

        let mut matches: Vec<(i64, Note)> = if query.is_empty() {
            notes.into_iter().map(|n| (0, n)).collect()
        } else {
            let query_lower = query.to_lowercase();
            notes.into_iter()
                .filter_map(|note| {
                    let clean_title = note.display_title();
                    let clean_snippet = note.clean_snippet();

                    let s_match = self.matcher.fuzzy_match(&clean_title, &query).unwrap_or(0);
                    let sn_match = self.matcher.fuzzy_match(&clean_snippet, &query).unwrap_or(0);
                    let score = std::cmp::max(s_match, sn_match);

                    let contains_match = clean_title.to_lowercase().contains(&query_lower) ||
                                         clean_snippet.to_lowercase().contains(&query_lower);

                    if score > 0 || contains_match {
                        Some((if score > 0 { score } else { 1 }, note))
                    } else {
                        None
                    }
                })
                .collect()
        };

        if !query.is_empty() {
            matches.sort_by(|a, b| b.0.cmp(&a.0));
        }

        let top_matches: Vec<Note> = matches.into_iter()
            .take(20)
            .map(|(_, n)| n)
            .collect();

        *self.current_results.borrow_mut() = top_matches.clone();

        dprintln!("Found {} matches. Pushing to listbox...", top_matches.len());

        self.results_list.clear();
        for note in &top_matches {
            self.results_list.push(note.display_title());
        }

        if !top_matches.is_empty() {
            self.results_list.set_selection(Some(0));
        }
    }

    fn handle_key(&self, data: &nwg::EventData) {
        if let nwg::EventData::OnKey(key) = data {
            // Check for Ctrl manually using winapi if nwg::GlobalCursor is problematic
            let ctrl = unsafe { GetKeyState(VK_CONTROL) < 0 };

            match *key {
                nwg::keys::ESCAPE => self.hide(),
                nwg::keys::RETURN => self.on_confirm(),
                nwg::keys::DOWN => {
                    let sel = self.results_list.selection();
                    let count = self.results_list.len();
                    if count > 0 {
                        let next = sel.map(|i| (i + 1) % count).unwrap_or(0);
                        self.results_list.set_selection(Some(next));
                    }
                }
                nwg::keys::UP => {
                    let sel = self.results_list.selection();
                    let count = self.results_list.len();
                    if count > 0 {
                        let next = sel.map(|i| if i == 0 { count - 1 } else { i - 1 }).unwrap_or(0);
                        self.results_list.set_selection(Some(next));
                    }
                }
                _ => {
                    if ctrl {
                        match *key {
                            nwg::keys::_A => { self.set_input_sel(0, 0); }
                            nwg::keys::_E => {
                                let len = self.input.text().len() as i32;
                                self.set_input_sel(len, len);
                            }
                            nwg::keys::_F => {
                                let (start, _) = self.get_input_sel();
                                let len = self.input.text().len() as i32;
                                if start < len { self.set_input_sel(start + 1, start + 1); }
                            }
                            nwg::keys::_B => {
                                let (start, _) = self.get_input_sel();
                                if start > 0 { self.set_input_sel(start - 1, start - 1); }
                            }
                            nwg::keys::_N => { self.handle_key(&nwg::EventData::OnKey(nwg::keys::DOWN)); }
                            nwg::keys::_P => { self.handle_key(&nwg::EventData::OnKey(nwg::keys::UP)); }
                            nwg::keys::_D => {
                                // Delete forward
                                let (start, _) = self.get_input_sel();
                                let len = self.input.text().len() as i32;
                                if start < len {
                                    self.set_input_sel(start, start + 1);
                                    if let Some(hwnd) = self.input.handle.hwnd() {
                                        unsafe { SendMessageW(hwnd as _, WM_CHAR, 8, 0); } // Backspace actually works here if we select forward
                                    }
                                }
                            }
                            nwg::keys::_H => {
                                // Backspace
                                if let Some(hwnd) = self.input.handle.hwnd() {
                                    unsafe { SendMessageW(hwnd as _, WM_CHAR, 8, 0); }
                                }
                            }
                            nwg::keys::_W => {
                                let (start, _) = self.get_input_sel();
                                let text = self.input.text();
                                let mut new_start = start as usize;
                                let chars: Vec<char> = text.chars().collect();
                                if new_start > 0 {
                                    new_start -= 1;
                                    while new_start > 0 && !chars[new_start].is_whitespace() {
                                        new_start -= 1;
                                    }
                                }
                                self.set_input_sel(new_start as i32, start);
                                if let Some(hwnd) = self.input.handle.hwnd() {
                                    unsafe { SendMessageW(hwnd as _, WM_CHAR, 8, 0); }
                                }
                            }
                            nwg::keys::_K => {
                                let (start, _) = self.get_input_sel();
                                let len = self.input.text().len() as i32;
                                self.set_input_sel(start, len);
                                if let Some(hwnd) = self.input.handle.hwnd() {
                                    unsafe { SendMessageW(hwnd as _, WM_CHAR, 8, 0); }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn set_input_sel(&self, start: i32, end: i32) {
        if let Some(hwnd) = self.input.handle.hwnd() {
            unsafe { SendMessageW(hwnd as _, EM_SETSEL as u32, start as usize, end as isize); }
        }
    }

    fn get_input_sel(&self) -> (i32, i32) {
        let mut start: u32 = 0;
        let mut end: u32 = 0;
        if let Some(hwnd) = self.input.handle.hwnd() {
            unsafe { SendMessageW(hwnd as _, EM_GETSEL as u32, &mut start as *mut u32 as usize, &mut end as *mut u32 as usize as isize); }
        }
        (start as i32, end as i32)
    }

    fn on_select(&self) {
        // Selection changed
    }

    fn on_confirm(&self) {
        if let Some(index) = self.results_list.selection() {
            let matches = self.current_results.borrow();
            if let Some(note) = matches.get(index) {
                self.perform_action(note);
            }
        }
        self.hide();
    }

    fn perform_action(&self, note: &Note) {
        let config = crate::api::AppConfig::load();

        // Use full content if available, fallback to snippet
        let raw_content = note.content.clone().unwrap_or_else(|| note.snippet.clone());

        // Decode HTML entities (e.g., &amp; -> &) and strip all <tags>
        let clean_content = strip_tags_multiline(&raw_content);

        if config.destination == "Clipboard" {
            set_clipboard_text(&clean_content);
        } else {
            // Previous Program
            self.hide(); // Must hide first to return focus
            std::thread::sleep(std::time::Duration::from_millis(500)); // Longer wait for safety
            type_text(&clean_content);
        }
    }
}

fn set_clipboard_text(text: &str) {
    use widestring::U16String;
    use winapi::um::winuser::{OpenClipboard, EmptyClipboard, SetClipboardData, CloseClipboard, CF_UNICODETEXT};
    use winapi::um::winbase::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            EmptyClipboard();
            let wide = U16String::from_str(text);
            let size = (wide.len() + 1) * 2;
            let h_mem = GlobalAlloc(GMEM_MOVEABLE, size);
            if !h_mem.is_null() {
                let ptr = GlobalLock(h_mem) as *mut u16;
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                *ptr.add(wide.len()) = 0;
                GlobalUnlock(h_mem);
                SetClipboardData(CF_UNICODETEXT, h_mem);
            }
            CloseClipboard();
        }
    }
}

fn type_text(text: &str) {
    // Basic SendInput implementation for previous focus
    use winapi::um::winuser::{SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_UNICODE, KEYEVENTF_KEYUP};

    let wide: Vec<u16> = text.encode_utf16().collect();
    for &ch in &wide {
        unsafe {
            let mut inputs: [INPUT; 2] = std::mem::zeroed();

            inputs[0].type_ = INPUT_KEYBOARD;
            let ki = inputs[0].u.ki_mut();
            ki.wVk = 0;
            ki.wScan = ch;
            ki.dwFlags = KEYEVENTF_UNICODE;

            inputs[1].type_ = INPUT_KEYBOARD;
            let ki = inputs[1].u.ki_mut();
            ki.wVk = 0;
            ki.wScan = ch;
            ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;

            SendInput(2, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
    }
}

pub use launch_bar_ui::LaunchBarUi;
static mut GLOBAL_LAUNCH_BAR: Option<LaunchBarUi> = None;

pub fn init_launch_bar() {
    unsafe {
        GLOBAL_LAUNCH_BAR = Some(LaunchBar::build_ui(Default::default()).expect("Failed to build LaunchBar UI"));
    }
}

pub fn toggle_launch_bar() {
    unsafe {
        if let Some(ref lb) = GLOBAL_LAUNCH_BAR {
            if lb.window.visible() {
                lb.hide();
            } else {
                if let Some(hwnd) = lb.window.handle.hwnd() {
                   SetForegroundWindow(hwnd as _);
                }
                lb.show();
            }
        }
    }
}
