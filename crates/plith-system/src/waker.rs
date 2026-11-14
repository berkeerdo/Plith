//! Wake-up hook for a consumer that cannot block on a channel (for example a UI thread that
//! waits on its message queue): the service calls it after every published change.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};

type WakeFn = Arc<dyn Fn() + Send + Sync>;

/// Holds the optional wake-up callback.
#[derive(Default)]
pub(crate) struct Waker {
    callback: Mutex<Option<WakeFn>>,
}

impl Waker {
    /// Replaces the callback.
    pub(crate) fn set(&self, callback: Box<dyn Fn() + Send + Sync>) {
        *self.callback.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::from(callback));
    }

    /// Calls the callback, if any, with no lock held.
    pub(crate) fn wake(&self) {
        let callback = self
            .callback
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}

/// Applies `change` to `current`; when it reports a change, sends the new value to every
/// subscriber and then wakes the consumer. Returns whether anything changed. No lock is held
/// while the waker runs.
pub(crate) fn publish_with<T: Clone>(
    current: &Mutex<T>,
    subscribers: &Mutex<Vec<Sender<T>>>,
    waker: &Waker,
    change: impl FnOnce(&mut T) -> bool,
) -> bool {
    let next = {
        let mut guard = current.lock().unwrap_or_else(PoisonError::into_inner);
        if !change(&mut guard) {
            return false;
        }
        guard.clone()
    };
    subscribers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|tx| tx.send(next.clone()).is_ok());
    waker.wake();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;

    fn set_to(value: i32) -> impl FnOnce(&mut i32) -> bool {
        move |current| {
            if *current == value {
                return false;
            }
            *current = value;
            true
        }
    }

    #[test]
    fn waker_fires_once_per_distinct_change() {
        let count = Arc::new(AtomicUsize::new(0));
        let waker = Waker::default();
        let seen = Arc::clone(&count);
        waker.set(Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        let current = Mutex::new(0);
        let (tx, rx) = channel();
        let subscribers = Mutex::new(vec![tx]);

        assert!(publish_with(&current, &subscribers, &waker, set_to(1)));
        assert!(!publish_with(&current, &subscribers, &waker, set_to(1)));
        assert!(publish_with(&current, &subscribers, &waker, set_to(2)));

        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn no_waker_set_is_fine() {
        let current = Mutex::new(0);
        let subscribers = Mutex::new(Vec::new());
        assert!(publish_with(
            &current,
            &subscribers,
            &Waker::default(),
            set_to(5)
        ));
    }

    #[test]
    fn waker_runs_outside_the_locks() {
        let current = Arc::new(Mutex::new(0));
        let subscribers: Arc<Mutex<Vec<Sender<i32>>>> = Arc::new(Mutex::new(Vec::new()));
        let waker = Waker::default();
        let (cur, subs) = (Arc::clone(&current), Arc::clone(&subscribers));
        // Would deadlock if either lock were still held while the waker runs.
        waker.set(Box::new(move || {
            drop(cur.try_lock().expect("state lock free"));
            drop(subs.try_lock().expect("subscribers lock free"));
        }));
        assert!(publish_with(&current, &subscribers, &waker, set_to(3)));
    }
}
