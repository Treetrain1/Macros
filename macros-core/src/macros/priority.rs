use tracing::warn;

/// Best-effort: raise the calling thread's OS scheduling priority so the OS is
/// less likely to preempt it for multiple milliseconds under load. This is the
/// only defense against scheduler-induced jitter that `spin_sleep`'s own
/// accuracy tuning can't reach. Silently falls back to default priority if the
/// platform/permissions don't allow it (e.g. missing `CAP_SYS_NICE`/`rtprio`
/// limit on Linux) — never treated as a hard failure.
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
