//! Raising militia, changing the country one belongs to, and paying to keep them.

use macroquad::math::{vec2, Vec2};

use super::DaySave;
use crate::format;
use crate::ids::{CityId, CountryId, MilitiaId};
use crate::militia::{self, Militia};
use crate::movement::DAILY_POINTS;
use crate::newsreel::{Cost, Event, News};

impl DaySave {
    /// The militia with this id.
    pub fn militia(&self, id: MilitiaId) -> Option<&Militia> {
        self.militias.iter().find(|m| m.id == id)
    }

    /// Where the militia raised in the city with this id are camped.
    pub fn camps_of(&self, city: CityId) -> Vec<Vec2> {
        self.militias.iter().filter(|m| m.city == city).map(|m| vec2(m.x, m.z)).collect()
    }

    /// Whether the city with this id has raised a militia today, which it can do once a day.
    pub fn raised_militia_today(&self, city: CityId) -> bool {
        self.militias.iter().any(|m| m.city == city && m.raised == self.date)
    }

    /// Why the city with this id can't raise any militia now, or `None` if it can raise at least one: it can raise one
    /// group a day, it needs people enough for one in ten of them to be a member, and the country it belongs to has to be
    /// able to pay for a member.
    pub fn militia_problem(&self, city: CityId) -> Option<String> {
        let Some(city) = self.city(city) else { return Some("There is no such city.".to_owned()) };
        if self.raised_militia_today(city.id) {
            return Some(format!("{} has already raised a militia today.", city.name));
        }
        if militia::most_from(city.population) == 0 {
            return Some(format!("{} has too few people to raise a militia.", city.name));
        }
        let Some(country) = self.country(city.country) else { return Some("There is no country for the militia to belong to.".to_owned()) };
        (country.money < militia::raising_cost(1)).then(|| {
            format!("Not enough coin: a member costs {} coins to raise, and {} has {}.", militia::RAISING_COST, country.name, format::money(country.money))
        })
    }

    /// The most members the city with this id could raise now: a tenth of its people, or 1,000, whichever is fewer, and no
    /// more than its country can pay for. None at all if it can't raise any (see [`militia_problem`](Self::militia_problem)).
    pub fn most_militia(&self, city: CityId) -> u32 {
        if self.militia_problem(city).is_some() {
            return 0;
        }
        let Some(city) = self.city(city) else { return 0 };
        let affordable = self.country(city.country).map_or(0, |c| c.money.max(0) / militia::RAISING_COST);
        militia::most_from(city.population).min(u32::try_from(affordable).unwrap_or(u32::MAX))
    }

    /// Raises a militia of `members` people in the city with this id, camped at `camp` (see [`militia::camp_near`]), for
    /// the country the city belongs to, which pays 10 coins a member. The members leave the city's population. This is
    /// reported in the newsreel. An error says why it can't be done: a city raises one militia a day, of no more than a
    /// tenth of its people and 1,000 members.
    pub fn raise_militia(&mut self, city: CityId, members: u32, camp: Vec2) -> Result<(), String> {
        let found = self.city(city).ok_or("There is no such city.")?;
        let (name, country, population) = (found.name.clone(), found.country, found.population);
        if !camp.x.is_finite() || !camp.y.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if self.raised_militia_today(city) {
            return Err(format!("{name} has already raised a militia today."));
        }
        if members == 0 {
            return Err("A militia needs at least 1 member.".to_owned());
        }
        let most = militia::most_from(population);
        if members > most {
            return Err(format!("{name} can raise no more than {} members: a tenth of its {} people, and never over {}.", format::thousands(most), format::thousands(population), format::thousands(militia::MAX_MEMBERS)));
        }
        let cost = militia::raising_cost(members);
        let country_name = self.country_name(country).to_owned();
        let money = self.country(country).ok_or("There is no country for the militia to belong to.")?.money;
        if money < cost {
            return Err(format!("Not enough coin: {} members cost {} coins to raise, and {country_name} has {}.", format::thousands(members), format::money(cost), format::money(money)));
        }
        self.add_money(country, -cost)?;
        if let Some(city) = self.cities.iter_mut().find(|c| c.id == city) {
            city.population -= members;
        }
        let id = MilitiaId::new();
        let name = militia::default_name(&name, self.date);
        self.militias.push(Militia { id, name: name.clone(), country, city, raised: self.date, members, x: camp.x, z: camp.y, points: DAILY_POINTS });
        self.newsreel.push(News::about(id, Event::MilitiaRaised { name, country, city, members, cost: vec![Cost::coins(cost)] }));
        Ok(())
    }

