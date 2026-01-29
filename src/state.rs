use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use crate::api::models::Note;
use winapi::shared::windef::HWND;
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct SendHwnd(pub HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

pub struct GlobalState {
    pub main_hwnd: Option<SendHwnd>,
    pub notes_cache: Vec<Note>,
}

pub static STATE: Lazy<Arc<Mutex<GlobalState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(GlobalState {
        main_hwnd: None,
        notes_cache: Vec::new(),
    }))
});

pub fn update_notes(notes: Vec<Note>) {
    let mut state = STATE.lock().unwrap();
    crate::dprintln!("Updating state with {} notes", notes.len());
    state.notes_cache = notes;
}

pub fn get_notes() -> Vec<Note> {
    let state = STATE.lock().unwrap();
    state.notes_cache.clone()
}

pub fn set_main_hwnd(hwnd: HWND) {
    let mut state = STATE.lock().unwrap();
    state.main_hwnd = Some(SendHwnd(hwnd));
}
