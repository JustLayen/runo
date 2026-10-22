use std::sync::atomic::{AtomicBool, Ordering};
static TERMINATE: AtomicBool = AtomicBool::new(false);
extern "C" fn handler(_: libc::c_int) {
    TERMINATE.store(true, Ordering::Relaxed);
}
pub fn install() {
    unsafe {
        libc::signal(libc::SIGHUP, handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handler as *const () as libc::sighandler_t);
    }
}
pub fn terminating() -> bool {
    TERMINATE.load(Ordering::Relaxed)
}