    /// Renames the militia with this id to `typed`, which is reported in the newsreel unless it was called that already. An
    /// error says why it can't be done.
    pub fn rename_militia(&mut self, id: MilitiaId, typed: &str) -> Result<(), String> {
        let name = militia::clean_name(typed)?;
        let militia = self.militias.iter_mut().find(|m| m.id == id).ok_or("There is no such militia.")?;
        if militia.name != name {
            let from = std::mem::replace(&mut militia.name, name.clone());
            self.newsreel.push(News::about(id, Event::MilitiaRenamed { from, to: name }));
        }
        Ok(())
    }

    /// Moves the militia with this id to `to`, which costs it `cost` of its movement points: that, and whether the ground
    /// between is fit to cross, is for the caller to say (see [`crate::movement::reach`]). `region` is the number of the
    /// map region it gets to, if the map has them. This is reported in the newsreel. An error says why it can't be done.
    pub fn move_militia(&mut self, id: MilitiaId, to: Vec2, cost: u32, region: Option<u16>) -> Result<(), String> {
        if !to.x.is_finite() || !to.y.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        let militia = self.militias.iter_mut().find(|m| m.id == id).ok_or("There is no such militia.")?;
        if cost > militia.points {
            return Err(format!("{} has only {} movement points left, and that takes {cost}.", militia.title(), militia.points));
        }
        militia.points -= cost;
        (militia.x, militia.z) = (to.x, to.y);
        let name = militia.title().to_owned();
        self.newsreel.push(News::about(id, Event::MilitiaMoved { name, points: cost, region }));
        Ok(())
    }

    /// Gives every militia all its movement points again, for a new day.
    pub fn rest_militia(&mut self) {
        for militia in &mut self.militias {
            militia.points = DAILY_POINTS;
        }
    }

    /// Makes the militia with this id belong to `country`, which pays to keep it from now on. This is reported in the
    /// newsreel unless it belonged to it already. An error says why it can't be done.
    pub fn set_militia_country(&mut self, id: MilitiaId, country: CountryId) -> Result<(), String> {
        self.country(country).ok_or("There is no such country.")?;
        let militia = self.militias.iter_mut().find(|m| m.id == id).ok_or("There is no such militia.")?;
        let (from, members) = (std::mem::replace(&mut militia.country, country), militia.members);
        if from != country {
            self.newsreel.push(News::about(id, Event::MilitiaTransferred { from, to: country, members }));
        }
        Ok(())
    }

