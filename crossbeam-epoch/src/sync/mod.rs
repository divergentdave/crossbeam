//! Synchronization primitives.

pub mod list;
pub mod queue;

#[cfg(miri)]
/// Update shadow fields used to assist Miri in identifying memory leaks.
/// After updating an atomic pointer, copy the updated value into the leak
/// tracking pointer, then read them back and retry in case they were
/// concurrently updated by another thread. The `Shared` pointer is
/// round-tripped to a reference followed by a raw pointer in order to force
/// int-to-pointer conversion in Miri.
unsafe fn miri_leak_tracking_update_ptr<T>(atomic: &crate::Atomic<T>, cell: &core::cell::UnsafeCell<*const T>) {
    use core::sync::atomic::Ordering::Acquire;
    let guard = crate::unprotected();
    let mut atomic_old = atomic
        .load(Acquire, guard)
        .as_ref()
        .map(|r| r as *const T)
        .unwrap_or_else(core::ptr::null);
    loop {
        let cell_old = *cell.get();
        if atomic_old != cell_old {
            std::ptr::write(cell.get(), atomic_old);
            continue;
        }
        let atomic_new = atomic
            .load(Acquire, guard)
            .as_ref()
            .map(|r| r as *const T)
            .unwrap_or_else(core::ptr::null);
        let cell_new = *cell.get();
        if atomic_old == atomic_new && atomic_new == cell_new {
            break;
        } else {
            atomic_old = atomic_new;
            continue;
        }
    }
}
