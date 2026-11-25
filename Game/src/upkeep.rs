//! Paying to keep units when there isn't the money for all of them. Each day the country pays the upkeep of its militia and ships out
//! of what it has and what its cities earn that day (see [`crate::rules`]), and a unit it can't pay for is lost. When that is going to
//! happen the alerts say so, and the player can pick which units to pay for, from a list (see [`crate::upkeep_popup`]): the ships
//! first and then the militia, the biggest first. The country pays for the units picked before the others, so what is left goes to the
//! rest, oldest first, as it does when nothing is picked.

use crate::day::DaySave;
use crate::ids::{CountryId, MilitiaId, ShipId};
use crate::ship::ShipKind;

/// What a country has to pay for upkeep, and what it will have to pay it with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outlook {
    /// What its units cost to keep for a day, all together.
    pub needed: i64,
    /// What it will have when they are paid for: the money it has (none if it is in debt), and the coin for each person in its cities
    /// that it earns first (see the income rule).
    pub available: i64,
}

impl Outlook {
    /// Whether there isn't enough for all of the units.
    pub fn is_short(&self) -> bool {
        self.needed > self.available
    }
}

/// A unit in the list of those to pay for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Ship(ShipId),
    Militia(MilitiaId),
}

/// What the list says of a unit.
#[derive(Debug, PartialEq, Eq)]
pub struct Payable {
    pub unit: Unit,
    pub name: String,
    /// How big it is, which the list is in order of: a ship's size (large before small) or a militia's members. A ship is bigger than any
    /// militia, as ships are listed first.
    pub size: String,
    /// What it costs to keep for a day, in coins.
    pub upkeep: i64,
    /// Whether the player picked it to be paid for.
    pub picked: bool,
}

/// What `country` has to pay for upkeep and what it will have.
pub fn outlook(day: &DaySave, country: CountryId) -> Outlook {
    let ships: i64 = day.ships.iter().filter(|s| s.country == country).map(|s| s.kind.upkeep()).sum();
    let militia: i64 = day.militias.iter().filter(|m| m.country == country).map(|m| m.upkeep()).sum();
    let money = day.country(country).map_or(0, |c| c.money.max(0));
    let income: i64 = day.cities.iter().filter(|c| c.country == country).map(|c| i64::from(c.population)).sum();
    Outlook { needed: ships + militia, available: money.saturating_add(income) }
}

/// The countries that can't afford the upkeep of all their units, in the order they were founded.
pub fn short_countries(day: &DaySave) -> Vec<CountryId> {
    day.countries.iter().filter(|c| outlook(day, c.id).is_short()).map(|c| c.id).collect()
}

/// The units of `country` to pick from: the ships first, the large before the small, and then the militia, the most members first. Units
/// the same size are in the order they were built or raised.
pub fn payables(day: &DaySave, country: CountryId) -> Vec<Payable> {
    let mut ships: Vec<_> = day.ships.iter().filter(|s| s.country == country).collect();
    ships.sort_by_key(|s| std::cmp::Reverse(s.kind == ShipKind::Large));
    let mut militia: Vec<_> = day.militias.iter().filter(|m| m.country == country).collect();
    militia.sort_by_key(|m| std::cmp::Reverse(m.members));
    let ships = ships.into_iter().map(|s| Payable { unit: Unit::Ship(s.id), name: s.title().to_owned(), size: s.kind.label().to_owned(), upkeep: s.kind.upkeep(), picked: s.pay_first });
    let militia = militia.into_iter().map(|m| Payable { unit: Unit::Militia(m.id), name: m.title().to_owned(), size: format!("{} {}", crate::format::thousands(m.members), m.people_label().to_lowercase()), upkeep: m.upkeep(), picked: m.pay_first });
    ships.chain(militia).collect()
}

impl DaySave {
    /// Picks the unit to be paid for (or takes it back). An error says why it can't be done.
    pub fn set_unit_picked(&mut self, unit: Unit, picked: bool) -> Result<(), String> {
        match unit {
            Unit::Ship(id) => self.set_ship_pay_first(id, picked),
            Unit::Militia(id) => self.set_militia_pay_first(id, picked),
        }
    }