    /// Each country pays 1 coin for each member of its militia, the oldest first. A member it can't pay for deserts, and
    /// is gone: a country with no money keeps none, and one with money for some keeps that many. The payment, and each
    /// militia that lost members, is in the newsreel. A militia that has none left is gone.
    pub fn pay_militia(&mut self) {
        for index in 0..self.countries.len() {
            let country = self.countries[index].id;
            let mut affordable = self.countries[index].money.max(0) as u64;
            let (mut paid, mut deserted) = (0u64, Vec::new());
            for militia in self.militias.iter_mut().filter(|m| m.country == country) {
                let wanted = u64::from(militia.members);
                let pay = wanted.min(affordable / militia::DAILY_UPKEEP as u64);
                affordable -= pay * militia::DAILY_UPKEEP as u64;
                paid += pay * militia::DAILY_UPKEEP as u64;
                if pay < wanted {
                    deserted.push(News::about(militia.id, Event::MilitiaDeserted { name: militia.title().to_owned(), country, deserters: (wanted - pay) as u32, remaining: pay as u32 }));
                    militia.members = pay as u32;
                }
            }
            if paid > 0 {
                self.countries[index].money -= paid as i64;
                self.newsreel.push(News::about(country, Event::MilitiaPaid { amount: paid }));
            }
            self.newsreel.extend(deserted);
        }
        self.militias.retain(|m| m.members > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::day::day;

    const CAMP: Vec2 = Vec2::new(40.0, -10.0);

    #[test]
    fn a_militia_starts_with_all_its_movement_points() {
        let mut day = day_with_a_city();
        day.raise_militia(day.cities[0].id, 20, CAMP).unwrap();
        assert_eq!(day.militias[0].points, DAILY_POINTS);
    }

    #[test]
    fn moving_costs_points_moves_the_militia_and_is_reported() {
        let mut day = day_with_a_city();
        day.raise_militia(day.cities[0].id, 20, CAMP).unwrap();
        let id = day.militias[0].id;
        day.move_militia(id, vec2(70.0, 5.0), 35, Some(0xC001)).unwrap();
        let militia = day.militia(id).unwrap();
        assert_eq!((militia.x, militia.z, militia.points), (70.0, 5.0, 165));
        assert_eq!(day.newsreel.last(), Some(&News::about(id, Event::MilitiaMoved { name: "Port Aldor Militia 1-1-1".to_owned(), points: 35, region: Some(0xC001) })));
        // It can't spend more than it has, and a refused move changes nothing.
        let before = day.clone();
        let err = day.move_militia(id, vec2(90.0, 5.0), 166, None).unwrap_err();
        assert!(err.contains("only 165 movement points left") && err.contains("166"), "{err}");
        assert!(day.move_militia(id, vec2(f32::NAN, 0.0), 1, None).is_err());
        assert!(day.move_militia(MilitiaId::new(), vec2(1.0, 1.0), 1, None).is_err());
        assert_eq!(day, before);
        // All the points that are left can be spent.
        day.move_militia(id, vec2(90.0, 5.0), 165, None).unwrap();
        assert_eq!(day.militia(id).unwrap().points, 0);
    }

    #[test]
    fn the_points_come_back_each_day_and_are_saved() {
        let mut day = day_with_a_city();
        day.raise_militia(day.cities[0].id, 20, CAMP).unwrap();
        let id = day.militias[0].id;
        day.move_militia(id, vec2(70.0, 5.0), 180, None).unwrap();
        // They carry forward as they are, and the rule gives them back.
        let mut tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.militia(id).unwrap().points, 20);
        tomorrow.rest_militia();
        assert_eq!(tomorrow.militia(id).unwrap().points, DAILY_POINTS);
        assert_eq!(day.militia(id).unwrap().points, 20, "the day that ended is as it was");
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["militias"][0]["points"], 20);
        // One saved before there were movement points has all of them.
        let mut old = json;
        old["militias"][0].as_object_mut().unwrap().remove("points");
        assert_eq!(serde_json::from_value::<DaySave>(old).unwrap().militias[0].points, DAILY_POINTS);
    }

