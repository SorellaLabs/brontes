pub mod base;
pub use base::*;

pub mod dedup;
pub use dedup::*;

pub mod split;
pub use split::*;

pub mod flatten;
pub use flatten::*;

pub mod zip;
pub use zip::*;

pub mod merge;
pub use merge::*;

pub mod map;
pub use map::*;

use crate::tree::NormalizedAction;

pub trait InTupleFnOutVec<V: NormalizedAction> {
    type Out;
}
macro_rules! in_tuple_out_vec {
    ($($out:ident),*) => {
        impl<V: NormalizedAction, $($out,)*> InTupleFnOutVec<V>
            for ($( Box<dyn Fn(V) -> Option<$out>>,)*) {
            type Out = ($( Vec<$out>,)*);
        }
    };
}

in_tuple_out_vec!(T0);
in_tuple_out_vec!(T0, T1);
in_tuple_out_vec!(T0, T1, T2);
in_tuple_out_vec!(T0, T1, T2, T3);
in_tuple_out_vec!(T0, T1, T2, T3, T4);
in_tuple_out_vec!(T0, T1, T2, T3, T4, T5);
in_tuple_out_vec!(T0, T1, T2, T3, T4, T5, T6);
