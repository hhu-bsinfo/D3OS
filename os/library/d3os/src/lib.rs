#![no_std]

pub mod const_string;

/// copy `(dst, src)`, but automatically adjust for differing sizes \
/// returns the actual length which was copied (i.e. `min(dst.len, src.len)`)
#[macro_export] macro_rules! copy_from_slice_sizechecked {
    ($dst:expr, $src:expr) => {
        {
            let l = core::cmp::min($dst.len(), $src.len());
            $dst[..l].copy_from_slice(&$src[..l]);
            l
        }
    };
    ($dst:expr, $src:expr, $amount:expr) => {
        {
            let l = core::cmp::min(core::cmp::min($dst.len(), $src.len()), $amount);
            $dst[..l].copy_from_slice(&$src[..l]);
            l
        }
    };
}

/// allocate a new array on the stack, initialized from the data of the given `src` slice \
/// the `src` slice data is copied sizechecked
#[macro_export] macro_rules! array_from_slice {
    ($src:expr) => {
        array_from_slice!($src, _, _)
    };
    ($src:expr, $type:ty) => {
        array_from_slice!($src, $type, _)
    };
    ($src:expr, $type:ty, $size:expr) => {
        {
            let mut arr = [<$type>::default(); $size];
            copy_from_slice_sizechecked!(arr, $src);
            arr
        }
    };
}