    /// A day with a country that has 5,000 coins and a city of 2,000 people.
    fn day_with_a_city() -> DaySave {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.add_money(day.countries[0].id, 5_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 2_000;
        day
    }

    #[test]
    fn raising_a_militia_takes_its_members_from_the_city_and_10_coins_each_from_the_country() {
        let mut day = day_with_a_city();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.raise_militia(port, 150, CAMP).unwrap();

        assert_eq!(day.cities[0].population, 1_850);
        assert_eq!(day.country(aldoria).unwrap().money, 5_000 - 1_500);
        let militia = day.militias[0].clone();
        assert_eq!((militia.country, militia.city, militia.members, militia.raised), (aldoria, port, 150, day.date));
        assert_eq!((militia.x, militia.z), (40.0, -10.0));
        assert_eq!(day.militia(militia.id), Some(&militia));
        assert_eq!(day.newsreel.last(), Some(&News::about(militia.id, Event::MilitiaRaised { name: "Port Aldor Militia 1-1-1".to_owned(), country: aldoria, city: port, members: 150, cost: vec![Cost::coins(1_500)] })));
    }

    #[test]
    fn a_militia_can_be_a_tenth_of_the_city_and_no_more() {
        let mut day = day_with_a_city();
        let port = day.cities[0].id;
        let before = day.clone();
        let err = day.raise_militia(port, 201, CAMP).unwrap_err();
        assert!(err.contains("no more than 200") && err.contains("2,000"), "{err}");
        assert_eq!(day, before, "a refused militia changes nothing");
        day.raise_militia(port, 200, CAMP).unwrap();
        assert_eq!(day.cities[0].population, 1_800);
    }

    #[test]
    fn a_militia_never_has_more_than_a_thousand_members() {
        let mut day = day_with_a_city();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.add_money(aldoria, 100_000).unwrap();
        day.cities[0].population = 100_000;
        assert_eq!(day.most_militia(port), 1_000);
        let err = day.raise_militia(port, 1_001, CAMP).unwrap_err();
        assert!(err.contains("no more than 1,000"), "{err}");
        day.raise_militia(port, 1_000, CAMP).unwrap();
        assert_eq!(day.cities[0].population, 99_000);
    }

    #[test]
    fn a_militia_needs_at_least_one_member() {
        let mut day = day_with_a_city();
        let before = day.clone();
        assert!(day.raise_militia(day.cities[0].id, 0, CAMP).is_err());
        assert_eq!(day, before);
    }

    #[test]
    fn a_city_raises_one_militia_a_day() {
        let mut day = day_with_a_city();
        day.found_city("Highmark", 500.0, 0.0, None).unwrap();
        day.cities[1].population = 1_000;
        let (port, highmark) = (day.cities[0].id, day.cities[1].id);
        assert_eq!(day.militia_problem(port), None);

        day.raise_militia(port, 50, CAMP).unwrap();
        let again = day.raise_militia(port, 50, CAMP).unwrap_err();
        assert!(again.contains("Port Aldor") && again.contains("already raised a militia today"), "{again}");
        assert_eq!(day.militia_problem(port).as_deref(), Some("Port Aldor has already raised a militia today."));
        assert_eq!(day.most_militia(port), 0);
        assert_eq!(day.militias.len(), 1);
        // Another city is not stopped by it.
        assert_eq!(day.militia_problem(highmark), None);
        day.raise_militia(highmark, 50, CAMP).unwrap();

        // The next day, it can raise another.
        let mut tomorrow = day.following(Date { year: 1, month: 1, day: 2 });
        assert_eq!(tomorrow.militia_problem(port), None);
        tomorrow.raise_militia(port, 50, CAMP).unwrap();
        assert_eq!(tomorrow.militias.len(), 3, "the militia raised yesterday are still there");
    }

    #[test]
    fn a_country_must_be_able_to_pay_for_the_members() {
        let mut day = day_with_a_city();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.add_money(aldoria, -4_950).unwrap(); // 50 coins left.
        assert_eq!(day.most_militia(port), 5, "50 coins pay for 5 members");
        let before = day.clone();
        let err = day.raise_militia(port, 6, CAMP).unwrap_err();
        assert!(err.contains("Not enough coin") && err.contains("60 coins") && err.contains("Aldoria has 50"), "{err}");
        assert_eq!(day, before);
        day.raise_militia(port, 5, CAMP).unwrap();
        assert_eq!(day.country(aldoria).unwrap().money, 0);

        // No money, or debt, and there is no militia at all.
        day.found_city("Highmark", 500.0, 0.0, None).unwrap();
        let highmark = day.cities[1].id;
        assert!(day.militia_problem(highmark).unwrap().contains("Not enough coin"));
        day.add_money(aldoria, -100).unwrap();
        assert_eq!(day.most_militia(highmark), 0);
    }

    #[test]
    fn a_city_too_small_to_spare_anyone_raises_nothing() {
        let mut day = day_with_a_city();
        let port = day.cities[0].id;
        day.cities[0].population = 9;
        assert!(day.militia_problem(port).unwrap().contains("too few people"));
        assert!(day.raise_militia(port, 1, CAMP).is_err());
    }

    #[test]
    fn a_militia_belongs_to_the_country_of_the_city_that_raised_it_and_that_country_pays() {
        let mut day = day_with_a_city();
        let (aldoria, brevon, port) = (day.countries[0].id, day.countries[1].id, day.cities[0].id);
        day.add_money(brevon, 400).unwrap();
        day.set_city_country(port, brevon).unwrap();
        day.raise_militia(port, 40, CAMP).unwrap();
        assert_eq!(day.militias[0].country, brevon);
        assert_eq!((day.country(aldoria).unwrap().money, day.country(brevon).unwrap().money), (5_000, 0));
    }

    #[test]
    fn nothing_that_is_not_there_can_raise_one() {
        let mut day = day_with_a_city();
        assert!(day.raise_militia(CityId::new(), 5, CAMP).is_err());
        assert_eq!(day.militia_problem(CityId::new()).as_deref(), Some("There is no such city."));
        assert!(day.raise_militia(day.cities[0].id, 5, vec2(f32::NAN, 0.0)).is_err());
        assert!(day.militias.is_empty());
    }

    #[test]
    fn a_militia_is_named_for_its_city_and_the_day_and_can_be_renamed() {
        let mut day = day_with_a_city();
        let port = day.cities[0].id;
        day.raise_militia(port, 30, CAMP).unwrap();
        let id = day.militias[0].id;
        assert_eq!(day.militias[0].name, "Port Aldor Militia 1-1-1");
        // On another day, the day is in the name.
        let mut later = day.following(Date { year: 2, month: 5, day: 9 });
        later.raise_militia(port, 30, CAMP).unwrap();
        assert_eq!(later.militias[1].name, "Port Aldor Militia 2-5-9");

        let news = day.newsreel.len();
        day.rename_militia(id, "  Harbour   Guard ").unwrap();
        assert_eq!(day.militia(id).unwrap().name, "Harbour Guard");
        assert_eq!(day.newsreel.last(), Some(&News::about(id, Event::MilitiaRenamed { from: "Port Aldor Militia 1-1-1".to_owned(), to: "Harbour Guard".to_owned() })));
        // The same name again is no change, and a bad one or a missing militia is refused.
        let before = day.clone();
        day.rename_militia(id, "Harbour Guard").unwrap();
        assert!(day.rename_militia(id, "  ").is_err() && day.rename_militia(id, "Guard #1").is_err());
        assert!(day.rename_militia(MilitiaId::new(), "Guard").is_err());
        assert_eq!(day, before);
        assert_eq!(day.newsreel.len(), news + 1);
        // Two can share a name.
        let second = later.militias[1].id;
        later.rename_militia(second, "Port Aldor Militia 1-1-1").unwrap();
    }

    #[test]
    fn a_militia_changes_country_and_the_change_is_reported_once() {
        let mut day = day_with_a_city();
        let (aldoria, brevon, port) = (day.countries[0].id, day.countries[1].id, day.cities[0].id);
        day.raise_militia(port, 30, CAMP).unwrap();
        let id = day.militias[0].id;
        let news = day.newsreel.len();

        day.set_militia_country(id, brevon).unwrap();
        assert_eq!(day.militia(id).unwrap().country, brevon);
        assert_eq!(day.newsreel.last(), Some(&News::about(id, Event::MilitiaTransferred { from: aldoria, to: brevon, members: 30 })));
        let before = day.clone();
        day.set_militia_country(id, brevon).unwrap();
        assert_eq!(day, before, "the same country again is no change");
        assert_eq!(day.newsreel.len(), news + 1);

        assert!(day.set_militia_country(id, CountryId::new()).is_err());
        assert!(day.set_militia_country(MilitiaId::new(), aldoria).is_err());
        // The city it came from stays Aldoria's.
        assert_eq!(day.cities[0].country, aldoria);
    }

    #[test]
    fn a_country_pays_a_coin_a_day_for_each_member() {
        let mut day = day_with_a_city();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.raise_militia(port, 100, CAMP).unwrap();
        day.newsreel.clear();
        let money = day.country(aldoria).unwrap().money;

        day.pay_militia();
        assert_eq!(day.country(aldoria).unwrap().money, money - 100);
        assert_eq!(day.militias[0].members, 100);
        assert_eq!(day.newsreel, [News::about(aldoria, Event::MilitiaPaid { amount: 100 })]);
        assert_eq!(day.cities[0].population, 1_900, "keeping them doesn't touch the city");
    }

    #[test]
    fn members_that_cannot_be_paid_for_desert_and_do_not_return_to_the_city() {
        let mut day = day_with_a_city();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        day.raise_militia(port, 100, CAMP).unwrap();
        let id = day.militias[0].id;
        let money = day.country(aldoria).unwrap().money;
        day.add_money(aldoria, 60 - money).unwrap(); // Money for 60 of the 100.
        day.newsreel.clear();

        day.pay_militia();
        assert_eq!(day.militias[0].members, 60);
        assert_eq!(day.country(aldoria).unwrap().money, 0);
        assert_eq!(day.cities[0].population, 1_900, "the 40 who left are not back in the city");
        assert_eq!(
            day.newsreel,
            [News::about(aldoria, Event::MilitiaPaid { amount: 60 }), News::about(id, Event::MilitiaDeserted { name: "Port Aldor Militia 1-1-1".to_owned(), country: aldoria, deserters: 40, remaining: 60 })]
        );
    }

    #[test]
    fn a_country_with_no_money_or_in_debt_loses_its_whole_militia() {
        for money in [0, -300] {
            let mut day = day_with_a_city();
            let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
            day.raise_militia(port, 100, CAMP).unwrap();
            let current = day.country(aldoria).unwrap().money;
            day.add_money(aldoria, money - current).unwrap();
            day.newsreel.clear();

            day.pay_militia();
            assert!(day.militias.is_empty(), "{money}: they all desert");
            assert_eq!(day.country(aldoria).unwrap().money, money, "and nothing is paid");
            assert_eq!(day.newsreel.len(), 1, "{money}: no payment to report, only the desertion");
            assert!(matches!(day.newsreel[0].event, Event::MilitiaDeserted { deserters: 100, remaining: 0, .. }));
        assert!(matches!(&day.newsreel[0].event, Event::MilitiaDeserted { name, .. } if name == "Port Aldor Militia 1-1-1"), "the name is kept, for the militia is gone");
        }
    }

    #[test]
    fn the_oldest_militia_are_paid_for_first() {
        let mut day = day_with_a_city();
        let aldoria = day.countries[0].id;
        day.found_city("Highmark", 500.0, 0.0, None).unwrap();
        day.cities[1].population = 1_000;
        let (port, highmark) = (day.cities[0].id, day.cities[1].id);
        day.raise_militia(port, 100, CAMP).unwrap();
        day.raise_militia(highmark, 100, CAMP).unwrap();
        let money = day.country(aldoria).unwrap().money;
        day.add_money(aldoria, 130 - money).unwrap();

        day.pay_militia();
        assert_eq!(day.militias.iter().map(|m| (m.city, m.members)).collect::<Vec<_>>(), [(port, 100), (highmark, 30)]);
    }

    #[test]
    fn each_country_pays_for_its_own_militia_only() {
        let mut day = day_with_a_city();
        let (aldoria, brevon, port) = (day.countries[0].id, day.countries[1].id, day.cities[0].id);
        day.raise_militia(port, 100, CAMP).unwrap();
        day.raise_militia(port, 1, CAMP).unwrap_err();
        let id = day.militias[0].id;
        day.set_militia_country(id, brevon).unwrap();
        day.add_money(brevon, 100).unwrap();
        let aldoria_money = day.country(aldoria).unwrap().money;

        day.pay_militia();
        assert_eq!(day.country(brevon).unwrap().money, 0, "Brevon pays for it now");
        assert_eq!(day.country(aldoria).unwrap().money, aldoria_money, "and Aldoria doesn't");
        assert_eq!(day.militias[0].members, 100);
    }

    #[test]
    fn a_day_with_no_militia_pays_and_reports_nothing() {
        let mut day = day_with_a_city();
        let before = day.clone();
        day.pay_militia();
        assert_eq!(day, before);
    }

    #[test]
    fn militia_are_saved_and_carry_forward() {
        let mut day = day_with_a_city();
        day.raise_militia(day.cities[0].id, 75, CAMP).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["militias"][0]["members"], 75);
        assert_eq!(json["militias"][0]["country"], json["countries"][0]["id"]);
        assert_eq!(json["militias"][0]["city"], json["cities"][0]["id"]);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).militias, day.militias);
    }

    #[test]
    fn a_day_saved_before_there_was_militia_has_none() {
        let mut json = serde_json::to_value(day_with_a_city()).unwrap();
        json.as_object_mut().unwrap().remove("militias");
        assert!(serde_json::from_value::<DaySave>(json).unwrap().militias.is_empty());
    }
}
