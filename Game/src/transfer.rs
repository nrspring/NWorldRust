//! Transfers: one country giving money or a resource to another. The sender must have what it gives (a transfer can't put
//! it in debt), and the transfer is reported in the day's newsreel.

use nworld_map::world::Resource;
use serde::{Deserialize, Serialize};

use crate::country::resource_name;
use crate::day::DaySave;
use crate::ids::CountryId;
use crate::newsreel::{Event, News};

/// Something a country has that it can give away: its money, or one of its resources.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    Coins,
    Wood,
    Stone,
    Iron,
    Copper,
    Sulphur,
    Oil,
}

impl Asset {
    /// Money first, then each resource in the order of [`Resource::ALL`].
    pub const ALL: [Asset; 7] = [Asset::Coins, Asset::Wood, Asset::Stone, Asset::Iron, Asset::Copper, Asset::Sulphur, Asset::Oil];

    /// The asset that is the stock of `resource`.
    pub fn of(resource: Resource) -> Self {
        match resource {
            Resource::Forest => Asset::Wood,
            Resource::Stone => Asset::Stone,
            Resource::Iron => Asset::Iron,
            Resource::Copper => Asset::Copper,
            Resource::Sulphur => Asset::Sulphur,
            Resource::Oil => Asset::Oil,
        }
    }

    /// The resource it is the stock of, or `None` for money.
    pub fn resource(self) -> Option<Resource> {
        Resource::ALL.into_iter().find(|r| Asset::of(*r) == self)
    }

    /// What it is called in the game, in lower case: "coins", "wood" and so on.
    pub fn name(self) -> &'static str {
        match self.resource() {
            Some(resource) => resource_name(resource),
            None => "coins",
        }
    }

    /// The same starting with a capital.
    pub fn label(self) -> String {
        let name = self.name();
        format!("{}{}", name[..1].to_uppercase(), &name[1..])
    }

    /// The asset called `typed`, whatever the capitals and the spaces around it. "Money" and "coin" will do for coins.
    pub fn named(typed: &str) -> Option<Self> {
        let typed = typed.trim().to_lowercase();
        match typed.as_str() {
            "money" | "coin" => Some(Asset::Coins),
            _ => Asset::ALL.into_iter().find(|a| a.name() == typed),
        }
    }
}

/// `amount` of `asset` going from the country `from` to the country `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transfer {
    pub from: CountryId,
    pub to: CountryId,
    pub asset: Asset,
    pub amount: u64,
}

impl Transfer {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        day.transfer(*self)
    }
}

