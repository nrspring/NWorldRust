//! Building ships at a shipyard, changing the country one belongs to, moving, renaming and scrapping them, and paying to keep them.

use macroquad::math::{vec2, Vec2};

use super::DaySave;
use crate::ids::{CountryId, ShipId, ShipyardId};
use crate::movement::DAILY_POINTS;
use crate::newsreel::{Event, News};
use crate::ship::{self, days_text, Ship, ShipKind};

impl DaySave {
    /// The ship with this id.
    pub fn ship(&self, id: ShipId) -> Option<&Ship> {
        self.ships.iter().find(|s| s.id == id)
    }

    /// Where every ship floats, which a new one can't be put on top of.
    pub fn berths_taken(&self) -> Vec<Vec2> {
        self.ships.iter().map(|s| vec2(s.x, s.z)).collect()
    }

    /// Why the shipyard with this id can't build a ship of `kind` now, or `None` if it can: the shipyard has to be there, belong to
    /// the country being played, which pays for the ship, be big enough for the ship, and not be resting after its last one; and the
    /// country has to be able to pay, in coins first, and then in each of the materials.
    pub fn ship_problem(&self, yard: ShipyardId, kind: ShipKind) -> Option<String> {
        let Some(found) = self.shipyard(yard) else { return Some("There is no such shipyard.".to_owned()) };
        let Some(player) = self.player_country() else { return Some(format!("There is no country yet to own {}.", kind.a_noun())) };
        if found.country != player.id {
            return Some(format!("This shipyard belongs to {}, and only {} can build ships there.", self.country_name(found.country), self.country_name(found.country)));
        }
        if !found.kind.can_build(kind) {
            return Some(format!("{} can only build small warships: upgrade it to a large shipyard for large ones.", found.kind.a_noun_capital()));
        }
        let left = found.rest_left(self.date);
        if left > 0 {
            return Some(format!("{} is resting after its last ship, and can build another in {}.", found.kind.a_noun_capital(), days_text(left)));
        }
        self.payment_problem(&kind.a_noun(), kind.cost(), kind.materials())
    }

    /// Builds a ship of `kind` at the shipyard with this id, floating at `berth` (see [`ship::berth_near`]), for the country being
    /// played, which pays for it. The shipyard then has to rest for as many days as its size and the ship's say (see
    /// [`ShipyardKind::break_days`](crate::shipyard::ShipyardKind::break_days)). This is reported in the newsreel. An error says why
    /// it can't be done, and nothing changes.
    pub fn build_ship(&mut self, yard: ShipyardId, kind: ShipKind, berth: Vec2) -> Result<(), String> {
        if let Some(problem) = self.ship_problem(yard, kind) {
            return Err(problem);
        }
        if !berth.x.is_finite() || !berth.y.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let found = *self.shipyard(yard).ok_or("There is no such shipyard.")?;
        let rest = found.kind.break_days(kind).ok_or("That shipyard can't build that ship.")?;
        let country = found.country;
        self.add_money(country, -kind.cost())?;
        for (resource, needed) in kind.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        let id = ShipId::new();
        let name = ship::default_name(kind, id);
        self.ships.push(Ship { id, name: name.clone(), kind, country, built: self.date, x: berth.x, z: berth.y, points: DAILY_POINTS });
        let ready = self.date.plus_days(rest);
        if let Some(shipyard) = self.shipyards.iter_mut().find(|s| s.id == yard) {
            shipyard.ready_on = Some(ready);
        }
        self.newsreel.push(News::about(id, Event::ShipBuilt { name, kind, country, region: found.region, cost: kind.full_cost() }));
        Ok(())
    }

    /// Renames the ship with this id to `typed`, which is reported in the newsreel unless it was called that already. An error says
    /// why it can't be done.
    pub fn rename_ship(&mut self, id: ShipId, typed: &str) -> Result<(), String> {
        let name = ship::clean_name(typed)?;
        let ship = self.ships.iter_mut().find(|s| s.id == id).ok_or("There is no such ship.")?;
        if ship.name != name {
            let from = std::mem::replace(&mut ship.name, name.clone());
            self.newsreel.push(News::about(id, Event::ShipRenamed { from, to: name }));
        }
        Ok(())
    }

