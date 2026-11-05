#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
type Hwnd = *mut c_void;
#[cfg(windows)]
#[repr(C)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> Hwnd;
    fn FindWindowExW(parent: Hwnd, child_after: Hwnd, class_name: *const u16, window_name: *const u16) -> Hwnd;
    fn EnumWindows(callback: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>, parameter: isize) -> i32;
    fn SendMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn SetParent(child: Hwnd, parent: Hwnd) -> Hwnd;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn SetWindowPos(hwnd: Hwnd, insert_after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetParent(hwnd: Hwnd) -> Hwnd;
    fn IsWindow(hwnd: Hwnd) -> i32;
    fn GetClassNameW(hwnd: Hwnd, class_name: *mut u16, max_count: i32) -> i32;
}

#[cfg(windows)]
pub fn has_live_workerw_parent(hwnd: *mut c_void) -> bool {
    unsafe {
        let parent = GetParent(hwnd);
        if parent.is_null() || IsWindow(parent) == 0 { return false; }
        let mut class_name = [0u16; 64];
        let length = GetClassNameW(parent, class_name.as_mut_ptr(), class_name.len() as i32);
        length > 0 && String::from_utf16_lossy(&class_name[..length as usize]) == "WorkerW"
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn SetLastError(error: u32);
    fn GetLastError() -> u32;
}

#[cfg(windows)]
unsafe extern "system" fn find_worker_window(top: Hwnd, parameter: isize) -> i32 {
    const SHELL_VIEW: [u16; 17] = [83, 72, 69, 76, 76, 68, 76, 76, 95, 68, 101, 102, 86, 105, 101, 119, 0];
    const WORKER: [u16; 8] = [87, 111, 114, 107, 101, 114, 87, 0];
    let shell_view = FindWindowExW(top, std::ptr::null_mut(), SHELL_VIEW.as_ptr(), std::ptr::null());
    if !shell_view.is_null() {
        let worker = FindWindowExW(std::ptr::null_mut(), top, WORKER.as_ptr(), std::ptr::null());
        if !worker.is_null() {
            *(parameter as *mut Hwnd) = worker;
            return 0;
        }
    }
    1
}

#[cfg(windows)]
pub fn attach_to_desktop(hwnd: *mut c_void, x: i32, y: i32, width: u32, height: u32) -> anyhow::Result<()> {
    const PROGMAN: [u16; 8] = [80, 114, 111, 103, 109, 97, 110, 0];
    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;
    const WS_CHILD: isize = 0x4000_0000;
    const WS_POPUP: isize = 0x8000_0000u32 as isize;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const HWND_BOTTOM: Hwnd = -2isize as Hwnd;
    unsafe {
        let progman = FindWindowW(PROGMAN.as_ptr(), std::ptr::null());
        if progman.is_null() { anyhow::bail!("Не найдено окно Progman рабочего стола"); }
        // Explorer's undocumented message asks the shell to create the desktop WorkerW layer.
        let _ = SendMessageW(progman, 0x052C, 0xD, 1);
        let mut worker: Hwnd = std::ptr::null_mut();
        let _ = EnumWindows(Some(find_worker_window), &mut worker as *mut Hwnd as isize);
        // Progman is not a safe substitute: parenting to it may put the surface
        // above desktop icons while appearing to succeed. Report failure so the
        // caller can leave the wallpaper unapplied and explain the problem.
        if worker.is_null() {
            anyhow::bail!("Не найден WorkerW рабочего стола. Обои не были применены; перезапустите Проводник Windows или проверьте совместимость этой версии Windows.");
        }
        let parent = worker;

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_POPUP) | WS_CHILD);
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW);
        SetLastError(0);
        let previous_parent = SetParent(hwnd, parent);
        if previous_parent.is_null() && GetLastError() != 0 {
            anyhow::bail!("Не удалось встроить поверхность обоев в WorkerW рабочего стола");
        }

        let mut parent_rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(parent, &mut parent_rect) == 0 { anyhow::bail!("Не удалось получить геометрию рабочего стола"); }
        let child_x = x - parent_rect.left;
        let child_y = y - parent_rect.top;
        if SetWindowPos(hwnd, HWND_BOTTOM, child_x, child_y, width as i32, height as i32, SWP_NOACTIVATE | SWP_SHOWWINDOW) == 0 {
            anyhow::bail!("Не удалось разместить поверхность обоев на рабочем столе");
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn attach_to_desktop(_hwnd: (), _x: i32, _y: i32, _width: u32, _height: u32) -> anyhow::Result<()> {
    anyhow::bail!("Размещение обоев за значками поддерживается только в Windows 10/11");
}