impl DaySave {
    /// Moves what `transfer` says from the sender to the receiver, and reports it in the newsreel. If it can't be done,
    /// nothing changes and the error says why.
    pub fn transfer(&mut self, transfer: Transfer) -> Result<(), String> {
        let Transfer { from, to, asset, amount } = transfer;
        let sender = self.country(from).ok_or("There is no such country to send from.")?;
        let receiver = self.country(to).ok_or("There is no such country to send to.")?;
        if from == to {
            return Err("A country can't send to itself.".to_owned());
        }
        if amount == 0 {
            return Err("Enter an amount of more than nothing.".to_owned());
        }
        let amount_signed = i64::try_from(amount).map_err(|_| "That is more than there can be.".to_owned())?;
        let has = match asset.resource() {
            Some(resource) => sender.stock.get(resource),
            None => u64::try_from(sender.money).unwrap_or(0),
        };
        if has < amount {
            let (name, receiver) = (asset.name(), receiver.name.clone());
            return Err(format!("{} doesn't have that many {name} to send to {receiver}.", sender.name));
        }

        // The receiver first, as it is the one that can run out of room, and then the sender, which has been checked.
        match asset.resource() {
            Some(resource) => {
                self.add_resource(to, resource, amount_signed)?;
                self.add_resource(from, resource, -amount_signed)?;
            }
            None => {
                self.add_money(to, amount_signed)?;
                self.add_money(from, -amount_signed)?;
            }
        }
        self.newsreel.push(News::about(from, Event::Transfer { to, asset, amount }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> (DaySave, CountryId, CountryId) {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.add_money(aldoria, 1_000).unwrap();
        day.add_resource(aldoria, Resource::Forest, 50).unwrap();
        (day, aldoria, brevon)
    }

    fn send(day: &mut DaySave, from: CountryId, to: CountryId, asset: Asset, amount: u64) -> Result<(), String> {
        day.transfer(Transfer { from, to, asset, amount })
    }

    #[test]
    fn every_asset_has_its_own_name_and_the_names_find_them_again() {
        let names: Vec<&str> = Asset::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(names, ["coins", "wood", "stone", "iron", "copper", "sulphur", "oil"]);
        for asset in Asset::ALL {
            assert_eq!(Asset::named(&asset.label()), Some(asset));
        }
        assert_eq!(Asset::named("  MONEY "), Some(Asset::Coins));
        assert_eq!(Asset::named("gold"), None);
        assert_eq!(Asset::Coins.resource(), None);
        assert_eq!(Asset::Wood.resource(), Some(Resource::Forest));
    }

    #[test]
    fn money_moves_from_one_country_to_the_other() {
        let (mut day, aldoria, brevon) = day();
        send(&mut day, aldoria, brevon, Asset::Coins, 400).unwrap();
        assert_eq!(day.country(aldoria).unwrap().money, 600);
        assert_eq!(day.country(brevon).unwrap().money, 400);
    }

    #[test]
    fn a_resource_moves_from_one_country_to_the_other() {
        let (mut day, aldoria, brevon) = day();
        send(&mut day, aldoria, brevon, Asset::Wood, 20).unwrap();
        assert_eq!(day.country(aldoria).unwrap().stock.wood, 30);
        assert_eq!(day.country(brevon).unwrap().stock.wood, 20);
        assert_eq!(day.country(brevon).unwrap().money, 0, "nothing else moved");
    }

    #[test]
    fn a_transfer_is_reported_in_the_newsreel_with_the_sender_as_its_subject() {
        let (mut day, aldoria, brevon) = day();
        let before = day.newsreel.len();
        send(&mut day, aldoria, brevon, Asset::Coins, 1).unwrap();
        send(&mut day, aldoria, brevon, Asset::Oil, 0).unwrap_err();
        send(&mut day, aldoria, brevon, Asset::Wood, 12).unwrap();
        assert_eq!(day.newsreel.len(), before + 2, "a refused transfer is not news");
        let news = &day.newsreel[before..];
        assert_eq!(news[0], News::about(aldoria, Event::Transfer { to: brevon, asset: Asset::Coins, amount: 1 }));
        assert_eq!(news[0].headline(&day), "Aldoria sends 1 coin to Brevon");
        assert_eq!(news[1].headline(&day), "Aldoria sends 12 wood to Brevon");
    }

    #[test]
    fn a_transfer_that_cannot_be_done_says_why_and_changes_nothing() {
        let (mut day, aldoria, brevon) = day();
        let before = day.clone();
        assert!(send(&mut day, aldoria, brevon, Asset::Coins, 1_001).unwrap_err().contains("doesn't have that many coins"));
        assert!(send(&mut day, aldoria, brevon, Asset::Iron, 1).unwrap_err().contains("iron"));
        assert!(send(&mut day, brevon, aldoria, Asset::Coins, 1).is_err(), "no money, and no debt");
        assert!(send(&mut day, aldoria, aldoria, Asset::Coins, 1).unwrap_err().contains("itself"));
        assert!(send(&mut day, aldoria, brevon, Asset::Coins, 0).unwrap_err().contains("more than nothing"));
        assert!(send(&mut day, aldoria, brevon, Asset::Coins, u64::MAX).is_err());
        assert!(send(&mut day, aldoria, CountryId::new(), Asset::Coins, 1).unwrap_err().contains("send to"));
        assert!(send(&mut day, CountryId::new(), brevon, Asset::Coins, 1).unwrap_err().contains("send from"));
        assert_eq!(day, before);
    }

    #[test]
    fn a_country_in_debt_has_no_money_to_send() {
        let (mut day, aldoria, brevon) = day();
        day.add_money(aldoria, -1_500).unwrap();
        assert!(send(&mut day, aldoria, brevon, Asset::Coins, 1).is_err());
        assert_eq!(day.country(aldoria).unwrap().money, -500);
    }

    #[test]
    fn a_receiver_with_no_room_leaves_the_sender_as_it_was() {
        let (mut day, aldoria, brevon) = day();
        day.add_money(brevon, i64::MAX).unwrap();
        let before = day.clone();
        assert!(send(&mut day, aldoria, brevon, Asset::Coins, 1).unwrap_err().contains("more money"));
        assert_eq!(day, before);
    }

    #[test]
    fn a_transfer_is_saved_in_the_news_with_what_was_sent() {
        let (mut day, aldoria, brevon) = day();
        send(&mut day, aldoria, brevon, Asset::Sulphur, 0).unwrap_err();
        day.add_resource(aldoria, Resource::Sulphur, 9).unwrap();
        send(&mut day, aldoria, brevon, Asset::Sulphur, 9).unwrap();
        let json = serde_json::to_string(day.newsreel.last().unwrap()).unwrap();
        assert!(json.contains(r#""type":"transfer""#) && json.contains(r#""asset":"sulphur""#) && json.contains(r#""amount":9"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *day.newsreel.last().unwrap());
    }
}
