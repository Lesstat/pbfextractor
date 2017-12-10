use osmpbfreader::{OsmObj, OsmPbfReader, Way};

use std::path::Path;
use std::fs::File;
use std::time::Instant;


/// Loads the graph from a pbf file.
///
/// All edges and nodes that contain a highway tag and are accessible
/// for either cars or pedestrians by the judgement of is_not_for_cars
/// and is_not_for_pedestrians
pub fn load_graph<P: AsRef<Path>>(p: P) -> (Vec<NodeInfo>, Vec<EdgeInfo>) {
    let fs = File::open(p).unwrap();
    let mut reader = OsmPbfReader::new(fs);
    let start_loading = Instant::now();
    let obj_map = reader
        .get_objs_and_deps(|obj| obj.tags().contains_key("highway"))
        .unwrap();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (_, obj) in obj_map {
        match obj {
            OsmObj::Node(node) => {
                nodes.push(NodeInfo::new(
                    node.id.0 as usize,
                    (node.decimicro_lat as f64) / 10_000_000.0,
                    (node.decimicro_lon as f64) / 10_000_000.0,
                    0,
                ));
            }
            OsmObj::Way(w) => {
                let speed = determine_speed(&w);
                let is_one_way = is_one_way(&w);
                let no_cars = is_not_for_cars(&w);
                let no_pedestrians = is_not_for_pedestrians(&w);
                for (index, node) in w.nodes[0..(w.nodes.len() - 1)].iter().enumerate() {
                    let mut edge = EdgeInfo::new(
                        node.0 as NodeId,
                        w.nodes[index + 1].0 as NodeId,
                        1.1, // calculating length happens inside the graph
                        speed,
                    );
                    if no_cars && no_pedestrians {
                        continue;
                    }
                    if no_cars {
                        edge.not_for_cars();
                    } else if no_pedestrians {
                        edge.not_for_pedestrians();
                    }
                    edges.push(edge);
                    if !is_one_way {
                        let mut edge = EdgeInfo::new(
                            w.nodes[index + 1].0 as NodeId,
                            node.0 as NodeId,
                            1.1, // calculating length happens inside the graph
                            speed,
                        );
                        if no_cars {
                            edge.not_for_cars();
                        } else if no_pedestrians {
                            edge.not_for_pedestrians();
                        }
                        edges.push(edge);
                    }
                }
            }
            _ => (),
        }
    }
    println!("Amount of Edges {}", edges.len());

    return (nodes, edges);
}

fn determine_speed(way: &Way) -> Speed {
    let speed = way.tags.get("maxspeed").and_then(|s| s.parse().ok());
    if speed.is_some() {
        speed.unwrap()
    } else {
        let street_type = way.tags.get("highway").map(String::as_ref);
        match street_type {
            Some("motorway") => 130,
            Some("residential") => 50,
            _ => 100,
        }
    }
}
fn is_one_way(way: &Way) -> bool {
    let one_way = way.tags.get("oneway").and_then(|s| s.parse().ok());
    match one_way {
        Some(rule) => rule,
        None => {
            match way.tags.get("highway").map(|h| h == "motorway") {
                Some(rule) => rule,
                None => false,
            }
        }
    }
}
fn is_not_for_cars(way: &Way) -> bool {
    let street_type = way.tags.get("highway").map(String::as_ref);
    match street_type {
        Some("footway") |
        Some("bridleway") |
        Some("steps") |
        Some("path") |
        Some("cycleway") |
        Some("track") |
        Some("proposed") |
        Some("construction") |
        Some("pedestrian") => true,
        _ => false,
    }

}

fn is_not_for_pedestrians(way: &Way) -> bool {

    let street_type = way.tags.get("highway").map(String::as_ref);
    let side_walk: Option<&str> = way.tags.get("sidewalk").map(String::as_ref);
    let has_side_walk: bool = match side_walk {
        Some(s) => s != "no",
        None => false,
    };
    if has_side_walk {
        return false;
    }
    match street_type {
        Some("motorway") |
        Some("trunk") |
        Some("proposed") |
        Some("construction") |
        Some("primary") => true,
        _ => false,
    }

}

pub type NodeId = usize;
pub type OsmNodeId = usize;
pub type Latitude = f64;
pub type Longitude = f64;
pub type Length = f64;
pub type Speed = usize;
pub type Height = usize;

pub struct NodeInfo {
    pub osm_id: OsmNodeId,
    pub lat: Latitude,
    pub long: Longitude,
    pub height: Height,
}

impl NodeInfo {
    pub fn new(osm_id: OsmNodeId, lat: Latitude, long: Longitude, height: Height) -> NodeInfo {
        NodeInfo {
            osm_id: osm_id,
            lat: lat,
            long: long,
            height: height,
        }
    }
}

pub struct EdgeInfo {
    pub source: NodeId,
    pub dest: NodeId,
    length: Length,
    speed: Speed,
    for_cars: bool,
    for_pedestrians: bool,
}

impl EdgeInfo {
    pub fn new(source: NodeId, dest: NodeId, length: Length, speed: Speed) -> EdgeInfo {
        EdgeInfo {
            source: source,
            dest: dest,
            length: length,
            speed: speed,
            for_cars: true,
            for_pedestrians: true,
        }
    }

    /// Prevent routes for cars from using this edge
    pub fn not_for_cars(&mut self) {
        self.for_cars = false;
    }
    /// Prevent routes for pedestrians from using this edge
    pub fn not_for_pedestrians(&mut self) {
        self.for_pedestrians = false;
    }
}
