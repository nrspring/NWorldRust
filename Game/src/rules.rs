//! The rules that run when the day changes. When the player ends a day, the next day starts as a copy of
//! it (see [`DaySave::following`]) and then each rule here runs on the new day, one after another in the
//! order they are listed, so a later rule sees what the earlier ones did.
//!
//! A rule is a function that changes the new day. To add one, write the function and put it in [`DAY_RULES`]
//! with a sentence saying what it does. The popup that asks about ending the day doesn't list them.

use crate::city::{MAX_POPULATION, MIN_POPULATION};
use crate::day::{DaySave, Round};
use crate::ids;
use crate::newsreel::{Event, News};
use crate::gatherer::GathererKind;

/// A rule that runs on a day switch.
pub struct Rule {
    /// What it does, in a sentence. Nothing in the game shows it; it is there for whoever reads the rules, and the
    /// tests check that every rule has one.
    #[cfg_attr(not(test), allow(dead_code))]
    pub description: &'static str,
    /// Changes the day that is starting.
    apply: fn(&mut DaySave),
}

/// How much a city that is not under siege grows by in a day, as a percent of its population: somewhere
/// between these.
const GROWTH_PERCENT: (f64, f64) = (1.0, 3.0);
/// How much a city under siege shrinks by in a day, as a percent of its population: somewhere between these.
pub const SIEGE_LOSS_PERCENT: (f64, f64) = (5.0, 10.0);

/// The rules, in the order they run.
pub const DAY_RULES: &[Rule] = &[
    Rule { description: "Each country earns 1 coin for each person living in its cities.", apply: income },
    Rule {
        description: "A city that is not under siege grows by 1 to 3 percent, and one under siege shrinks by 5 to 10 percent, each by a random amount. No city has fewer than 100 people or more than 100,000.",
        apply: population,
    },
    Rule {
        description: "A path has a 2% chance of failing, a road 0.1% and a bridge 0.05%. What fails is ruined and is listed in the alerts until it is built again.",
        apply: failures,
    },
    Rule { description: "Each woodcutter, sawmill, stonecutter or quarry that has a path, road or bridge route to a city of its country provides its wood or stone to the country each day: 100 from a woodcutter or stonecutter, and 1,000 from a sawmill or quarry.", apply: gather },
    Rule { description: "Each militia gets all of its movement points back: 200 a day. A garrison gets none.", apply: militia_rest },
    Rule {
        description: "A militia under siege loses 5 to 10 percent of its members, each by a random amount, rounded up so that it loses at least one, and is never left with fewer than 1.",
        apply: militia_siege,
    },
    Rule { description: "Each ship gets all of its movement points back: 200 a day. One at anchor gets none.", apply: ships_rest },
    Rule { description: "Each person gets all of their movement points back: 200 a day, as the army does.", apply: people_rest },
    Rule {
        description: "Each person is a day older. From 25 years old they may die, with a chance that grows each year, and much faster after 60, so that on average a person lives to 50.",
        apply: people_age,
    },
    Rule {
        description: "Each country pays first for the militia picked to be paid, 1 coin a day for each member, the oldest first. A member it can't pay for deserts, and does not go back to a city.",
        apply: militia_upkeep_first,
    },
    Rule {
        description: "Each country then pays a day's upkeep for each of its ships picked to be paid, the oldest first: 50 coins for a small warship and 250 for a large one. A ship it can't pay for is abandoned by its crew and is gone.",
        apply: ships_upkeep_first,
    },
    Rule {
        description: "Each country pays 1 coin a day for each member of the rest of its militia, the oldest first, with what it has left. A member it can't pay for deserts, and does not go back to a city.",
        apply: militia_upkeep,
    },
    Rule {
        description: "Each country pays a day's upkeep for the rest of its ships, the oldest first, with what it has left. A ship it can't pay for is abandoned by its crew and is gone.",
        apply: ships_upkeep,
    },
];

/// Runs every rule, in order, on `day`, which has just started.
pub fn run(day: &mut DaySave) {
    for rule in DAY_RULES {
        (rule.apply)(day);
    }
}

/// Each country gets 1 coin for each person in the cities it owns: the total population of its cities. A country
/// that earned something is in the newsreel.
fn income(day: &mut DaySave) {
    for country in &mut day.countries {
        let population: u64 = day.cities.iter().filter(|city| city.country == country.id).map(|city| city.population as u64).sum();
        let before = country.money;
        country.money = before.saturating_add(i64::try_from(population).unwrap_or(i64::MAX));
        let earned = country.money - before;
        if earned > 0 {
            day.newsreel.push(News::about(country.id, Event::Income { amount: earned }));
        }
    }
}

