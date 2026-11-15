//! Fixed-chunk parallel loops. Chunk boundaries depend only on the data size, never on the number of
//! threads, so every result (including ordered reductions done by the caller) is identical for any
//! thread count. Without the `parallel` feature the same loops run sequentially.

/// Run `f(scratch, chunk_index, chunk, info)` over equal chunks of `data` and the matching element of
/// `infos` (one per chunk). `init` makes the per-thread scratch state.
#[cfg(feature = "parallel")]
pub(crate) fn for_each_chunk<T, U, S>(
    data: &mut [T],
    chunk: usize,
    infos: &mut [U],
    init: impl Fn() -> S + Sync + Send,
    f: impl Fn(&mut S, usize, &mut [T], &mut U) + Sync + Send,
) where
    T: Send,
    U: Send,
{
    use rayon::prelude::*;
    data.par_chunks_mut(chunk)
        .zip(infos.par_iter_mut())
        .enumerate()
        .for_each_init(init, |s, (i, (c, u))| f(s, i, c, u));
}

#[cfg(not(feature = "parallel"))]
pub(crate) fn for_each_chunk<T, U, S>(
    data: &mut [T],
    chunk: usize,
    infos: &mut [U],
    init: impl Fn() -> S + Sync + Send,
    f: impl Fn(&mut S, usize, &mut [T], &mut U) + Sync + Send,
) {
    let mut s = init();
    for (i, (c, u)) in data.chunks_mut(chunk).zip(infos.iter_mut()).enumerate() {
        f(&mut s, i, c, u);
    }
}

/// Run `f(chunk_index, chunk)` over equal chunks of `data`.
#[cfg(feature = "parallel")]
pub(crate) fn for_each_plain<T: Send>(
    data: &mut [T],
    chunk: usize,
    f: impl Fn(usize, &mut [T]) + Sync + Send,
) {
    use rayon::prelude::*;
    data.par_chunks_mut(chunk)
        .enumerate()
        .for_each(|(i, c)| f(i, c));
}

#[cfg(not(feature = "parallel"))]
pub(crate) fn for_each_plain<T: Send>(
    data: &mut [T],
    chunk: usize,
    f: impl Fn(usize, &mut [T]) + Sync + Send,
) {
    for (i, c) in data.chunks_mut(chunk).enumerate() {
        f(i, c);
    }
}

/// `dst.copy_from_slice(src)` in fixed chunks, in parallel.
#[cfg(feature = "parallel")]
pub(crate) fn copy(dst: &mut [f64], src: &[f64]) {
    use rayon::prelude::*;
    dst.par_chunks_mut(COPY_CHUNK)
        .zip(src.par_chunks(COPY_CHUNK))
        .for_each(|(d, s)| d.copy_from_slice(s));
}

#[cfg(not(feature = "parallel"))]
pub(crate) fn copy(dst: &mut [f64], src: &[f64]) {
    dst.copy_from_slice(src);
}

/// Elements per chunk of a parallel copy.
#[cfg(feature = "parallel")]
const COPY_CHUNK: usize = 1 << 16;
