use std::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_queue::ArrayQueue;
use crossbeam_utils::thread::scope;
use rand::{thread_rng, Rng};

#[test]
#[cfg_attr(miri, ignore = "UB: incorrect layout on deallocation")]
fn smoke() {
    let q = ArrayQueue::new(1);

    q.push(7).unwrap();
    assert_eq!(q.pop(), Ok(7));

    q.push(8).unwrap();
    assert_eq!(q.pop(), Ok(8));
    assert!(q.pop().is_err());
}

#[test]
#[cfg_attr(miri, ignore = "UB: incorrect layout on deallocation")]
fn capacity() {
    for i in 1..10 {
        let q = ArrayQueue::<i32>::new(i);
        assert_eq!(q.capacity(), i);
    }
}

#[test]
#[should_panic(expected = "capacity must be non-zero")]
fn zero_capacity() {
    let _ = ArrayQueue::<i32>::new(0);
}

#[test]
#[cfg_attr(miri, ignore = "UB: incorrect layout on deallocation")]
fn len_empty_full() {
    let q = ArrayQueue::new(2);

    assert_eq!(q.len(), 0);
    assert_eq!(q.is_empty(), true);
    assert_eq!(q.is_full(), false);

    q.push(()).unwrap();

    assert_eq!(q.len(), 1);
    assert_eq!(q.is_empty(), false);
    assert_eq!(q.is_full(), false);

    q.push(()).unwrap();

    assert_eq!(q.len(), 2);
    assert_eq!(q.is_empty(), false);
    assert_eq!(q.is_full(), true);

    q.pop().unwrap();

    assert_eq!(q.len(), 1);
    assert_eq!(q.is_empty(), false);
    assert_eq!(q.is_full(), false);
}

#[test]
#[cfg_attr(miri, ignore = "deadlocks, second thread never runs")]
fn len() {
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    #[cfg(miri)]
    const COUNT: usize = 2_500;
    #[cfg(not(miri))]
    const CAP: usize = 1000;
    #[cfg(miri)]
    const CAP: usize = 100;

    let q = ArrayQueue::new(CAP);
    assert_eq!(q.len(), 0);

    for _ in 0..CAP / 10 {
        for i in 0..50 {
            q.push(i).unwrap();
            assert_eq!(q.len(), i + 1);
        }

        for i in 0..50 {
            q.pop().unwrap();
            assert_eq!(q.len(), 50 - i - 1);
        }
    }
    assert_eq!(q.len(), 0);

    for i in 0..CAP {
        q.push(i).unwrap();
        assert_eq!(q.len(), i + 1);
    }

    for _ in 0..CAP {
        q.pop().unwrap();
    }
    assert_eq!(q.len(), 0);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                loop {
                    if let Ok(x) = q.pop() {
                        assert_eq!(x, i);
                        break;
                    }
                }
                let len = q.len();
                assert!(len <= CAP);
            }
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                while q.push(i).is_err() {}
                let len = q.len();
                assert!(len <= CAP);
            }
        });
    })
    .unwrap();
    assert_eq!(q.len(), 0);
}

#[test]
#[cfg_attr(miri, ignore = "deadlocks, only the first thread runs")]
fn spsc() {
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;
    #[cfg(miri)]
    const COUNT: usize = 100;

    let q = ArrayQueue::new(3);

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                loop {
                    if let Ok(x) = q.pop() {
                        assert_eq!(x, i);
                        break;
                    }
                }
            }
            assert!(q.pop().is_err());
        });

        scope.spawn(|_| {
            for i in 0..COUNT {
                while q.push(i).is_err() {}
            }
        });
    })
    .unwrap();
}

#[test]
#[cfg_attr(miri, ignore = "deadlocks, only the first thread runs")]
fn mpmc() {
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    #[cfg(miri)]
    const COUNT: usize = 25;
    const THREADS: usize = 4;

    let q = ArrayQueue::<usize>::new(3);
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    let n = loop {
                        if let Ok(x) = q.pop() {
                            break x;
                        }
                    };
                    v[n].fetch_add(1, Ordering::SeqCst);
                }
            });
        }
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for i in 0..COUNT {
                    while q.push(i).is_err() {}
                }
            });
        }
    })
    .unwrap();

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}

#[test]
#[cfg_attr(miri, ignore = "deadlocks, neither for loop body that pushes to the queue ever runs")]
fn drops() {
    #[cfg(not(miri))]
    const RUNS: usize = 100;
    #[cfg(miri)]
    const RUNS: usize = 10;
    #[cfg(not(miri))]
    const MAX_STEPS: usize = 10_000;
    #[cfg(miri)]
    const MAX_STEPS: usize = 100;

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq)]
    struct DropCounter;

    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut rng = thread_rng();

    for _ in 0..RUNS {
        let steps = rng.gen_range(0, MAX_STEPS);
        let additional = rng.gen_range(0, 50);

        DROPS.store(0, Ordering::SeqCst);
        let q = ArrayQueue::new(50);

        scope(|scope| {
            scope.spawn(|_| {
                for _ in 0..steps {
                    while q.pop().is_err() {}
                }
            });

            scope.spawn(|_| {
                for _ in 0..steps {
                    while q.push(DropCounter).is_err() {
                        DROPS.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            });
        })
        .unwrap();

        for _ in 0..additional {
            q.push(DropCounter).unwrap();
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
        drop(q);
        assert_eq!(DROPS.load(Ordering::SeqCst), steps + additional);
    }
}

#[test]
fn linearizable() {
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    #[cfg(miri)]
    const COUNT: usize = 25;
    const THREADS: usize = 4;

    let q = ArrayQueue::new(THREADS);

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    while q.push(0).is_err() {}
                    q.pop().unwrap();
                }
            });
        }
    })
    .unwrap();
}
