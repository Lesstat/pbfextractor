use super::pbf::{MetricIndices, Node};
use osmpbfreader::Tags;

use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub enum MetricError {
    UnknownMetric,
    NonFiniteTime(f64, f64),
}

pub type MetricResult = Result<f64, MetricError>;

pub trait Metric {
    fn name(&self) -> &'static str;
}

macro_rules! metric {
    ($t:ty) => {
        impl Metric for $t {
            fn name(&self) -> &'static str {
                stringify!($t)
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

impl PartialEq for Metric {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for Metric {}

impl Hash for Metric {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

#[derive(Clone)]
pub struct CarSpeed;
metric!(CarSpeed);

impl TagMetric for CarSpeed {
    fn calc(&self, tags: &Tags) -> MetricResult {
        let max_speed = tags.get("maxspeed").and_then(|s| s.parse().ok());
        let speed = match max_speed {
            Some(s) if s > 0.0 => s,
            _ => {
                let street_type = tags.get("highway").map(String::as_ref);
                match street_type {
                    Some("motorway") | Some("trunk") => 130.0,
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
                }
            }
        };
        Ok(speed)
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct TravelTime;
metric!(TravelTime);

impl CostMetric for TravelTime {
    fn calc(&self, costs: &[f64], map: &MetricIndices) -> MetricResult {
        let dist_index = map.get(Distance.name()).ok_or(MetricError::UnknownMetric)?;
        let speed_index = map.get(CarSpeed.name()).ok_or(MetricError::UnknownMetric)?;
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

#[derive(Clone)]
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

#[derive(Clone)]
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

pub trait EdgeFilter {
    fn is_invalid(&self, tags: &Tags) -> bool;
}

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
