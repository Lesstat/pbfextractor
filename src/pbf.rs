use osmpbfreader::{OsmObj, OsmPbfReader, Way};

use std::path::Path;
use std::fs::File;


/// Loads the graph from a pbf file.
pub fn load_graph<P: AsRef<Path>>(p: P) -> (Vec<NodeInfo>, Vec<EdgeInfo>) {
    println!("Extracting data out of: {}", p.as_ref().to_str().unwrap());
    let fs = File::open(p).unwrap();
    let mut reader = OsmPbfReader::new(fs);
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
                ));
            }
            OsmObj::Way(w) => {
                if is_not_for_bicycle(&w) {
                    continue;
                }
                let unsuitability = determine_unsuitability(&w);
                let is_one_way = is_one_way(&w);
                for (index, node) in w.nodes[0..(w.nodes.len() - 1)].iter().enumerate() {
                    let edge = EdgeInfo::new(
                        node.0 as NodeId,
                        w.nodes[index + 1].0 as NodeId,
                        1.1, // calculating length happens inside the graph
                        1.1,
                        unsuitability,
                    );
                    edges.push(edge);
                    if !is_one_way {
                        let edge = EdgeInfo::new(
                            w.nodes[index + 1].0 as NodeId,
                            node.0 as NodeId,
                            1.1, // calculating length happens inside the graph
                            1.1,
                            unsuitability,
                        );
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

fn determine_unsuitability(way: &Way) -> Unsuitability {
    if way.tags.get("cycleway").is_some() || way.tags.get("bicycle") == Some(&"yes".to_string()) {
        return 0;
    }

    let side_walk: Option<&str> = way.tags.get("sidewalk").map(String::as_ref);
    if side_walk == Some("yes") {
        return 1;
    }

    let street_type = way.tags.get("highway").map(String::as_ref);
    match street_type {
        Some("primary") => 5,
        Some("primary_link") => 5,
        Some("secondary") => 4,
        Some("secondary_link") => 4,
        Some("tertiary") => 3,
        Some("tertiary_link") => 3,
        Some("road") => 3,
        Some("bridleway") => 3,
        Some("unclassified") => 2,
        Some("residential") => 2,
        Some("traffic_island") => 2,
        Some("living_street") => 1,
        Some("service") => 1,
        Some("track") => 1,
        Some("platform") => 1,
        Some("pedestrian") => 1,
        Some("path") => 1,
        Some("footway") => 1,
        Some("cycleway") => 0,
        _ => 10,
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

fn is_not_for_bicycle(way: &Way) -> bool {

    if way.tags.get("cycleway").is_some() || way.tags.get("bicycle") == Some(&"yes".to_string()) {
        return false;
    }

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
        Some("motorway_link") |
        Some("trunk") |
        Some("trunk_link") |
        Some("proposed") |
        Some("path") |
        Some("footway") |
        Some("steps") |
        Some("elevator") |
        Some("corridor") |
        Some("raceway") |
        Some("rest_area") |
        Some("construction") => true,
        _ => false,
    }

}

pub type NodeId = usize;
pub type OsmNodeId = usize;
pub type Latitude = f64;
pub type Longitude = f64;
pub type Length = f64;
pub type Height = f64;
pub type Unsuitability = usize;

pub struct NodeInfo {
    pub osm_id: OsmNodeId,
    pub lat: Latitude,
    pub long: Longitude,
}

impl NodeInfo {
    pub fn new(osm_id: OsmNodeId, lat: Latitude, long: Longitude) -> NodeInfo {
        NodeInfo {
            osm_id: osm_id,
            lat: lat,
            long: long,
        }
    }
}

pub struct EdgeInfo {
    pub source: NodeId,
    pub dest: NodeId,
    pub length: Length,
    pub height: Height,
    pub unsuitability: Unsuitability,
}

impl EdgeInfo {
    pub fn new(
        source: NodeId,
        dest: NodeId,
        length: Length,
        height: Height,
        unsuitability: Unsuitability,
    ) -> EdgeInfo {

        EdgeInfo {
            source: source,
            dest: dest,
            length: length,
            height: height,
            unsuitability: unsuitability,
        }
    }
}
