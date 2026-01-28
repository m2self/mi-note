use std::sync::{Arc, Mutex};
use webview2::{Environment, Controller};
use webview2_sys::*;
use winapi::shared::winerror::S_OK;
use winapi::shared::ntdef::{HRESULT, LPWSTR};
use winapi::shared::minwindef::{UINT, WPARAM, LPARAM};
use winapi::shared::windef::{HWND};
use winapi::um::winuser::*;
use winapi::um::combaseapi::{CoTaskMemFree, CoInitializeEx};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::objbase::{COINIT_APARTMENTTHREADED};
use widestring::WideCStr;
use com::{ComPtr, ComRc, interfaces::IUnknown};
use crate::api::AppConfig;

use native_windows_gui as nwg;
use std::rc::Rc;

const INITIAL_URL: &str = "https://i.mi.com/note/h5#/";
const WINDOW_CLASS: &str = "MiNoteWebViewMain";
const WINDOW_TITLE: &str = "Xiaomi Cloud Note WebView";

const WM_TRAY_ICON: UINT = WM_USER + 1;
const TRAY_ICON_ID: UINT = 1;
const HOTKEY_ID: i32 = 100;

use winapi::um::shellapi::{NOTIFYICONDATAW, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIF_ICON, NIF_MESSAGE, NIF_TIP, Shell_NotifyIconW};
use winapi::um::winuser::{RegisterHotKey, UnregisterHotKey, MOD_ALT};
use winapi::um::wingdi::DeleteObject;

static mut GLOBAL_CONTROLLER: Option<Controller> = None;

pub struct WebViewManager {
    pub hwnd: HWND,
    pub cookies: Arc<Mutex<Option<String>>>,
}

