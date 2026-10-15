//! Platform abstraction — `std` on hosted builds, `alloc` on bare metal.

#[cfg(feature = "std")]
pub use std::{
    collections::{BTreeMap, HashMap, HashSet},
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "baremetal")]
pub use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "baremetal")]
pub use hashbrown::{HashMap, HashSet};
