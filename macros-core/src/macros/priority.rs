use tracing::warn;

/// Best-effort: raise the calling thread's OS scheduling priority so it's less
/// likely to be preempted for multiple milliseconds under load. Falls back to
/// default priority silently if the platform/permissions don't allow it.
#[cfg(unix)]
pub fn raise_current_thread_priority() {
    let param = unsafe {
        let mut p = std::mem::MaybeUninit::<libc::sched_param>::zeroed().assume_init();
        p.sched_priority = 10;
        p
    };
    let ok = unsafe {
        libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param)
    } == 0;
    if !ok {
        warn!("Failed to raise thread priority (SCHED_FIFO); continuing at default priority");
    }
}

#[cfg(windows)]
pub fn raise_current_thread_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };
    let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) } != 0;
    if !ok {
        warn!("Failed to raise thread priority; continuing at default priority");
    }
}
