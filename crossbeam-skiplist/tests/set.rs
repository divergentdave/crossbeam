use crossbeam_skiplist::SkipSet;

#[test]
#[cfg_attr(miri, ignore = "UB: trying to reborrow for SharedReadWrite, but parent tag does not have an appropriate item in the borrow stack")]
fn smoke() {
    let m = SkipSet::new();
    m.insert(1);
    m.insert(5);
    m.insert(7);
}
