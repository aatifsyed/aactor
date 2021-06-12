use std::marker::PhantomPinned;

struct Intellibuf<const N: usize> {
    buffer: [u8; N],
    current_slice_end: usize,

    // #![feature(negative_impls)]
    // impl<const N: usize> !Unpin for Intellibuf<N> {}
    _immovable: PhantomPinned,
}

impl<'a, const N: usize> Intellibuf<N> {
    fn current_slice(&'a self) -> &'a [u8] {
        &self.buffer[..self.current_slice_end]
    }
}