    /// Takes back every pick of the units of `country`.
    pub fn clear_picked(&mut self, country: CountryId) {
        self.ships.iter_mut().filter(|s| s.country == country).for_each(|s| s.pay_first = false);
        self.militias.iter_mut().filter(|m| m.country == country).for_each(|m| m.pay_first = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::shipyard::ShipyardKind;
    use macroquad::math::vec2;
    use nworld_map::world::Resource;

    /// Aldoria with two cities of 2,500 people, a small warship (50 a day), a large one (250), and militia of 100 and of 300 members.
    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 1_000_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 100_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 5_000;
        day.build_shipyard_claiming(ShipyardKind::Small, 100.0, 0.0, Some(0x4005)).unwrap();
        let yard = day.shipyards[0].id;
        day.build_ship(yard, ShipKind::Small, vec2(140.0, 0.0)).unwrap();
        day.upgrade_shipyard(yard).unwrap();
        day.shipyards[0].ready_on = None;
        day.build_ship(yard, ShipKind::Large, vec2(180.0, 0.0)).unwrap();
        day.found_city("Highmark", 500.0, 0.0, None).unwrap();
        day.cities[1].population = 5_000;
        let (port, highmark) = (day.cities[0].id, day.cities[1].id);
        day.raise_militia(port, 100, vec2(30.0, 0.0)).unwrap();
        day.raise_militia(highmark, 300, vec2(530.0, 0.0)).unwrap();
        day.cities[0].population = 2_500;
        day.cities[1].population = 2_500;
        day
    }

    fn set_money(day: &mut DaySave, country: CountryId, money: i64) {
        let now = day.country(country).unwrap().money;
        day.add_money(country, money - now).unwrap();
    }

    #[test]
    fn a_country_is_short_when_its_money_and_the_days_income_do_not_cover_its_upkeep() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        assert_eq!(day.ships.len(), 2);
        assert_eq!(day.militias.len(), 2);
        // 50 + 250 for the ships, and 400 members for the militia.
        assert_eq!(outlook(&day, aldoria).needed, 700);
        assert!(short_countries(&day).is_empty(), "it has plenty");

        // Its 5,000 people (in its two cities) earn 5,000 coins before the upkeep is paid, so with no money at all it can still afford it.
        set_money(&mut day, aldoria, 0);
        assert_eq!(outlook(&day, aldoria), Outlook { needed: 700, available: 5_000 });
        assert!(short_countries(&day).is_empty());

        // Without the people it is short.
        day.cities[0].population = 100;
        day.cities[1].population = 0;
        assert_eq!(outlook(&day, aldoria).available, 100);
        assert_eq!(short_countries(&day), [aldoria]);
        set_money(&mut day, aldoria, 600);
        assert!(short_countries(&day).is_empty(), "600 and 100 is 700");
        set_money(&mut day, aldoria, 599);
        assert_eq!(short_countries(&day), [aldoria]);

        // Debt counts as none, and the other country, which has no units, is never short.
        set_money(&mut day, aldoria, -5_000);
        assert_eq!(outlook(&day, aldoria).available, 100);
        assert!(!short_countries(&day).contains(&day.countries[1].id));
    }

    #[test]
    fn the_list_has_the_ships_first_and_then_the_militia_the_biggest_first() {
        let day = day();
        let aldoria = day.countries[0].id;
        let listed = payables(&day, aldoria);
        let sizes: Vec<&str> = listed.iter().map(|p| p.size.as_str()).collect();
        assert_eq!(sizes, ["Large warship", "Small warship", "300 members", "100 members"]);
        assert_eq!(listed.iter().map(|p| p.upkeep).collect::<Vec<_>>(), [250, 50, 300, 100]);
        assert!(matches!(listed[0].unit, Unit::Ship(_)) && matches!(listed[3].unit, Unit::Militia(_)));
        assert!(listed.iter().all(|p| !p.picked));
        assert!(payables(&day, day.countries[1].id).is_empty(), "only the country's own");
    }

    #[test]
    fn picking_a_unit_and_taking_it_back_and_clearing_all_picks() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        let listed = payables(&day, aldoria);
        day.set_unit_picked(listed[1].unit, true).unwrap();
        day.set_unit_picked(listed[2].unit, true).unwrap();
        assert_eq!(payables(&day, aldoria).iter().map(|p| p.picked).collect::<Vec<_>>(), [false, true, true, false]);
        day.set_unit_picked(listed[1].unit, false).unwrap();
        assert_eq!(payables(&day, aldoria).iter().map(|p| p.picked).collect::<Vec<_>>(), [false, false, true, false]);
        day.clear_picked(aldoria);
        assert!(payables(&day, aldoria).iter().all(|p| !p.picked));
        assert!(day.set_unit_picked(Unit::Ship(ShipId::new()), true).is_err());
        assert!(day.set_unit_picked(Unit::Militia(MilitiaId::new()), true).is_err());
    }

    #[test]
    fn the_units_picked_are_the_ones_paid_for_when_the_money_is_short() {
        let mut day = day();
        let aldoria = day.countries[0].id;
        let listed = payables(&day, aldoria);
        let (small, small_militia) = (listed[1].unit, listed[3].unit);
        // 150 coins: with nothing picked the militia (oldest) get what they need first, so 100 members are kept and 50 of the others.
        set_money(&mut day, aldoria, 150);
        let mut unpicked = day.clone();
        unpicked.pay_militia_round(crate::day::Round::All);
        unpicked.pay_ships_round(crate::day::Round::All);
        assert_eq!(unpicked.ships.len(), 0, "no money is left for the ships");

        // Pick the small ship (50) and the small militia (100): both are kept whole, and the rest are lost.
        day.set_unit_picked(small, true).unwrap();
        day.set_unit_picked(small_militia, true).unwrap();
        day.pay_militia_round(crate::day::Round::First);
        day.pay_ships_round(crate::day::Round::First);
        day.pay_militia_round(crate::day::Round::Rest);
        day.pay_ships_round(crate::day::Round::Rest);
        assert_eq!(day.ships.len(), 1);
        assert!(matches!(small, Unit::Ship(id) if day.ships[0].id == id));
        assert_eq!(day.militias.iter().map(|m| m.members).collect::<Vec<_>>(), [100], "the 300 are all gone");
        assert_eq!(day.country(aldoria).unwrap().money, 0);
    }
}
