//! Preserve bitmap paste commands which egui-winit's text clipboard path consumes.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{WM_KEYDOWN, WM_NCDESTROY},
    },
};

struct PasteHook {
    requested: Arc<AtomicBool>,
    ctx: eframe::egui::Context,
}

pub fn install(cc: &eframe::CreationContext<'_>, requested: Arc<AtomicBool>) {
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hook = Box::into_raw(Box::new(PasteHook {
        requested,
        ctx: cc.egui_ctx.clone(),
    }));
    // The window and this callback run on the UI thread. The allocation belongs to
    // the subclass until WM_NCDESTROY, including if the App is dropped first.
    unsafe {
        if SetWindowSubclass(
            handle.hwnd.get() as HWND,
            Some(window_proc),
            hook as usize,
            hook as usize,
        ) == 0
        {
            drop(Box::from_raw(hook));
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(window_proc), id);
            drop(Box::from_raw(data as *mut PasteHook));
        }
    } else if message == WM_KEYDOWN
        && wparam == b'V' as usize
        && lparam & (1 << 30) == 0
        && unsafe { GetKeyState(VK_CONTROL as i32) } < 0
    {
        let hook = unsafe { &*(data as *const PasteHook) };
        hook.requested.store(true, Ordering::Relaxed);
        hook.ctx.request_repaint();
    }
    // Forward all events so text editing and the original window lifecycle still work.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