/// Each gatherer that is operating, which is to say that has a route to a city of its country (see
/// [`DaySave::cut_off_gatherers`]), adds what its kind provides in a day (`GathererKind::daily_yield`) to what the
/// country has of what it gathers: wood from a woodcutter, stone from a stonecutter. This is after the failures, so a
/// gatherer whose route broke today doesn't count. A country that gathered some of a kind is in the newsreel.
fn gather(day: &mut DaySave) {
    let cut_off = day.cut_off_gatherers();
    for country in day.countries.iter().map(|c| c.id).collect::<Vec<_>>() {
        for kind in GathererKind::ALL {
            let operating = day.gatherers.iter().filter(|g| g.country == country && g.kind == kind && !cut_off.contains(&g.id)).count() as u64;
            let amount = operating * kind.daily_yield();
            if amount > 0 && day.add_resource(country, kind.resource(), amount as i64).is_ok() {
                day.newsreel.push(News::about(country, Event::Gathered { amount, kind }));
            }
        }
    }
}

/// Each militia has all its movement points again.
fn militia_rest(day: &mut DaySave) {
    day.rest_militia();
}

/// Each militia under siege loses members (see [`DaySave::shrink_besieged_militia`]). This comes before the militia are paid for, so
/// that a country pays only for the members that are left.
fn militia_siege(day: &mut DaySave) {
    day.shrink_besieged_militia(ids::random_unit);
}

/// Each country pays to keep the militia it picked to be paid (see [`DaySave::pay_militia_round`]). This comes after the income,
/// so that a country pays out of what it earned today.
fn militia_upkeep_first(day: &mut DaySave) {
    day.pay_militia_round(Round::First);
}

/// Each country pays to keep the rest of its militia, out of what it has left after the units picked to be paid.
fn militia_upkeep(day: &mut DaySave) {
    day.pay_militia_round(Round::Rest);
}

/// Each ship has all its movement points again.
fn people_rest(day: &mut DaySave) {
    day.rest_people();
}

/// Each person is a day older, and some of the older ones die (see [`DaySave::age_people`]).
fn people_age(day: &mut DaySave) {
    day.age_people(ids::random_unit);
}

fn ships_rest(day: &mut DaySave) {
    day.rest_ships();
}

/// Each country pays to keep the ships it picked to be paid (see [`DaySave::pay_ships_round`]), after the militia picked.
fn ships_upkeep_first(day: &mut DaySave) {
    day.pay_ships_round(Round::First);
}

/// Each country pays to keep the rest of its ships, last of all.
fn ships_upkeep(day: &mut DaySave) {
    day.pay_ships_round(Round::Rest);
}

/// Each path, road and bridge that hasn't failed already has a chance of failing: `PATH_FAILURE`, 0.1% and 0.05%. What fails is
/// marked as failed, which the alerts report, and it stays so until it is built again.
fn failures(day: &mut DaySave) {
    fail_things(day, ids::random_unit);
}

/// [`failures`], with `unit` giving the random numbers (each from 0 up to 1, one for each thing that could fail), so
/// that tests can choose them. A thing that has failed already doesn't use one.
fn fail_things(day: &mut DaySave, mut unit: impl FnMut() -> f32) {
    for thing in day.things() {
        if !day.is_failed(thing) && unit() < thing.chance() {
            day.fail(thing);
        }
    }
}

/// Each city's population changes by a random amount: it grows by 1 to 3 percent, or, if the city is under
/// siege, shrinks by 5 to 10 percent, and then stays between the least and the most a city can have. A city
/// whose population changed is in the newsreel.
fn population(day: &mut DaySave) {
    change_populations(day, ids::random_unit);
}

/// [`population`], with `unit` giving the random numbers (each from 0 up to 1, one for each city), so that
/// tests can choose them.
fn change_populations(day: &mut DaySave, mut unit: impl FnMut() -> f32) {
    for city in &mut day.cities {
        let from = city.population;
        city.population = next_population(from, city.under_siege, unit());
        if city.population != from {
            day.newsreel.push(News::about(city.id, Event::PopulationChanged { from, to: city.population, under_siege: city.under_siege }));
        }
    }
}

