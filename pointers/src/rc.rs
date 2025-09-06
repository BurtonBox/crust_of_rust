use crate::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

struct RcInner<T> {
    value: T,
    refcount: Cell<usize>,
}

pub struct Rc<T> {
    inner: NonNull<RcInner<T>>,
    _marker: PhantomData<RcInner<T>>,
}

impl<T> Rc<T> {
    pub fn new(v: T) -> Self {
        //     let inner = Box::new(RcInner {
        //         value: v,
        //         refcount: Cell::new(1),
        //     });
        //
        //     Rc {
        //         // SAFETY: Box does not give us a null pointer.
        //         inner: unsafe { NonNull::new_unchecked(Box::into_raw(inner)) },
        //         _marker: PhantomData,
        //     }
        let boxed = Box::new(RcInner {
            value: v,
            refcount: Cell::new(1),
        });

        let inner = NonNull::from(Box::leak(boxed));

        Rc {
            inner,
            _marker: PhantomData,
        }
    }

    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        // SAFETY: we have &mut self; if refcount==1, no other Rc exists, so &mut T is fine.
        unsafe {
            let ptr = this.inner.as_ptr();
            if (*ptr).refcount.get() == 1 {
                Some(&mut (*ptr).value)
            } else {
                None
            }
        }
    }
}

impl<T> std::ops::Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        //// SAFETY: self.inner is a Box that is only deallocated when the last Rc goes away.
        //// we have an Rc, therefore the Box has not been deallocated, so deref is fine.
        //&unsafe { self.inner.as_ref() }.value

        // SAFETY: inner points to a valid RcInner until the last Rc drops it.
        unsafe { &self.inner.as_ref().value }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        // SAFETY: reading and writing the Cell is fine (single-threaded).
        let inner = unsafe { self.inner.as_ref() };
        let c = inner.refcount.get();
        debug_assert!(c != usize::MAX, "Rc refcount overflow");
        inner.refcount.set(c + 1);
        Rc {
            inner: self.inner,
            _marker: PhantomData,
        }
    }
}

// TODO: #[may_dangle] (advanced; lets Drop run even if T's destructor could observe a partially dropped value)
impl<T> Drop for Rc<T> {
    // fn drop(&mut self) {
    //     let inner = unsafe { self.inner.as_ref() };
    //     let c = inner.refcount.get();
    //     if c == 1 {
    //         drop(inner);
    //         // SAFETY: we are the _only_ Rc left, and we are being dropped.
    //         // therefore, after us, there will be no Rc's, and no references to T.
    //         let _ = unsafe { Box::from_raw(self.inner.as_ptr()) };
    //     } else {
    //         // there are other Rcs, so don't drop the Box!
    //         inner.refcount.set(c - 1);
    //     }
    // }
    fn drop(&mut self) {
        unsafe {
            let ptr = self.inner.as_ptr();
            // Read the count without keeping an & alive across the free.
            let c = (*ptr).refcount.get();
            if c == 1 {
                // Drop the allocation (drops T then frees the box).
                drop(Box::from_raw(ptr));
            } else {
                (*ptr).refcount.set(c - 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rc;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    /// A value that bumps a shared counter when dropped.
    #[derive(Debug)]
    struct DropSpy {
        drops: Arc<AtomicUsize>,
        _id: &'static str,
    }

    impl Drop for DropSpy {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn deref_reads_value() {
        let x = Rc::new(42);
        assert_eq!(*x, 42);
    }

    #[test]
    fn clones_share_same_inner_address() {
        let a = Rc::new(5i32);
        let b = a.clone();

        let pa: *const i32 = &*a;
        let pb: *const i32 = &*b;
        assert_eq!(pa, pb, "both Rcs must point to the same inner value");
    }

    #[test]
    fn drop_happens_once_on_last_owner() {
        let drops = Arc::new(AtomicUsize::new(0));
        let a = Rc::new(DropSpy { drops: drops.clone(), _id: "one" });
        let b = a.clone();

        // Dropping one clone should not drop the inner value.
        drop(b);
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        // Dropping the last owner should drop the inner exactly once.
        drop(a);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn many_clones_still_drop_inner_exactly_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let base = Rc::new(DropSpy { drops: drops.clone(), _id: "many" });

        // Make a bunch of clones.
        let mut v = Vec::with_capacity(1024);
        for _ in 0..1024 {
            v.push(base.clone());
        }

        // Drop the original; there are still many clones.
        drop(base);
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        // Drop all the clones; the inner should drop exactly once at the very end.
        while let Some(_) = v.pop() {}
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn forget_leaks_and_skips_drop() {
        let drops = Arc::new(AtomicUsize::new(0));
        {
            let r = Rc::new(DropSpy { drops: drops.clone(), _id: "leak" });
            std::mem::forget(r); // Intentionally leak
        }
        // Because we leaked, drop never ran.
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn get_mut_allows_unique_mutation() {
        let mut r = Rc::new(10);
        // Unique -> Some(&mut T)
        if let Some(x) = Rc::get_mut(&mut r) {
            *x = 99;
        } else {
            panic!("expected unique access");
        }
        assert_eq!(*r, 99);
    
        // After cloning, not unique -> None
        let mut r2 = r.clone();
        assert!(Rc::get_mut(&mut r2).is_none());
    }
}
