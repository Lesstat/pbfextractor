/*
 Pbfextractor creates graph files for the cycle-routing projects from pbf and srtm data
 Copyright (C) 2018  Florian Barth

 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.

 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU General Public License for more details.

 You should have received a copy of the GNU General Public License
 along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
use super::pbf::{MetricIndices, Node};
use osmpbfreader::Tags;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub enum MetricError {
    UnknownMetric,
    NonFiniteTime(f64, f64),
}

pub type MetricResult = Result<f64, MetricError>;

pub trait Metric {
    fn name(&self) -> String;
}

macro_rules! metric {
    ($t:ty) => {
        impl Metric for $t {
            fn name(&self) -> String {
                stringify!($t).to_owned()
            }
        }
    };
}

pub trait TagMetric: Metric {
    fn calc(&self, tags: &Tags) -> MetricResult;
}

pub trait NodeMetric: Metric {
    fn calc(&self, source: &Node, target: &Node) -> MetricResult;
}

pub trait CostMetric: Metric {
    fn calc(&self, costs: &[f64], map: &MetricIndices) -> MetricResult;
}

fn bounded_speed(tags: &Tags, driver_max: f64) -> MetricResult {
    let street_type = tags.get("highway").map(String::as_ref);
    let tag_speed = match street_type {
        Some("motorway") | Some("trunk") => driver_max,
        Some("primary") => 100.0,
        Some("secondary") | Some("trunk_link") => 80.0,
        Some("motorway_link")
        | Some("primary_link")
        | Some("secondary_link")
        | Some("tertiary")
        | Some("tertiary_link") => 70.0,
        Some("service") => 30.0,
        Some("living_street") => 5.0,
        _ => 50.0,
    };

    let max_speed_tag = tags.get("maxspeed");
    let max_speed = match max_speed_tag.map(String::as_ref) {
        Some("none") => Some(driver_max),
        Some("walk") | Some("DE:walk") => Some(10.0),
        Some("living_street") | Some("DE:living_street") => Some(10.0),
        Some(s) => s.parse().ok(),
        None => None,
    };

    let speed = match max_speed {
        Some(s) if s > 0.0 && s <= driver_max => s,
        _ => tag_speed.min(driver_max),
    };
    Ok(speed)
}

#[allow(dead_code)]
pub struct CarSpeed;
metric!(CarSpeed);
impl TagMetric for CarSpeed {
    fn calc(&self, tags: &Tags) -> MetricResult {
        bounded_speed(&tags, 120.0)
    }
}

#[allow(dead_code)]
pub struct TruckSpeed;
metric!(TruckSpeed);
impl TagMetric for TruckSpeed {
    fn calc(&self, tags: &Tags) -> MetricResult {
        bounded_speed(&tags, 80.0)
    }
}

#[allow(dead_code)]
pub struct FastCarSpeed;
metric!(FastCarSpeed);
impl TagMetric for FastCarSpeed {
    fn calc(&self, tags: &Tags) -> MetricResult {
        bounded_speed(&tags, 180.0)
    }
}

#[allow(dead_code)]
pub struct Distance;
metric!(Distance);

impl NodeMetric for Distance {
    fn calc(&self, source: &Node, target: &Node) -> MetricResult {
        const EARTH_RADIUS: f64 = 6_371_007.2;
        let theta1 = source.lat.to_radians();
        let theta2 = target.lat.to_radians();
        let delta_theta = (target.lat - source.lat).to_radians();
        let delta_lambda = (target.long - source.long).to_radians();
        let a = (delta_theta / 2.0).sin().powi(2)
            + theta1.cos() * theta2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        Ok(EARTH_RADIUS * c)
    }
}

#[allow(dead_code)]
pub struct TravelTime<D: Metric, S: Metric> {
    distance: Rc<D>,
    speed: Rc<S>,
}

impl<D, S> Metric for TravelTime<D, S>
where
    D: Metric,
    S: Metric,
{
    fn name(&self) -> String {
        format!(
            "TravelTime: {} / {}",
            self.distance.name(),
            self.speed.name()
        )
    }
}

impl<D, S> TravelTime<D, S>
where
    D: Metric,
    S: Metric,
{
    pub fn new(distance: Rc<D>, speed: Rc<S>) -> TravelTime<D, S> {
        TravelTime { distance, speed }
    }
}

impl<D, S> CostMetric for TravelTime<D, S>
where
    D: Metric,
    S: Metric,
{
    fn calc(&self, costs: &[f64], map: &MetricIndices) -> MetricResult {
        let dist_index = map
            .get(&self.distance.name())
            .ok_or(MetricError::UnknownMetric)?;
        let speed_index = map
            .get(&self.speed.name())
            .ok_or(MetricError::UnknownMetric)?;
        let dist = costs[*dist_index];
        let speed = costs[*speed_index];
        let time = dist * 360.0 / speed;
        if time.is_finite() {
            Ok(time)
        } else {
            Err(MetricError::NonFiniteTime(dist, speed))
        }
    }
}

#[allow(dead_code)]
pub struct HeightAscent;
metric!(HeightAscent);

impl NodeMetric for HeightAscent {
    fn calc(&self, source: &Node, target: &Node) -> MetricResult {
        let height_diff = target.height - source.height;
        if height_diff > 0.0 {
            Ok(height_diff)
        } else {
            Ok(0.0)
        }
    }
}

#[allow(dead_code)]
pub struct BicycleUnsuitability;
metric!(BicycleUnsuitability);

impl TagMetric for BicycleUnsuitability {
    fn calc(&self, tags: &Tags) -> MetricResult {
        let bicycle_tag = tags.get("bicycle");
        if tags.get("cycleway").is_some()
            || bicycle_tag.is_some() && bicycle_tag != Some(&"no".to_string())
        {
            return Ok(0.5);
        }

        let side_walk: Option<&str> = tags.get("sidewalk").map(String::as_ref);
        if side_walk == Some("yes") {
            return Ok(1.0);
        }

        let street_type = tags.get("highway").map(String::as_ref);
        let unsuitability = match street_type {
            Some("primary") => 5.0,
            Some("primary_link") => 5.0,
            Some("secondary") => 4.0,
            Some("secondary_link") => 4.0,
            Some("tertiary") => 3.0,
            Some("tertiary_link") => 3.0,
            Some("road") => 3.0,
            Some("bridleway") => 3.0,
            Some("unclassified") => 2.0,
            Some("residential") => 2.0,
            Some("traffic_island") => 2.0,
            Some("living_street") => 1.0,
            Some("service") => 1.0,
            Some("track") => 1.0,
            Some("platform") => 1.0,
            Some("pedestrian") => 1.0,
            Some("path") => 1.0,
            Some("footway") => 1.0,
            Some("cycleway") => 0.5,
            _ => 6.0,
        };
        Ok(unsuitability)
    }
}

#[allow(dead_code)]
pub struct EdgeCount;
metric!(EdgeCount);

impl TagMetric for EdgeCount {
    fn calc(&self, _: &Tags) -> MetricResult {
        Ok(1.0)
    }
}
#[derive(Debug)]
pub struct Grid {
    lat_min: f64,
    lat_max: f64,
    lng_min: f64,
    lng_max: f64,
    side_length: u32,
}

pub struct Coord {
    pub x: u32,
    pub y: u32,
}

impl Grid {
    pub fn new_ptr() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            lat_min: 90.0,
            lat_max: -90.0,
            lng_min: 180.0,
            lng_max: -180.0,
            side_length: 20,
        }))
    }
    pub fn add(&mut self, n: &Node) {
        self.lat_min = n.lat.min(self.lat_min);
        self.lat_max = n.lat.max(self.lat_max);
        self.lng_min = n.long.min(self.lng_min);
        self.lng_max = n.long.max(self.lng_max);
    }
    pub fn index(&self, n: &Node) -> Coord {
        let x_len = (self.lng_max - self.lng_min) / Into::<f64>::into(self.side_length);
        let x = (n.long - self.lng_min) / x_len;
        let y_len = (self.lat_max - self.lat_min) / Into::<f64>::into(self.side_length);
        let y = (n.lat - self.lat_min) / y_len;

        Coord {
            x: (x.ceil() - 1.0) as u32,
            y: (y.ceil() - 1.0) as u32,
        }
    }
}

pub struct GridX(pub Rc<RefCell<Grid>>);
metric!(GridX);
impl NodeMetric for GridX {
    fn calc(&self, a: &Node, _: &Node) -> MetricResult {
        if self.0.borrow().index(a).x % 2 == 0 {
            Ok(20.0)
        } else {
            Ok(1.0)
        }
    }
}

pub struct GridY(pub Rc<RefCell<Grid>>);
metric!(GridY);
impl NodeMetric for GridY {
    fn calc(&self, a: &Node, _: &Node) -> MetricResult {
        if self.0.borrow().index(a).y % 2 == 0 {
            Ok(20.0)
        } else {
            Ok(1.0)
        }
    }
}

pub struct ChessBoard(pub Rc<RefCell<Grid>>);
metric!(ChessBoard);
impl NodeMetric for ChessBoard {
    fn calc(&self, a: &Node, _: &Node) -> MetricResult {
        let c = self.0.borrow().index(a);
        if c.y % 2 == 0 && c.x % 2 == 0 {
            Ok(20.0)
        } else {
            Ok(1.0)
        }
    }
}

pub trait EdgeFilter {
    fn is_invalid(&self, tags: &Tags) -> bool;
}

#[allow(dead_code)]
pub struct BicycleEdgeFilter;

impl EdgeFilter for BicycleEdgeFilter {
    fn is_invalid(&self, tags: &Tags) -> bool {
        let bicycle_tag = tags.get("bicycle");
        if bicycle_tag == Some(&"no".to_string()) {
            return true;
        }
        if tags.get("cycleway").is_some()
            || bicycle_tag.is_some() && bicycle_tag != Some(&"no".to_string())
        {
            return false;
        }

        let street_type = tags.get("highway").map(String::as_ref);
        let side_walk: Option<&str> = tags.get("sidewalk").map(String::as_ref);
        let has_side_walk: bool = match side_walk {
            Some(s) => s != "no",
            None => false,
        };
        if has_side_walk {
            return false;
        }
        match street_type {
            Some("motorway")
            | Some("motorway_link")
            | Some("trunk")
            | Some("trunk_link")
            | Some("proposed")
            | Some("steps")
            | Some("elevator")
            | Some("corridor")
            | Some("raceway")
            | Some("rest_area")
            | Some("construction")
            | None => true,
            _ => false,
        }
    }
}
#[allow(dead_code)]
pub struct CarEdgeFilter;

impl EdgeFilter for CarEdgeFilter {
    fn is_invalid(&self, tags: &Tags) -> bool {
        let street_type = tags.get("highway").map(String::as_ref);
        match street_type {
            Some("footway") | Some("bridleway") | Some("steps") | Some("path")
            | Some("cycleway") | Some("track") | Some("proposed") | Some("construction")
            | Some("pedestrian") | Some("rest_area") | Some("elevator") | Some("raceway")
            | None => true,
            _ => false,
        }
    }
}

#[test]
fn test_index() {
    let g = Grid {
        lng_min: 5.0,
        lng_max: 20.0,
        lat_min: 7.0,
        lat_max: 30.0,
        side_length: 20,
    };

    let c = g.index(&Node::new(1, 12.7, 7.3, 0.0));

    assert_eq!(3, c.x);
    assert_eq!(4, c.y);

    let c = g.index(&Node::new(1, 7.1, 5.1, 0.0));

    assert_eq!(0, c.x);
    assert_eq!(0, c.y);

    let c = g.index(&Node::new(1, 30.0, 20.00, 0.0));

    assert_eq!(19, c.x);
    assert_eq!(19, c.y);
}

#[test]
fn index_for_negative_coords() {
    let g = Grid {
        lng_min: -10.0,
        lng_max: 10.0,
        lat_min: -20.0,
        lat_max: 20.0,
        side_length: 20,
    };

    let c = g.index(&Node::new(1, 5.2, -3.3, 0.0));

    assert_eq!(6, c.x);
    assert_eq!(12, c.y);
}