/// What a city of `population` people has the next day: it changes by a percent that is `unit` (0 up to 1) of the
/// way through the range for its situation, and the result is kept between the least and the most a city can
/// have. A city under siege loses people, and any other gains them.
fn next_population(population: u32, under_siege: bool, unit: f32) -> u32 {
    let unit = f64::from(unit.clamp(0.0, 1.0));
    let factor = if under_siege {
        1.0 - (SIEGE_LOSS_PERCENT.0 + (SIEGE_LOSS_PERCENT.1 - SIEGE_LOSS_PERCENT.0) * unit) / 100.0
    } else {
        1.0 + (GROWTH_PERCENT.0 + (GROWTH_PERCENT.1 - GROWTH_PERCENT.0) * unit) / 100.0
    };
    let changed = (f64::from(population) * factor).round();
    (changed as u32).clamp(MIN_POPULATION, MAX_POPULATION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatherer::GathererKind;
    use crate::calendar::Date;
    use crate::bridge::Spot;
    use crate::failure::{Thing, PATH_FAILURE};
    use crate::path::{Kind, PathSegment};

    #[test]
    fn a_city_that_is_free_grows_by_one_to_three_percent() {
        assert_eq!(next_population(1_000, false, 0.0), 1_010, "the least");
        assert_eq!(next_population(1_000, false, 0.5), 1_020, "in the middle");
        assert_eq!(next_population(1_000, false, 1.0), 1_030, "the most");
        for n in 0..1_000 {
            let grown = next_population(5_000, false, n as f32 / 1_000.0);
            assert!((5_050..=5_150).contains(&grown), "{grown}");
        }
    }

    #[test]
    fn a_city_under_siege_shrinks_by_five_to_ten_percent() {
        assert_eq!(next_population(1_000, true, 0.0), 950, "the least");
        assert_eq!(next_population(1_000, true, 0.5), 925, "in the middle");
        assert_eq!(next_population(1_000, true, 1.0), 900, "the most");
        for n in 0..1_000 {
            let shrunk = next_population(5_000, true, n as f32 / 1_000.0);
            assert!((4_500..=4_750).contains(&shrunk), "{shrunk}");
        }
    }

    #[test]
    fn a_citys_population_stays_between_100_and_100_000() {
        // Under siege at the least a city can have, it stays there.
        assert_eq!(next_population(100, true, 0.7), 100);
        assert_eq!(next_population(105, true, 1.0), 100, "it can't fall below it");
        // As big as it can be, it stays there.
        assert_eq!(next_population(100_000, false, 0.7), 100_000);
        assert_eq!(next_population(99_000, false, 1.0), 100_000, "it can't pass it");
        // A city that somehow has more or fewer than the limits is brought back to them.
        assert_eq!(next_population(250_000, false, 0.5), 100_000);
        assert_eq!(next_population(30, false, 0.5), 100);
        // Both are loosened by a siege being lifted or put on: a city at the top shrinks if besieged.
        assert_eq!(next_population(100_000, true, 0.0), 95_000);
        assert!(next_population(100, false, 0.0) > 100, "a city at the bottom grows if free");
    }

    #[test]
    fn a_random_number_outside_the_range_is_taken_as_its_nearest_end() {
        assert_eq!(next_population(1_000, false, 7.0), 1_030);
        assert_eq!(next_population(1_000, false, -3.0), 1_010);
    }

    #[test]
    fn each_city_has_its_own_random_number_and_its_own_siege() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        for (k, name) in ["Port Aldor", "Highmark", "Lowmoor"].into_iter().enumerate() {
            day.found_city(name, k as f32 * 100.0, 0.0, None).unwrap();
            day.cities[k].population = 1_000;
        }
        day.set_city_under_siege(day.cities[1].id, true).unwrap();

        // The numbers are used one for each city, in order: 0 for the first, 1 for the second, 0.5 for the third.
        let mut numbers = [0.0, 1.0, 0.5].into_iter();
        change_populations(&mut day, || numbers.next().unwrap());
        assert_eq!(day.cities.iter().map(|c| c.population).collect::<Vec<_>>(), [1_010, 900, 1_020]);
    }

    /// A day with a path, a road and a bridge, and a country with the money for them.
    fn day_with_one_of_each() -> DaySave {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, 5_000).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(1, 2)).unwrap();
        day.build_segment(Kind::Road, PathSegment::to_edge(3, 4)).unwrap();
        day.stock_for_bridges();
        day.build_bridge(Spot { x: -30.0, z: 0.0 }, Spot { x: 30.0, z: 0.0 }).unwrap();
        day
    }

    #[test]
    fn a_thing_fails_if_the_number_drawn_is_below_its_chance() {
        let (path, road) = (Thing::Path(PathSegment::to_edge(1, 2)), Thing::Road(PathSegment::to_edge(3, 4)));
        let fails_with = |number: f32| {
            let mut day = day_with_one_of_each();
            let bridge = Thing::Bridge(day.bridges[0].id);
            fail_things(&mut day, || number);
            (day.is_failed(path), day.is_failed(road), day.is_failed(bridge))
        };
        assert_eq!(fails_with(0.5), (false, false, false), "nothing fails on most draws");
        assert_eq!(fails_with(PATH_FAILURE * 0.99), (true, false, false), "just under the chance of a path: only a path");
        assert_eq!(fails_with(PATH_FAILURE * 1.01), (false, false, false), "just over it");
        assert_eq!(fails_with(0.0009), (true, true, false), "under 0.1%: a path and a road");
        assert_eq!(fails_with(0.0004), (true, true, true), "under 0.05%: all three");
        assert_eq!(fails_with(0.0), (true, true, true));
    }

    #[test]
    fn something_that_has_failed_does_not_fail_again_or_use_up_a_draw() {
        let mut day = day_with_one_of_each();
        day.fail(Thing::Path(PathSegment::to_edge(1, 2)));
        let mut draws = 0;
        fail_things(&mut day, || {
            draws += 1;
            0.5
        });
        assert_eq!(draws, 2, "only the road and the bridge are drawn for");
        assert_eq!(day.failed.len(), 1);
    }

    #[test]
    fn the_failures_come_out_in_the_right_proportions() {
        // A lot of each, so that the chances show: 5,000 paths, 20,000 roads and 20,000 bridges, drawn for once.
        let mut day = day();
        for n in 0..5_000u16 {
            day.paths.push(PathSegment::to_edge(n, n + 1));
        }
        for n in 0..20_000u16 {
            day.roads.push(PathSegment::to_edge(n, n + 1));
        }
        for _ in 0..20_000 {
            day.bridges.push(crate::bridge::Bridge { id: crate::ids::BridgeId::new(), first: Spot { x: 0.0, z: 0.0 }, second: Spot { x: 1.0, z: 1.0 }, regions: [None; 2] });
        }
        failures(&mut day);
        let count = |f: fn(&Thing) -> bool| day.failed.iter().filter(|t| f(t)).count();
        let (paths, roads, bridges) = (count(|t| matches!(t, Thing::Path(_))), count(|t| matches!(t, Thing::Road(_))), count(|t| matches!(t, Thing::Bridge(_))));
        let (expected, spread) = (5_000.0 * PATH_FAILURE, (5_000.0 * PATH_FAILURE * (1.0 - PATH_FAILURE)).sqrt());
        assert!((paths as f32 - expected).abs() < 6.0 * spread + 5.0, "about {expected} of 5,000 paths: {paths}");
        assert!((3..=45).contains(&roads), "about 20 of 20,000 roads: {roads}");
        assert!(bridges <= 40, "about 10 of 20,000 bridges: {bridges}");
    }

    #[test]
    fn a_failed_thing_is_mended_by_building_it_again_for_the_usual_cost() {
        let mut day = day_with_one_of_each();
        let (path, road) = (PathSegment::to_edge(1, 2), PathSegment::to_edge(3, 4));
        let bridge = day.bridges[0].id;
        day.fail(Thing::Path(path));
        day.fail(Thing::Road(road));
        day.fail(Thing::Bridge(bridge));
        let money = day.countries[0].money;

        day.build_segment(Kind::Path, path).unwrap();
        assert_eq!(day.countries[0].money, money - 10);
        day.build_segment(Kind::Road, road).unwrap();
        assert_eq!(day.countries[0].money, money - 10 - 100);
        day.stock_for_bridges();
        day.build_bridge(Spot { x: 31.0, z: 1.0 }, Spot { x: -29.0, z: 0.0 }).unwrap();
        assert_eq!(day.countries[0].money, money - 10 - 100 - 1_000);
        assert!(day.failed.is_empty(), "all three are mended");
        assert_eq!((day.paths.len(), day.roads.len(), day.bridges.len()), (1, 1, 1), "mended, not built a second time");
        // One that hasn't failed can't be built again.
        assert!(day.build_segment(Kind::Path, path).unwrap_err().contains("already"));
    }

    #[test]
    fn paving_a_failed_path_leaves_nothing_of_it_in_the_failed_list() {
        let mut day = day_with_one_of_each();
        let path = PathSegment::to_edge(1, 2);
        day.fail(Thing::Path(path));
        day.build_segment(Kind::Road, path).unwrap();
        assert!(day.paths.is_empty() && day.failed.is_empty());
        assert_eq!(day.roads.len(), 2);
    }

    #[test]
    fn a_failed_road_cannot_be_replaced_by_a_path() {
        let mut day = day_with_one_of_each();
        let road = PathSegment::to_edge(3, 4);
        day.fail(Thing::Road(road));
        assert!(day.build_segment(Kind::Path, road).unwrap_err().contains("already a road"));
    }

    #[test]
    fn the_real_rule_changes_populations_within_the_ranges() {
        for _ in 0..50 {
            let mut day = day();
            day.found_country("Aldoria").unwrap();
            day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
            day.found_city("Highmark", 100.0, 0.0, None).unwrap();
            day.cities[0].population = 10_000;
            day.cities[1].population = 10_000;
            day.set_city_under_siege(day.cities[1].id, true).unwrap();
            population(&mut day);
            assert!((10_100..=10_300).contains(&day.cities[0].population), "{}", day.cities[0].population);
            assert!((9_000..=9_500).contains(&day.cities[1].population), "{}", day.cities[1].population);
        }
    }

    #[test]
    fn income_comes_from_the_populations_before_they_change() {
        // Income is the first rule and population the second, so a country earns on the people it had.
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 1_000;
        run(&mut day);
        assert_eq!(day.country(id).unwrap().money, 1_000);
        assert!((1_010..=1_030).contains(&day.cities[0].population));
    }

    fn day() -> DaySave {
        DaySave::new(Date::FIRST, "Assets/map.nworld")
    }

    #[test]
    fn a_country_earns_a_coin_for_each_person_in_its_cities() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_country("Cardell").unwrap();
        let (aldoria, brevon, cardell) = (day.countries[0].id, day.countries[1].id, day.countries[2].id);
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.found_city("Highmark", 100.0, 0.0, None).unwrap();
        day.cities[0].population = 1_200;
        day.cities[1].population = 340;
        day.set_city_country(day.cities[1].id, brevon).unwrap();
        day.found_city("Lowmoor", 200.0, 0.0, None).unwrap();
        day.cities[2].population = 60;

        income(&mut day);
        // Aldoria has Port Aldor and Lowmoor (the player's country gets new cities), Brevon has Highmark.
        assert_eq!(day.country(aldoria).unwrap().money, 1_200 + 60);
        assert_eq!(day.country(brevon).unwrap().money, 340);
        assert_eq!(day.country(cardell).unwrap().money, 0, "a country with no cities earns nothing");
    }

    #[test]
    fn income_is_added_to_the_money_a_country_has() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.add_money(id, 500).unwrap();
        income(&mut day);
        assert_eq!(day.country(id).unwrap().money, 500 + 100);
        income(&mut day);
        assert_eq!(day.country(id).unwrap().money, 500 + 200);
    }

    #[test]
    fn a_country_that_earns_is_in_the_newsreel_and_one_that_does_not_is_not() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 1_200;
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.newsreel.clear();

        income(&mut day);
        assert_eq!(day.newsreel, [News::about(aldoria, Event::Income { amount: 1_200 })]);
        assert!(day.newsreel.iter().all(|n| n.subject != brevon.into()), "Brevon earned nothing, so there is no news of it");
    }

    #[test]
    fn a_city_whose_population_changed_is_in_the_newsreel_with_how() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        for (k, name) in ["Port Aldor", "Highmark", "Lowmoor"].into_iter().enumerate() {
            day.found_city(name, k as f32 * 100.0, 0.0, None).unwrap();
        }
        day.cities[0].population = 1_000;
        day.cities[1].population = 1_000;
        day.cities[2].population = 100; // As few as there can be, and under siege: no change.
        day.set_city_under_siege(day.cities[1].id, true).unwrap();
        day.set_city_under_siege(day.cities[2].id, true).unwrap();
        let ids: Vec<_> = day.cities.iter().map(|c| c.id).collect();
        day.newsreel.clear();

        change_populations(&mut day, || 0.0);
        assert_eq!(
            day.newsreel,
            [
                News::about(ids[0], Event::PopulationChanged { from: 1_000, to: 1_010, under_siege: false }),
                News::about(ids[1], Event::PopulationChanged { from: 1_000, to: 950, under_siege: true }),
            ],
            "the city that did not change is not in it"
        );
    }

    #[test]
    fn income_cannot_overflow_the_money() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.add_money(id, i64::MAX - 10).unwrap();
        income(&mut day);
        assert_eq!(day.country(id).unwrap().money, i64::MAX);
    }

    #[test]
    fn running_the_rules_changes_only_money_populations_peoples_ages_and_the_newsreel() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4005)).unwrap();
        let mut ran = day.clone();
        run(&mut ran);
        ran.countries[0].money = day.countries[0].money;
        ran.cities[0].population = day.cities[0].population;
        assert_eq!(ran.people[0].age_days, day.people[0].age_days + 1, "a day older");
        ran.people[0].age_days = day.people[0].age_days;
        // The news is added to, and what was in it is as it was.
        assert_eq!(ran.newsreel.len(), day.newsreel.len() + 2, "income and population");
        ran.newsreel.truncate(day.newsreel.len());
        assert_eq!(ran, day);
    }

    #[test]
    fn each_gatherer_with_a_route_to_a_city_provides_100_wood_to_its_country_and_one_without_provides_none() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        for id in [aldoria, brevon] {
            day.add_money(id, 5_000).unwrap();
        }
        // Aldoria has a city and a gatherer in the same region, and another gatherer in a region with no route.
        day.found_city("Port Aldor", 0.0, 0.0, Some(0xC001)).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 10.0, 0.0, 0xC001).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 50.0, 0.0, 0xC002).unwrap();
        // Brevon has a gatherer but no city at all.
        day.select_country(brevon).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 90.0, 0.0, 0xC003).unwrap();
        day.select_country(aldoria).unwrap();

        gather(&mut day);
        assert_eq!(day.country(aldoria).unwrap().stock.wood, 100, "one woodcutter that operates");
        assert_eq!(day.country(brevon).unwrap().stock.wood, 0, "none that does");
        let news: Vec<String> = day.newsreel.iter().filter(|n| matches!(n.event, Event::Gathered { .. })).map(|n| n.headline(&day)).collect();
        assert_eq!(news, ["Aldoria gathers 100 wood from its woodcutters"], "and nothing is said of a country that got none");

        // A route to the city makes the second operate, and the wood adds up from day to day.
        day.build_segment(Kind::Path, PathSegment::to_edge(0xC002, 0xC001)).unwrap();
        gather(&mut day);
        assert_eq!(day.country(aldoria).unwrap().stock.wood, 300);
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria gathers 200 wood from its woodcutters");

        // A route that broke today, as the failures ran, is no route: the gatherer provides nothing.
        day.fail(Thing::Path(PathSegment::to_edge(0xC002, 0xC001)));
        gather(&mut day);
        assert_eq!(day.country(aldoria).unwrap().stock.wood, 400);
    }

    #[test]
    fn a_stonecutter_provides_100_stone_and_each_kind_is_counted_and_reported_apart() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 9_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(0xC001)).unwrap();
        // A woodcutter and a stonecutter in the region of the city, and a stonecutter in another with no route.
        day.build_gatherer_claiming(GathererKind::Woodcutter, 10.0, 0.0, 0xC001).unwrap();
        day.build_gatherer_claiming(GathererKind::Stonecutter, 20.0, 0.0, 0xC001).unwrap();
        day.build_gatherer_claiming(GathererKind::Stonecutter, 60.0, 0.0, 0xC002).unwrap();

        gather(&mut day);
        let stock = day.country(aldoria).unwrap().stock;
        assert_eq!((stock.wood, stock.stone), (100, 100), "one of each operates");
        let news: Vec<String> = day.newsreel.iter().filter(|n| matches!(n.event, Event::Gathered { .. })).map(|n| n.headline(&day)).collect();
        assert_eq!(news, ["Aldoria gathers 100 wood from its woodcutters", "Aldoria gathers 100 stone from its stonecutters"]);

        // A route for the other stonecutter doubles the stone, and the wood is as it was.
        day.build_segment(Kind::Path, PathSegment::to_edge(0xC002, 0xC001)).unwrap();
        gather(&mut day);
        let stock = day.country(aldoria).unwrap().stock;
        assert_eq!((stock.wood, stock.stone), (200, 300));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria gathers 200 stone from its stonecutters");
        assert!(day.country(aldoria).unwrap().stock.iron == 0 && day.country(aldoria).unwrap().stock.oil == 0, "nothing else is touched");
    }

    #[test]
    fn a_sawmill_provides_1000_wood_and_a_quarry_1000_stone_and_the_news_says_what_gathered_it() {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 100_000).unwrap();
        day.add_resource(aldoria, nworld_map::world::Resource::Forest, 3_000).unwrap();
        day.add_resource(aldoria, nworld_map::world::Resource::Stone, 3_000).unwrap();
        // One of each kind, each in the region of a city, since a region has one gatherer of each resource.
        for (n, kind) in [GathererKind::Woodcutter, GathererKind::Sawmill, GathererKind::Stonecutter, GathererKind::Quarry].into_iter().enumerate() {
            let region = 0xC001 + n as u16 * 2;
            day.found_city(&format!("City {n}"), n as f32 * 50.0, 0.0, Some(region)).unwrap();
            day.build_gatherer_claiming(kind, n as f32 * 50.0 + 5.0, 0.0, region).unwrap();
        }
        let before = day.country(aldoria).unwrap().stock;
        gather(&mut day);
        let stock = day.country(aldoria).unwrap().stock;
        assert_eq!((stock.wood - before.wood, stock.stone - before.stone), (1_100, 1_100), "100 and 1,000 of each");
        let news: Vec<String> = day.newsreel.iter().filter(|n| matches!(n.event, Event::Gathered { .. })).map(|n| n.headline(&day)).collect();
        assert_eq!(
            news,
            [
                "Aldoria gathers 100 wood from its woodcutters",
                "Aldoria gathers 1,000 wood from its sawmills",
                "Aldoria gathers 100 stone from its stonecutters",
                "Aldoria gathers 1,000 stone from its quarries",
            ]
        );
    }

    #[test]
    fn the_ships_get_their_points_back_and_are_paid_for_each_day_and_the_unpaid_are_abandoned() {
        let rest_at = DAY_RULES.iter().position(|r| r.description.contains("Each ship gets all of its movement points back")).unwrap();
        let ships_at = DAY_RULES.iter().position(|r| r.description.contains("for the rest of its ships")).unwrap();
        assert!(rest_at < ships_at);

        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 20_000).unwrap();
        for resource in nworld_map::world::Resource::ALL {
            day.add_resource(aldoria, resource, 2_000).unwrap();
        }
        day.build_shipyard_claiming(crate::shipyard::ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        let yard = day.shipyards[0].id;
        day.build_ship(yard, crate::ship::ShipKind::Small, macroquad::math::vec2(40.0, 0.0)).unwrap();
        let id = day.ships[0].id;
        day.move_ship(id, macroquad::math::vec2(60.0, 0.0), 150, None).unwrap();
        let money = day.country(aldoria).unwrap().money;

        // A day on: it is rested, and a day's upkeep of 50 coins is paid.
        let mut tomorrow = day.following(day.date.next());
        run(&mut tomorrow);
        assert_eq!(tomorrow.ship(id).unwrap().points, 200);
        assert_eq!(tomorrow.country(aldoria).unwrap().money, money - 50);
        assert!(tomorrow.newsreel.iter().any(|n| matches!(n.event, Event::ShipPaid { amount: 50 })));

        // With no money at all, and no cities to earn any, it is abandoned.
        let mut broke = day.clone();
        broke.add_money(aldoria, -money).unwrap();
        let mut tomorrow = broke.following(broke.date.next());
        run(&mut tomorrow);
        assert!(tomorrow.ships.is_empty());
        assert!(tomorrow.newsreel.iter().any(|n| matches!(n.event, Event::ShipDeserted { .. })));
    }

    #[test]
    fn a_ship_marked_to_be_paid_first_is_kept_before_militia_that_are_not_when_there_is_too_little_money() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 30_000).unwrap();
        for resource in nworld_map::world::Resource::ALL {
            day.add_resource(aldoria, resource, 2_000).unwrap();
        }
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 2_000;
        day.raise_militia(day.cities[0].id, 100, macroquad::math::vec2(40.0, 0.0)).unwrap();
        day.build_shipyard_claiming(crate::shipyard::ShipyardKind::Small, 0.0, 0.0, Some(0x4005)).unwrap();
        let yard = day.shipyards[0].id;
        day.build_ship(yard, crate::ship::ShipKind::Small, macroquad::math::vec2(60.0, 0.0)).unwrap();
        let (militia, ship) = (day.militias[0].id, day.ships[0].id);
        let money = day.country(aldoria).unwrap().money;
        day.add_money(aldoria, 70 - money).unwrap();

        let pay = |day: &mut DaySave| {
            militia_upkeep_first(day);
            ships_upkeep_first(day);
            militia_upkeep(day);
            ships_upkeep(day);
        };

        // Nothing marked: the militia are paid for first, so 70 coins keep 70 members and the ship, which costs 50, is lost.
        let mut unmarked = day.clone();
        pay(&mut unmarked);
        assert_eq!((unmarked.militia(militia).unwrap().members, unmarked.ships.len()), (70, 0));

        // The ship marked: it is paid for, 50 coins, and the militia get the 20 that are left.
        day.set_ship_pay_first(ship, true).unwrap();
        pay(&mut day);
        assert_eq!((day.militia(militia).unwrap().members, day.ships.len()), (20, 1));
        assert_eq!(day.country(aldoria).unwrap().money, 0);
    }

    #[test]
    fn the_wood_comes_after_the_failures_in_the_rules_and_a_day_with_no_gatherers_gathers_none() {
        let wood_at = DAY_RULES.iter().position(|r| r.description.contains("provides its wood or stone")).unwrap();
        let failures_at = DAY_RULES.iter().position(|r| r.description.contains("chance of failing")).unwrap();
        assert!(wood_at > failures_at);
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let before = day.clone();
        gather(&mut day);
        assert_eq!(day, before);
    }

    #[test]
    fn the_militia_are_paid_for_after_the_income_and_before_the_ships_out_of_what_the_country_earned_that_day() {
        let paid_at = DAY_RULES.iter().position(|r| r.description.contains("the rest of its militia")).unwrap();
        let ships_at = DAY_RULES.iter().position(|r| r.description.contains("for the rest of its ships")).unwrap();
        assert_eq!(ships_at, DAY_RULES.len() - 1, "the ships are paid for last");
        assert!(paid_at < ships_at);

        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 3_000).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 2_000;
        day.raise_militia(day.cities[0].id, 200, macroquad::math::vec2(40.0, 0.0)).unwrap();
        assert_eq!(day.country(aldoria).unwrap().money, 1_000, "200 members at 10 coins each");

        let mut tomorrow = day.following(day.date.next());
        run(&mut tomorrow);
        // Income on the 1,800 left in the city, less 200 for the militia.
        assert_eq!(tomorrow.country(aldoria).unwrap().money, 1_000 + 1_800 - 200);
        assert_eq!(tomorrow.militias[0].members, 200, "all of them paid for");
        assert!(tomorrow.newsreel.iter().any(|n| matches!(n.event, Event::MilitiaPaid { amount: 200 })));
    }

    #[test]
    fn a_country_that_earns_nothing_and_has_nothing_loses_its_militia_on_the_day_switch() {
        let mut day = day();
        day.found_country("Aldoria").unwrap();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 500).unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day.cities[0].population = 500;
        day.raise_militia(day.cities[0].id, 50, macroquad::math::vec2(40.0, 0.0)).unwrap();
        // The city goes to another country, so Aldoria earns nothing and has no money left for the militia.
        day.found_country("Brevon").unwrap();
        day.set_city_country(day.cities[0].id, day.countries[1].id).unwrap();
        day.add_money(aldoria, -day.country(aldoria).unwrap().money).unwrap();

        let mut tomorrow = day.following(day.date.next());
        run(&mut tomorrow);
        assert!(tomorrow.militias.is_empty());
        assert_eq!(tomorrow.country(aldoria).unwrap().money, 0);
        assert!(tomorrow.cities[0].population < 500, "the 50 who deserted are not in it: {}", tomorrow.cities[0].population);
    }

    #[test]
    fn every_rule_says_what_it_does() {
        assert!(!DAY_RULES.is_empty());
        assert!(DAY_RULES.iter().all(|rule| !rule.description.trim().is_empty()));
    }
}
