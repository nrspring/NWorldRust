//! Ids. Every country, person and city has a GUID, and things refer to one another by it, never by
//! name. A person belongs to a country by that country's id, so the name can be anything, and can
//! change, without breaking the link.
//!
//! Each kind of thing has an id type of its own, so an id of a country can't be used where an id of
//! a city is wanted. In the saved files an id is a plain GUID string like
//! `"3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90"`.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::{Builder, Uuid};

/// A new random GUID (a version 4 UUID). The randomness comes from the system, through the keys the
/// standard library gives its hash maps, and is mixed with the time and a counter so that two ids
/// made at once still differ.
fn random_uuid() -> Uuid {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mut bytes = [0u8; 16];
    for half in bytes.chunks_mut(8) {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        hasher.write_u128(now);
        half.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    Builder::from_random_bytes(bytes).into_uuid()
}

/// A random number, from the same source as the ids.
pub fn random_u64() -> u64 {
    let (a, b) = random_uuid().as_u64_pair();
    a.rotate_left(17) ^ b
}

/// A random number from 0 up to (but not including) 1, from the same source as the ids.
pub fn random_unit() -> f32 {
    (random_u64() >> 40) as f32 / (1u64 << 24) as f32
}

/// The id of any thing, whichever kind it is. The typed ids below turn into one and back, for the places
/// that have to hold the id of any kind of thing, like the subject of a news item.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Guid(Uuid);

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Guid({})", self.0.hyphenated())
    }
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// A new id, different from every other.
            pub fn new() -> Self {
                Self(random_uuid())
            }

            /// The id whose GUID is the number `n`, for places that need an id that is always the
            /// same, such as a preview or a test.
            #[allow(dead_code)]
            pub const fn from_u128(n: u128) -> Self {
                Self(Uuid::from_u128(n))
            }
        }

        impl From<$name> for Guid {
            fn from(id: $name) -> Guid {
                Guid(id.0)
            }
        }

        impl From<Guid> for $name {
            fn from(guid: Guid) -> $name {
                $name(guid.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.hyphenated())
            }
        }
    };
}

id_type!(
    /// The id of a country.
    CountryId
);
id_type!(
    /// The id of a person.
    PersonId
);
id_type!(
    /// The id of a city.
    CityId
);
id_type!(
    /// The id of a kind of title, like "Monarch".
    TitleTypeId
);
id_type!(
    /// The id of a bridge.
    BridgeId
);
id_type!(
    /// The id of a note the player wrote in the newsreel.
    NoteId
);

impl CityId {
    /// A number made from the id, for anything that should come out the same for the same city every
    /// time, like where its buildings stand.
    pub fn seed(self) -> u32 {
        let (a, b) = self.0.as_u64_pair();
        let folded = a ^ b;
        (folded ^ (folded >> 32)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_different_every_time() {
        let ids: HashSet<CityId> = (0..20_000).map(|_| CityId::new()).collect();
        assert_eq!(ids.len(), 20_000);
    }

    #[test]
    fn an_id_is_a_version_4_guid() {
        let text = PersonId::new().to_string();
        assert_eq!(text.len(), 36);
        let parts: Vec<&str> = text.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(text.chars().all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{text}");
        // The version is 4, and the variant is the standard one.
        assert!(parts[2].starts_with('4'), "{text}");
        assert!(matches!(parts[3].chars().next(), Some('8' | '9' | 'a' | 'b')), "{text}");
    }

    #[test]
    fn an_id_is_saved_as_its_guid_string() {
        let id = CountryId::from_u128(0x3f2b8c1e_5a47_4d0e_9b6a_7c1d2e8f4a90);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90\"");
        assert_eq!(serde_json::from_str::<CountryId>(&json).unwrap(), id);
        assert_eq!(id.to_string(), "3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90");
    }

    #[test]
    fn a_typed_id_and_the_id_of_anything_turn_into_each_other() {
        let city = CityId::new();
        let any: Guid = city.into();
        assert_eq!(CityId::from(any), city);
        assert_eq!(any.to_string(), city.to_string());
        // The same in a saved file.
        assert_eq!(serde_json::to_string(&any).unwrap(), serde_json::to_string(&city).unwrap());
    }

    #[test]
    fn something_that_is_not_a_guid_is_refused() {
        assert!(serde_json::from_str::<CountryId>("\"Aldoria\"").is_err());
        assert!(serde_json::from_str::<CountryId>("7").is_err());
    }

    #[test]
    fn the_same_city_always_seeds_the_same_and_different_cities_differ() {
        assert_eq!(CityId::from_u128(7).seed(), CityId::from_u128(7).seed());
        let seeds: HashSet<u32> = (0..1_000).map(|n| CityId::from_u128(n).seed()).collect();
        assert_eq!(seeds.len(), 1_000);
    }
}