    /// Moves the ship with this id to `to`, which costs it `cost` of its movement points: that, and whether the water between is fit
    /// to sail, is for the caller to say (see [`crate::movement::reach_by_water`]). `region` is the number of the map region it gets
    /// to, if the map has them. This is reported in the newsreel. An error says why it can't be done.
    pub fn move_ship(&mut self, id: ShipId, to: Vec2, cost: u32, region: Option<u16>) -> Result<(), String> {
        if !to.x.is_finite() || !to.y.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let ship = self.ships.iter_mut().find(|s| s.id == id).ok_or("There is no such ship.")?;
        if cost > ship.points {
            return Err(format!("{} has only {} movement points left, and that takes {cost}.", ship.title(), ship.points));
        }
        ship.points -= cost;
        (ship.x, ship.z) = (to.x, to.y);
        let name = ship.title().to_owned();
        self.newsreel.push(News::about(id, Event::ShipMoved { name, points: cost, region }));
        Ok(())
    }

    /// Scraps the ship with this id: it is gone, and what it cost to build is not given back. This is reported in the newsreel. An
    /// error says why it can't be done.
    pub fn scrap_ship(&mut self, id: ShipId) -> Result<(), String> {
        let index = self.ships.iter().position(|s| s.id == id).ok_or("There is no such ship.")?;
        let ship = self.ships.remove(index);
        self.newsreel.push(News::about(id, Event::ShipScrapped { name: ship.title().to_owned() }));
        Ok(())
    }

    /// Gives every ship all its movement points again, for a new day.
    pub fn rest_ships(&mut self) {
        for ship in &mut self.ships {
            ship.points = DAILY_POINTS;
        }
    }

    /// Makes the ship with this id belong to `country`, which pays to keep it from now on. This is reported in the newsreel unless it
    /// belonged to it already. An error says why it can't be done.
    pub fn set_ship_country(&mut self, id: ShipId, country: CountryId) -> Result<(), String> {
        self.country(country).ok_or("There is no such country.")?;
        let ship = self.ships.iter_mut().find(|s| s.id == id).ok_or("There is no such ship.")?;
        let from = std::mem::replace(&mut ship.country, country);
        if from != country {
            self.newsreel.push(News::about(id, Event::ShipTransferred { from, to: country }));
        }
        Ok(())
    }

