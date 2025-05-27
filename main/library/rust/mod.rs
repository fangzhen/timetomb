use core::cell::UnsafeCell;

pub struct SyncOnceUnsafeCell<T> {
    inner: UnsafeCell<Option<T>>,
}

unsafe impl<T: Sync> Sync for SyncOnceUnsafeCell<T> {}

// TODO: It's actually unsafe.
impl<T> SyncOnceUnsafeCell<T> {
    pub const fn new() -> SyncOnceUnsafeCell<T> {
        SyncOnceUnsafeCell {
            inner: UnsafeCell::new(None),
        }
    }
    pub fn get(&self) -> Option<&T> {
        unsafe { &*self.inner.get() }.as_ref()
    }
    pub fn get_unchecked(&self) -> &T {
        self.get().unwrap()
    }
    pub fn set(&self, value: T) -> Result<(), ()> {
        if let Some(_) = self.get() {
            return Err(());
        }

        unsafe { *self.inner.get() = Some(value) };
        Ok(())
    }
}
