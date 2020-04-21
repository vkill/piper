use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::event::Event;

/// An asynchronous lock.
///
/// This type is similar to [`std::sync::Mutex`], except locking is an asynchronous operation.
///
/// # Examples
///
/// ```
/// # smol::run(async {
/// #
/// use piper::Lock;
/// use smol::Task;
/// use std::sync::Arc;
///
/// let l = Arc::new(Lock::new(0));
/// let mut tasks = vec![];
///
/// for _ in 0..10 {
///     let l = l.clone();
///     tasks.push(Task::spawn(async move {
///         *l.lock().await += 1;
///     }));
/// }
///
/// for t in tasks {
///     t.await;
/// }
/// assert_eq!(*l.lock().await, 10);
/// #
/// # })
/// ```
pub struct Lock<T>(Arc<Inner<T>>);

impl<T> Clone for Lock<T> {
    fn clone(&self) -> Lock<T> {
        Lock(self.0.clone())
    }
}

struct Inner<T> {
    locked: AtomicBool,
    lock_ops: Event,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for Lock<T> {}
unsafe impl<T: Send> Sync for Lock<T> {}

impl<T> Lock<T> {
    /// Creates a new async lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use piper::Lock;
    ///
    /// let l = Lock::new(0);
    /// ```
    pub fn new(data: T) -> Lock<T> {
        Lock(Arc::new(Inner {
            locked: AtomicBool::new(false),
            lock_ops: Event::new(),
            data: UnsafeCell::new(data),
        }))
    }

    /// Acquires the lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # smol::block_on(async {
    /// #
    /// use piper::Lock;
    ///
    /// let l = Lock::new(10);
    /// let guard = l.lock().await;
    /// assert_eq!(*guard, 10);
    /// #
    /// # })
    /// ```
    pub async fn lock(&self) -> LockGuard<T> {
        loop {
            // Try acquiring the lock.
            if let Some(guard) = self.try_lock() {
                return guard;
            }

            // Start watching for notifications and try locking again.
            let l = self.0.lock_ops.listen();
            if let Some(guard) = self.try_lock() {
                return guard;
            }
            l.await;
        }
    }

    /// Attempts to acquire the lock.
    ///
    /// If the lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use piper::Lock;
    ///
    /// let l = Lock::new(10);
    /// if let Ok(guard) = l.try_lock() {
    ///     assert_eq!(*guard, 10);
    /// }
    /// # ;
    /// ```
    #[inline]
    pub fn try_lock(&self) -> Option<LockGuard<T>> {
        if !self.0.locked.compare_and_swap(false, true, Ordering::Acquire) {
            Some(LockGuard(self.0.clone()))
        } else {
            None
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Lock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Locked;
        impl fmt::Debug for Locked {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<locked>")
            }
        }

        match self.try_lock() {
            None => f.debug_struct("Lock").field("data", &Locked).finish(),
            Some(guard) => f.debug_struct("Lock").field("data", &&*guard).finish(),
        }
    }
}

impl<T> From<T> for Lock<T> {
    fn from(val: T) -> Lock<T> {
        Lock::new(val)
    }
}

impl<T: Default> Default for Lock<T> {
    fn default() -> Lock<T> {
        Lock::new(Default::default())
    }
}

/// A guard that releases the lock when dropped.
pub struct LockGuard<T>(Arc<Inner<T>>);

unsafe impl<T: Send> Send for LockGuard<T> {}
unsafe impl<T: Sync> Sync for LockGuard<T> {}

impl<T> Drop for LockGuard<T> {
    fn drop(&mut self) {
        self.0.locked.store(false, Ordering::Release);
        self.0.lock_ops.notify_one();
    }
}

impl<T: fmt::Debug> fmt::Debug for LockGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display> fmt::Display for LockGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> Deref for LockGuard<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.0.data.get() }
    }
}

impl<T> DerefMut for LockGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.data.get() }
    }
}