    /// Each country pays to keep its ships, the oldest first: 50 coins a day for a small one and 250 for a large one. A ship it can't
    /// pay for is abandoned by its crew, and is gone, so a country with no money keeps no ships, and one with money for some keeps
    /// those. The payment, and each ship that was lost, is in the newsreel.
    pub fn pay_ships(&mut self) {
        for index in 0..self.countries.len() {
            let country = self.countries[index].id;
            let mut affordable = self.countries[index].money.max(0);
            let (mut paid, mut abandoned) = (0i64, Vec::new());
            self.ships.retain(|ship| {
                if ship.country != country {
                    return true;
                }
                let upkeep = ship.kind.upkeep();
                if upkeep <= affordable {
                    affordable -= upkeep;
                    paid += upkeep;
                    true
                } else {
                    abandoned.push(News::about(ship.id, Event::ShipDeserted { name: ship.title().to_owned(), country }));
                    false
                }
            });
            if paid > 0 {
                self.countries[index].money -= paid;
                self.newsreel.push(News::about(country, Event::ShipPaid { amount: paid as u64 }));
            }
            self.newsreel.extend(abandoned);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;
    use crate::shipyard::ShipyardKind;
    use nworld_map::world::Resource;

    const BERTH: Vec2 = Vec2::new(40.0, -10.0);

    /// A day with Aldoria, which has 100,000 coins and 5,000 of every resource, and a small shipyard of its own, and Brevon.
    fn day_with_a_yard() -> DaySave {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 100_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(aldoria, resource, 5_000).unwrap();
        }
        day.build_shipyard_claiming(ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        day
    }

    fn large_yard(day: &mut DaySave) -> ShipyardId {
        let id = day.shipyards[0].id;
        day.upgrade_shipyard(id).unwrap();
        id
    }

    #[test]
    fn a_ship_costs_its_coins_and_materials_and_floats_where_it_is_put_with_all_its_movement_points() {
        let mut day = day_with_a_yard();
        let (aldoria, yard) = (day.countries[0].id, day.shipyards[0].id);
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        day.build_ship(yard, ShipKind::Small, BERTH).unwrap();

        let ship = day.ships[0].clone();
        assert_eq!((ship.kind, ship.country, ship.built, ship.x, ship.z, ship.points), (ShipKind::Small, aldoria, day.date, 40.0, -10.0, DAILY_POINTS));
        assert_eq!(ship.name, format!("Small warship {}", ship.id.short()));
        assert_eq!(day.ship(ship.id), Some(&ship));
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 2_000);
        assert_eq!((country.stock.wood, country.stock.iron, country.stock.sulphur), (stock.wood - 400, stock.iron - 100, stock.sulphur - 50));
        assert_eq!((country.stock.stone, country.stock.copper, country.stock.oil), (stock.stone, stock.copper, stock.oil), "nothing else is taken");
        assert_eq!(day.newsreel.last(), Some(&News::about(ship.id, Event::ShipBuilt { name: ship.name.clone(), kind: ShipKind::Small, country: aldoria, region: Some(0x4005), cost: ShipKind::Small.full_cost() })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("Aldoria builds {} in region 5 for 2,000 coins, 400 wood, 100 iron and 50 sulphur", ship.name));
    }

    #[test]
    fn a_large_warship_takes_more_and_a_small_shipyard_cannot_build_one() {
        let mut day = day_with_a_yard();
        let (aldoria, yard) = (day.countries[0].id, day.shipyards[0].id);
        let before = day.clone();
        let err = day.build_ship(yard, ShipKind::Large, BERTH).unwrap_err();
        assert!(err.contains("A small shipyard can only build small warships") && err.contains("upgrade"), "{err}");
        assert_eq!(day, before, "a refused ship changes nothing");

        large_yard(&mut day);
        let (coins, stock) = (day.country(aldoria).unwrap().money, day.country(aldoria).unwrap().stock);
        day.build_ship(yard, ShipKind::Large, BERTH).unwrap();
        let country = day.country(aldoria).unwrap();
        assert_eq!(country.money, coins - 10_000);
        assert_eq!((country.stock.wood, country.stock.iron, country.stock.copper, country.stock.sulphur), (stock.wood - 1_500, stock.iron - 600, stock.copper - 200, stock.sulphur - 200));
        assert_eq!(day.ships[0].kind, ShipKind::Large);
    }

    #[test]
    fn a_shipyard_rests_5_days_after_a_small_ship_if_it_is_small_3_if_it_is_large_and_10_after_a_large_one() {
        let rest_of = |yard_kind: ShipyardKind, ship: ShipKind| {
            let mut day = day_with_a_yard();
            if yard_kind == ShipyardKind::Large {
                large_yard(&mut day);
            }
            let yard = day.shipyards[0].id;
            day.build_ship(yard, ship, BERTH).unwrap();
            let yard = *day.shipyard(yard).unwrap();
            (yard.rest_left(day.date), yard.ready_on, day)
        };
        let (left, ready, day) = rest_of(ShipyardKind::Small, ShipKind::Small);
        assert_eq!((left, ready), (5, Some(Date::FIRST.plus_days(5))));
        assert!(day.ship_problem(day.shipyards[0].id, ShipKind::Small).unwrap().contains("resting") && day.ship_problem(day.shipyards[0].id, ShipKind::Small).unwrap().contains("in 5 days"));
        assert_eq!(rest_of(ShipyardKind::Large, ShipKind::Small).0, 3);
        assert_eq!(rest_of(ShipyardKind::Large, ShipKind::Large).0, 10);
        assert_eq!(ShipyardKind::Small.break_days(ShipKind::Large), None);
        assert!(!ShipyardKind::Small.can_build(ShipKind::Large) && ShipyardKind::Large.can_build(ShipKind::Large));
    }

    #[test]
    fn a_resting_shipyard_builds_nothing_until_its_break_is_over_and_then_builds_again() {
        let mut day = day_with_a_yard();
        let yard = day.shipyards[0].id;
        day.build_ship(yard, ShipKind::Small, BERTH).unwrap();
        let ships = day.ships.len();
        // Each day the break is a day shorter; the ship is not built on any of the 5 days of it.
        let mut today = day;
        for left in (1..=5).rev() {
            assert_eq!(today.shipyard(yard).unwrap().rest_left(today.date), left);
            let before = today.clone();
            let err = today.build_ship(yard, ShipKind::Small, vec2(80.0, 0.0)).unwrap_err();
            assert!(err.contains(&format!("in {}", days_text(left))), "{err}");
            assert_eq!(today, before);
            today = today.next_day();
        }
        assert_eq!(today.shipyard(yard).unwrap().rest_left(today.date), 0, "5 days after, it can build again");
        today.build_ship(yard, ShipKind::Small, vec2(80.0, 0.0)).unwrap();
        assert_eq!(today.ships.len(), ships + 1);
    }

    #[test]
    fn a_large_shipyard_can_build_a_small_ship_after_3_days_but_a_large_one_not_until_10() {
        let mut day = day_with_a_yard();
        let yard = large_yard(&mut day);
        day.build_ship(yard, ShipKind::Large, BERTH).unwrap();
        let mut today = day;
        for _ in 0..9 {
            today = today.next_day();
            assert!(today.ship_problem(yard, ShipKind::Small).is_some() && today.ship_problem(yard, ShipKind::Large).is_some());
        }
        today = today.next_day();
        assert_eq!(today.ship_problem(yard, ShipKind::Large), None, "10 days later");
    }

    #[test]
    fn a_ship_needs_the_coin_each_material_and_the_players_own_country() {
        let mut broke = day_with_a_yard();
        let (id, yard) = (broke.countries[0].id, broke.shipyards[0].id);
        let money = broke.country(id).unwrap().money;
        broke.add_money(id, -(money - 1_999)).unwrap();
        let err = broke.build_ship(yard, ShipKind::Small, BERTH).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("a small warship costs 2,000") && err.contains("1,999"), "{err}");

        for (resource, name, needed) in [(Resource::Forest, "wood", 400), (Resource::Iron, "iron", 100), (Resource::Sulphur, "sulphur", 50)] {
            let mut day = day_with_a_yard();
            let id = day.countries[0].id;
            let has = day.country(id).unwrap().stock.get(resource);
            day.add_resource(id, resource, -(has as i64 - needed as i64 + 1)).unwrap(); // One short.
            let before = day.clone();
            let err = day.build_ship(day.shipyards[0].id, ShipKind::Small, BERTH).unwrap_err();
            assert!(err.contains(&format!("Not enough {name}")), "{err}");
            assert_eq!(day, before, "a refused ship changes nothing");
        }

        // Only the country being played builds ships, and at a shipyard of its own.
        let mut day = day_with_a_yard();
        let (brevon, yard) = (day.countries[1].id, day.shipyards[0].id);
        day.shipyards[0].country = brevon;
        assert!(day.ship_problem(yard, ShipKind::Small).unwrap().contains("belongs to Brevon"));
        assert!(day.build_ship(ShipyardId::new(), ShipKind::Small, BERTH).is_err());
        let mut day = day_with_a_yard();
        let yard = day.shipyards[0].id;
        assert!(day.build_ship(yard, ShipKind::Small, vec2(f32::NAN, 0.0)).is_err());
        assert!(day.ships.is_empty());
    }

    #[test]
    fn moving_costs_points_moves_the_ship_and_is_reported() {
        let mut day = day_with_a_yard();
        day.build_ship(day.shipyards[0].id, ShipKind::Small, BERTH).unwrap();
        let id = day.ships[0].id;
        day.move_ship(id, vec2(70.0, 5.0), 35, Some(0xC001)).unwrap();
        let ship = day.ship(id).unwrap();
        assert_eq!((ship.x, ship.z, ship.points), (70.0, 5.0, 165));
        assert_eq!(day.newsreel.last(), Some(&News::about(id, Event::ShipMoved { name: ship.name.clone(), points: 35, region: Some(0xC001) })));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("{} sails to {}, spending 35 movement points", ship.name, day.region_label(0xC001)));
        // It can't spend more than it has, and a refused move changes nothing.
        let before = day.clone();
        let err = day.move_ship(id, vec2(90.0, 5.0), 166, None).unwrap_err();
        assert!(err.contains("only 165 movement points left") && err.contains("166"), "{err}");
        assert!(day.move_ship(id, vec2(f32::NAN, 0.0), 1, None).is_err() && day.move_ship(ShipId::new(), vec2(1.0, 1.0), 1, None).is_err());
        assert_eq!(day, before);
        day.move_ship(id, vec2(90.0, 5.0), 165, None).unwrap();
        assert_eq!(day.ship(id).unwrap().points, 0);
    }

