//! Stable identifiers. IDs are never reused within a campaign, so saves and network messages can
//! refer to things unambiguously.

use serde::{Deserialize, Serialize};

macro_rules! id {
    ($name:ident, $t:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub $t);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}#{}", stringify!($name), self.0)
            }
        }
    };
}

id!(RoomId, u32);
id!(EdgeId, u32);
id!(EntityId, u64);
id!(ObjectId, u64);