impl WebViewManager {
    pub fn new() -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            cookies: Arc::new(Mutex::new(None)),
        }
    }

    pub fn run(&mut self) {
        unsafe {
            nwg::init().expect("Failed to init NWG");

            CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);

            let class_name: Vec<u16> = WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
            let title: Vec<u16> = WINDOW_TITLE.encode_utf16().chain(std::iter::once(0)).collect();

            let h_instance = GetModuleHandleW(std::ptr::null());
            let h_icon = LoadIconW(h_instance, 1 as *const u16); // ID 1 from build.rs

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: h_icon,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: (COLOR_WINDOW + 1) as _,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: h_icon,
            };

            RegisterClassExW(&wc);

            self.hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1200,
                900,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut(),
            );

            if self.hwnd.is_null() { panic!("Failed to create window"); }

            crate::state::set_main_hwnd(self.hwnd);

            // Initialize NWG GUI elements
            crate::gui::launch_bar::init_launch_bar();
            crate::gui::settings::init_settings_window();

            // Register Hotkey (Alt-L)
            let config = AppConfig::load();
            let (mods, vk) = (MOD_ALT as UINT, 0x4C as UINT); // Alt-L
            RegisterHotKey(self.hwnd, HOTKEY_ID, mods, vk);

            // Setup Tray Icon
            setup_tray_icon(self.hwnd);

            ShowWindow(self.hwnd, SW_SHOW);
            UpdateWindow(self.hwnd);

            let cookies_arc = self.cookies.clone();
            let hwnd_val = self.hwnd;

            Environment::builder().build(move |env| {
                let env = env.expect("WebView2 environment creation failed");
                let cookies_arc_inner = cookies_arc.clone();

                env.create_controller(hwnd_val as _, move |controller| {
                    let controller = controller.expect("WebView2 controller creation failed");
                    GLOBAL_CONTROLLER = Some(controller.clone());

                    let mut rect = winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetClientRect(hwnd_val as _, &mut rect);
                    controller.put_bounds(rect).ok();
                    controller.put_is_visible(true).ok();

                    let webview = controller.get_webview().expect("Failed to get webview");
                    webview.navigate(INITIAL_URL).ok();

                    let cookies_arc_nav = cookies_arc_inner.clone();
                    webview.add_navigation_completed(move |wv: webview2::WebView, _args: webview2::NavigationCompletedEventArgs| -> std::result::Result<(), webview2::Error> {
                        if let Ok(uri) = wv.get_source() {
                            if uri.contains("i.mi.com/note/h5") {
                                // Extract cookies
                                let inner = wv.as_inner();
                                let wv2 = inner.get_interface::<dyn ICoreWebView2_2>();

                                if let Some(wv2) = wv2 {
                                    let mut manager_ptr: *mut *mut ICoreWebView2CookieManagerVTable = std::ptr::null_mut();
                                    if wv2.get_cookie_manager(&mut manager_ptr) == S_OK && !manager_ptr.is_null() {
                                        let manager = ComPtr::new(manager_ptr);
                                        IUnknown::add_ref(&manager);
                                        let manager: ComRc<dyn ICoreWebView2CookieManager> = manager.upgrade();

                                        let cookies_arc_final = cookies_arc_nav.clone();
                                        #[allow(unused_must_use)]
                                        let handler = webview2::callback!(ICoreWebView2GetCookiesCompletedHandler, move |res: HRESULT, list_ptr: *mut *mut ICoreWebView2CookieListVTable| -> HRESULT {
                                            if res == S_OK && !list_ptr.is_null() {
                                                let list = ComPtr::new(list_ptr);
                                                IUnknown::add_ref(&list);
                                                let list: ComRc<dyn ICoreWebView2CookieList> = list.upgrade();

                                                let mut count: u32 = 0;
                                                list.get_count(&mut count);

                                                let mut cookie_list = Vec::new();
                                                let (mut has_token, mut has_user, mut has_slh, mut has_ph) = (false, false, false, false);

                                                for i in 0..count {
                                                    let mut cookie_ptr: *mut *mut ICoreWebView2CookieVTable = std::ptr::null_mut();
                                                    if list.get_value_at_index(i, &mut cookie_ptr) == S_OK && !cookie_ptr.is_null() {
                                                        let cookie = ComPtr::new(cookie_ptr);
                                                        IUnknown::add_ref(&cookie);
                                                        let cookie: ComRc<dyn ICoreWebView2Cookie> = cookie.upgrade();

                                                        let mut name_ptr: LPWSTR = std::ptr::null_mut();
                                                        let mut value_ptr: LPWSTR = std::ptr::null_mut();
                                                        let mut domain_ptr: LPWSTR = std::ptr::null_mut();

                                                        cookie.get_name(&mut name_ptr);
                                                        cookie.get_value(&mut value_ptr);
                                                        cookie.get_domain(&mut domain_ptr);

                                                        let name = wide_to_string(name_ptr);
                                                        let value = wide_to_string(value_ptr);
                                                        let domain = wide_to_string(domain_ptr);

                                                        if name == "serviceToken" && !value.is_empty() { has_token = true; }
                                                        if name == "userId" && !value.is_empty() { has_user = true; }
                                                        if name == "i.mi.com_slh" && !value.is_empty() { has_slh = true; }
                                                        if name == "i.mi.com_ph" && !value.is_empty() { has_ph = true; }

                                                        if domain.contains("mi.com") {
                                                            cookie_list.push(format!("{}={}", name, value));
                                                        }
                                                    }
                                                }

                                                if has_token && has_user && has_slh && has_ph {
                                                    let cookie_str = cookie_list.join("; ");
                                                    let cookies_arc_task = cookies_arc_final.clone();
                                                    wv.execute_script("navigator.userAgent", move |res| {
                                                        let ua = res.trim_matches('"').to_string();
                                                        println!("Captured User-Agent: {}", ua);
                                                        let mut config = AppConfig::load();
                                                        config.account_cookie = Some(cookie_str.clone());
                                                        config.user_agent = Some(ua);
                                                        config.save().ok();

                                                        let mut guard = cookies_arc_task.lock().unwrap();
                                                        *guard = Some(cookie_str);
                                                        println!("Cookies updated successfully.");
                                                        Ok(())
                                                    }).ok();
                                                }
                                            }
                                            S_OK
                                        });

                                        manager.get_cookies(std::ptr::null(), handler.as_raw());
                                    }
                                }
                            }
                        }
                        Ok(())
                    }).ok();

                    Ok(())
                }).ok();
                Ok(())
            }).expect("WebView2 build failed");

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            GLOBAL_CONTROLLER = None;
            winapi::um::combaseapi::CoUninitialize();
        }
    }
}