    #[test]
    fn the_points_come_back_each_day_and_are_saved() {
        let mut day = day_with_a_yard();
        day.build_ship(day.shipyards[0].id, ShipKind::Small, BERTH).unwrap();
        let id = day.ships[0].id;
        day.move_ship(id, vec2(70.0, 5.0), 180, None).unwrap();
        let mut tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.ship(id).unwrap().points, 20);
        tomorrow.rest_ships();
        assert_eq!(tomorrow.ship(id).unwrap().points, DAILY_POINTS);
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["ships"][0]["points"], 20);
        assert_eq!(json["ships"][0]["kind"], "small");
        assert_eq!(serde_json::from_value::<DaySave>(json.clone()).unwrap(), day);
        // A day saved before there were ships has none, and a shipyard saved before has no rest to take.
        let mut old = json;
        old.as_object_mut().unwrap().remove("ships");
        old["shipyards"][0].as_object_mut().unwrap().remove("ready_on");
        let old: DaySave = serde_json::from_value(old).unwrap();
        assert!(old.ships.is_empty() && old.shipyards[0].ready_on.is_none());
    }

    #[test]
    fn a_ship_can_be_renamed_given_to_another_country_and_scrapped() {
        let mut day = day_with_a_yard();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.build_ship(day.shipyards[0].id, ShipKind::Small, BERTH).unwrap();
        let (id, first) = (day.ships[0].id, day.ships[0].name.clone());

        day.rename_ship(id, "  Sea   Wolf ").unwrap();
        assert_eq!(day.ship(id).unwrap().name, "Sea Wolf");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), format!("{first} is renamed Sea Wolf"));
        let news = day.newsreel.len();
        day.rename_ship(id, "Sea Wolf").unwrap();
        assert_eq!(day.newsreel.len(), news, "no change is no news");
        assert!(day.rename_ship(id, " ").is_err() && day.rename_ship(ShipId::new(), "x").is_err());

        day.set_ship_country(id, brevon).unwrap();
        assert_eq!(day.ship(id).unwrap().country, brevon);
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Sea Wolf passes from Aldoria to Brevon");
        let news = day.newsreel.len();
        day.set_ship_country(id, brevon).unwrap();
        assert_eq!(day.newsreel.len(), news);
        assert!(day.set_ship_country(id, CountryId::new()).is_err() && day.set_ship_country(ShipId::new(), aldoria).is_err());

        let money = day.country(aldoria).unwrap().money;
        day.scrap_ship(id).unwrap();
        assert!(day.ships.is_empty() && day.country(aldoria).unwrap().money == money, "nothing is paid back");
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Sea Wolf is scrapped");
        assert!(day.scrap_ship(id).is_err());
    }

    #[test]
    fn a_country_pays_for_its_ships_oldest_first_and_loses_the_ones_it_cannot_pay_for() {
        let mut day = day_with_a_yard();
        let (aldoria, yard) = (day.countries[0].id, large_yard(&mut day));
        day.build_ship(yard, ShipKind::Large, BERTH).unwrap();
        let (large, mut today) = (day.ships[0].id, day);
        for _ in 0..10 {
            today = today.following(today.date.next());
        }
        today.build_ship(yard, ShipKind::Small, vec2(80.0, 0.0)).unwrap();
        let small = today.ships[1].id;
        assert_eq!((today.ships[0].id, today.ships[1].kind), (large, ShipKind::Small), "the oldest first");

        // With money for both they are both paid for: 250 and 50.
        let money = today.country(aldoria).unwrap().money;
        let mut paid = today.clone();
        paid.pay_ships();
        assert_eq!(paid.country(aldoria).unwrap().money, money - 300);
        assert_eq!(paid.ships.len(), 2);
        assert_eq!(paid.newsreel.last().unwrap().headline(&paid), "Aldoria pays 300 coins to keep its ships");

        // With 260 coins the large one (the oldest) is paid for, and the small one, which would take 50 more, is abandoned.
        let mut short = today.clone();
        short.add_money(aldoria, -(money - 260)).unwrap();
        short.pay_ships();
        assert_eq!(short.ships.iter().map(|s| s.id).collect::<Vec<_>>(), [large]);
        assert_eq!(short.country(aldoria).unwrap().money, 10);
        let name = today.ship(small).unwrap().name.clone();
        assert!(short.newsreel.iter().any(|n| n.headline(&short) == format!("The crew of {name} desert for want of pay, and it is abandoned")));

        // With less than the oldest costs, the oldest is lost, and the next, which is cheaper, is kept: 100 coins keeps the small one.
        let mut poor = today.clone();
        poor.add_money(aldoria, -(money - 100)).unwrap();
        poor.pay_ships();
        assert_eq!(poor.ships.iter().map(|s| s.id).collect::<Vec<_>>(), [small]);
        assert_eq!(poor.country(aldoria).unwrap().money, 50);

        // With none at all, none are kept, and a ship of a country that has no money doesn't make it owe.
        let mut none = today.clone();
        none.add_money(aldoria, -money).unwrap();
        none.pay_ships();
        assert!(none.ships.is_empty() && none.country(aldoria).unwrap().money == 0);
        assert!(!none.newsreel.iter().any(|n| matches!(n.event, Event::ShipPaid { .. })), "nothing was paid");
    }
}
