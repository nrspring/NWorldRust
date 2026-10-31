//! Countries.

use nworld_map::world::Resource;
use serde::{Deserialize, Serialize};

use crate::ids::CountryId;
use crate::rgb::Rgb;
use crate::text;

/// The longest a country's name can be, in characters.
pub const MAX_NAME_CHARS: usize = 32;
/// The money a country has when it is founded.
pub const STARTING_MONEY: i64 = 0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Country {
    /// Tells it apart from every other country, whatever it is called. Everything that belongs to
    /// the country refers to it by this.
    pub id: CountryId,
    pub name: String,
    /// What the country is shown in. Random when it is founded, and the player can change it.
    pub color: Rgb,
    /// The country's money, in whole coins. It can go below zero, which is debt. Each day's file has what the
    /// country had that day, so it can be looked back on.
    #[serde(default)]
    pub money: i64,
    /// The country's stock of each resource. Saved with the day, like the money.
    #[serde(default)]
    pub stock: Stock,
}

/// How much of each resource a country has, in whole units. The names are those the game uses: the map's forest is
/// wood. A stock is never below zero, unlike money, which can be in debt.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Stock {
    pub wood: u64,
    pub stone: u64,
    pub iron: u64,
    pub copper: u64,
    pub sulphur: u64,
    pub oil: u64,
}

impl Stock {
    /// How much there is of `resource`.
    pub fn get(&self, resource: Resource) -> u64 {
        match resource {
            Resource::Forest => self.wood,
            Resource::Stone => self.stone,
            Resource::Iron => self.iron,
            Resource::Copper => self.copper,
            Resource::Sulphur => self.sulphur,
            Resource::Oil => self.oil,
        }
    }

    /// The amount of `resource`, for changing.
    pub fn slot(&mut self, resource: Resource) -> &mut u64 {
        match resource {
            Resource::Forest => &mut self.wood,
            Resource::Stone => &mut self.stone,
            Resource::Iron => &mut self.iron,
            Resource::Copper => &mut self.copper,
            Resource::Sulphur => &mut self.sulphur,
            Resource::Oil => &mut self.oil,
        }
    }
}

/// What the game calls `resource`: "wood", "stone", "iron", "copper", "sulphur" or "oil".
pub fn resource_name(resource: Resource) -> &'static str {
    match resource {
        Resource::Forest => "wood",
        Resource::Stone => "stone",
        Resource::Iron => "iron",
        Resource::Copper => "copper",
        Resource::Sulphur => "sulphur",
        Resource::Oil => "oil",
    }
}

/// A region of the map that a country owns. A region nobody owns has none of these.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct RegionOwner {
    /// The number of the region, as the map knows it.
    pub region: u16,
    pub country: CountryId,
}

/// A country name as typed, tidied up. An error says what is wrong with it.
pub fn clean_name(typed: &str) -> Result<String, String> {
    text::clean(typed, MAX_NAME_CHARS, "a name for your country")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stock_has_each_resource_under_the_names_the_game_uses() {
        let mut stock = Stock::default();
        assert!(Resource::ALL.iter().all(|r| stock.get(*r) == 0), "nothing to begin with");
        for (i, resource) in Resource::ALL.into_iter().enumerate() {
            *stock.slot(resource) = i as u64 + 1;
        }
        assert_eq!(stock, Stock { wood: 1, stone: 2, iron: 3, copper: 4, sulphur: 5, oil: 6 });
        let names: Vec<&str> = Resource::ALL.iter().map(|r| resource_name(*r)).collect();
        assert_eq!(names, ["wood", "stone", "iron", "copper", "sulphur", "oil"], "the map's forest is wood");
    }

    #[test]
    fn names_are_tidied() {
        assert_eq!(clean_name("  Old   Aldoria ").unwrap(), "Old Aldoria");
    }

    #[test]
    fn an_empty_name_asks_for_one() {
        assert_eq!(clean_name("").unwrap_err(), "Enter a name for your country.");
    }

    #[test]
    fn a_name_has_a_length_limit() {
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(clean_name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }
}
