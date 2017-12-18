use osmpbfreader::{OsmObj, OsmPbfReader, Way};

use std::fs::File;

pub struct Loader {
    pbf_path: String,
    srtm_path: String,
}

impl Loader {
    pub fn new(pbf_path: String, srtm_path: String) -> Loader {
        Loader {
            pbf_path: pbf_path,
            srtm_path: srtm_path,
        }
    }

    /// Loads the graph from a pbf file.
    pub fn load_graph(&self) -> (Vec<NodeInfo>, Vec<EdgeInfo>) {
        println!("Extracting data out of: {}", self.pbf_path);
        let fs = File::open(&self.pbf_path).unwrap();
        let mut reader = OsmPbfReader::new(fs);
        let obj_map = reader
            .get_objs_and_deps(|obj| obj.tags().contains_key("highway"))
            .unwrap();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (_, obj) in obj_map {
            match obj {
                OsmObj::Node(node) => {
                    let lat = (node.decimicro_lat as f64) / 10_000_000.0;
                    let lng = (node.decimicro_lon as f64) / 10_000_000.0;
                    nodes.push(NodeInfo::new(
                        node.id.0 as usize,
                        lat,
                        lng,
                        self.srtm(lat, lng),
                    ));
                }
                OsmObj::Way(w) => {
                    if self.is_not_for_bicycle(&w) {
                        continue;
                    }
                    let unsuitability = self.determine_unsuitability(&w);
                    let is_one_way = self.is_one_way(&w);
                    for (index, node) in w.nodes[0..(w.nodes.len() - 1)].iter().enumerate() {
                        let edge = EdgeInfo::new(
                            node.0 as NodeId,
                            w.nodes[index + 1].0 as NodeId,
                            1.1, // calculating length happens inside the graph
                            0,
                            unsuitability,
                        );
                        edges.push(edge);
                        if !is_one_way {
                            let edge = EdgeInfo::new(
                                w.nodes[index + 1].0 as NodeId,
                                node.0 as NodeId,
                                1.1, // calculating length happens inside the graph
                                0,
                                unsuitability,
                            );
                            edges.push(edge);
                        }
                    }
                }
                _ => (),
            }
        }

        println!("Calculating distances and height differences on edges ");

        self.rename_node_ids_and_calculate_distance(&mut nodes, &mut edges);

        return (nodes, edges);
    }

    fn determine_unsuitability(&self, way: &Way) -> Unsuitability {
        if way.tags.get("cycleway").is_some() ||
            way.tags.get("bicycle") == Some(&"yes".to_string())
        {
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

    fn is_one_way(&self, way: &Way) -> bool {
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

    fn is_not_for_bicycle(&self, way: &Way) -> bool {

        if way.tags.get("cycleway").is_some() ||
            way.tags.get("bicycle") == Some(&"yes".to_string())
        {
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


    fn rename_node_ids_and_calculate_distance(
        &self,
        nodes: &mut [NodeInfo],
        edges: &mut [EdgeInfo],
    ) {
        use std::collections::hash_map::HashMap;

        let map: HashMap<OsmNodeId, (usize, &NodeInfo)> =
            nodes.iter().enumerate().map(|n| (n.1.osm_id, n)).collect();
        for e in edges.iter_mut() {
            let (source_id, source) = map[&e.source];
            let (dest_id, dest) = map[&e.dest];
            e.source = source_id;
            e.dest = dest_id;
            e.length = self.haversine_distance(source, dest);
            let height_difference = source.height - dest.height;
            e.height = if height_difference > 0 {
                height_difference
            } else {
                0
            };
        }

    }

    /// Calculate the haversine distance. Adapted from https://github.com/georust/rust-geo
    pub fn haversine_distance(&self, a: &NodeInfo, b: &NodeInfo) -> Length {
        const EARTH_RADIUS: f64 = 6_371_007.2;

        let theta1 = a.lat.to_radians();
        let theta2 = b.lat.to_radians();
        let delta_theta = (b.lat - a.lat).to_radians();
        let delta_lambda = (b.long - a.long).to_radians();
        let a = (delta_theta / 2.0).sin().powi(2) +
            theta1.cos() * theta2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        EARTH_RADIUS * c
    }

    fn srtm(&self, lat: Latitude, lng: Longitude) -> Height {
        use std::io::{Seek, SeekFrom};
        use byteorder::{ReadBytesExt, BigEndian};

        let second = 1.0 / 3600.0;

        let north = self.f64_to_whole_number(lat);
        let east = self.f64_to_whole_number(lng);

        let file_name = format!("/N{:02}E{:03}.hgt", north, east);


        let mut srtm_file = String::new();
        srtm_file.push_str(self.srtm_path.as_ref());
        srtm_file.push_str(&file_name);
        let mut f = File::open(&srtm_file).expect(&format!("srtm file {} not found", srtm_file));

        let lat_offset = 3601 - ((lat - north as f64) / second).round() as u64;
        let long_offset = ((lng - east as f64) / second).round() as u64;

        f.seek(SeekFrom::Start(
            ((lat_offset - 1) * 3601 + (long_offset)) * 2,
        )).unwrap();

        let h = f.read_i16::<BigEndian>().expect(&format!(
            "Reading failed at {}, {}",
            lat,
            lng
        ));

        h
    }

    fn f64_to_whole_number(&self, x: f64) -> u64 {
        x.trunc() as u64
    }
}


pub type NodeId = usize;
pub type OsmNodeId = usize;
pub type Latitude = f64;
pub type Longitude = f64;
pub type Length = f64;
pub type Height = i16;
pub type Unsuitability = usize;

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