fn wide_to_string(ptr: LPWSTR) -> String {
    if ptr.is_null() { return String::new(); }
    unsafe {
        let s = WideCStr::from_ptr_str(ptr).to_string().unwrap_or_default();
        CoTaskMemFree(ptr as _);
        s
    }
}

const IDM_TRAY_HIDE: usize = 2001;
const IDM_TRAY_RESTORE: usize = 2002;
const IDM_TRAY_SWITCH: usize = 2003;
const IDM_TRAY_SETTINGS: usize = 2004;
const IDM_TRAY_EXIT: usize = 2005;

fn setup_tray_icon(hwnd: HWND) {
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());
        let h_icon = LoadIconW(h_instance, 1 as *const u16);

        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY_ICON;
        nid.hIcon = h_icon;

        let tip: Vec<u16> = "MiNote WebView".encode_utf16().chain(std::iter::once(0)).collect();
        std::ptr::copy_nonoverlapping(tip.as_ptr(), nid.szTip.as_mut_ptr(), tip.len().min(128));

        Shell_NotifyIconW(NIM_ADD, &mut nid);
    }
}

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let h_menu = CreatePopupMenu();
        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_HIDE, "隐藏窗口\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_RESTORE, "恢复窗口\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        AppendMenuW(h_menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_SWITCH, "切换界面 (桌面/手机)\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_SETTINGS, "设置\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        AppendMenuW(h_menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_EXIT, "退出\0".encode_utf16().collect::<Vec<u16>>().as_ptr());

        let mut pos = winapi::shared::windef::POINT { x: 0, y: 0 };
        GetCursorPos(&mut pos);

        SetForegroundWindow(hwnd);
        TrackPopupMenu(h_menu, TPM_RIGHTBUTTON, pos.x, pos.y, 0, hwnd, std::ptr::null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(h_menu);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        WM_DESTROY => {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_ID;
            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            PostQuitMessage(0);
            0
        }
        WM_HOTKEY => {
            if wparam as i32 == HOTKEY_ID {
                println!("Hotkey pressed!");
                // Toggle Launch Bar
                crate::gui::launch_bar::toggle_launch_bar();
            }
            0
        }
        WM_TRAY_ICON => {
            if lparam as UINT == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            } else if lparam as UINT == WM_LBUTTONDBLCLK {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            0
        }
        WM_COMMAND => {
            match wparam as usize {
                IDM_TRAY_HIDE => { ShowWindow(hwnd, SW_HIDE); }
                IDM_TRAY_RESTORE => { ShowWindow(hwnd, SW_RESTORE); SetForegroundWindow(hwnd); }
                IDM_TRAY_EXIT => { DestroyWindow(hwnd); }
                IDM_TRAY_SWITCH => { switch_interface(); }
                IDM_TRAY_SETTINGS => { crate::gui::settings::show_settings(); }
                _ => {}
            }
            0
        }
        WM_SIZE => {
            if wparam == SIZE_MINIMIZED {
                ShowWindow(hwnd, SW_HIDE);
                return 0;
            }
            if let Some(ref controller) = GLOBAL_CONTROLLER {
                let mut rect = winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
                GetClientRect(hwnd, &mut rect);
                controller.put_bounds(rect).ok();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// toggle_launch_bar removed

fn switch_interface() {
    unsafe {
        if let Some(ref controller) = GLOBAL_CONTROLLER {
            if let Ok(webview) = controller.get_webview() {
                let mut config = AppConfig::load();
                if config.theme == "Mobile" {
                    config.theme = "Desktop".to_string();
                    println!("Switching to Desktop mode...");
                    webview.navigate("https://i.mi.com/note/v2#/").ok();
                } else {
                    config.theme = "Mobile".to_string();
                    println!("Switching to Mobile mode...");
                    webview.navigate("https://i.mi.com/note/h5#/").ok();
                }
                config.save().ok();
            }
        }
    }
}

// show_settings removed
