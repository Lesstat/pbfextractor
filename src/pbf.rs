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
use osmpbfreader::{OsmObj, OsmPbfReader, Way};

use super::metrics::*;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::spawn;

pub type TagMetrics = Vec<Rc<dyn TagMetric<f64>>>;
pub type NodeMetrics = Vec<Rc<dyn NodeMetric<f64>>>;
pub type CostMetrics = Vec<Rc<dyn CostMetric<f64>>>;
pub type InternalMetrics = HashSet<String>;
pub type MetricIndices = BTreeMap<String, usize>;

pub struct Loader<'a, Filter: EdgeFilter> {
    pbf_path: &'a str,
    srtm_path: &'a str,
    edge_filter: Filter,
    tag_metrics: TagMetrics,
    node_metrics: NodeMetrics,
    cost_metrics: CostMetrics,
    pub internal_metrics: InternalMetrics,
    pub metrics_indices: MetricIndices,
    grid: Rc<RefCell<Grid>>,
}

#[allow(clippy::too_many_arguments)]
impl<'a, Filter: EdgeFilter> Loader<'a, Filter> {
    pub fn new(
        pbf_path: &'a str,
        srtm_path: &'a str,
        edge_filter: Filter,
        tag_metrics: TagMetrics,
        node_metrics: NodeMetrics,
        cost_metrics: CostMetrics,
        internal_metrics: InternalMetrics,
        grid: Rc<RefCell<Grid>>,
    ) -> Loader<'a, Filter> {
        let mut metrics_indices: MetricIndices = BTreeMap::new();
        let mut index = 0;
        for t in &tag_metrics {
            metrics_indices.insert(t.name(), index);
            index += 1;
        }
        for n in &node_metrics {
            metrics_indices.insert(n.name(), index);
            index += 1;
        }
        for c in &cost_metrics {
            metrics_indices.insert(c.name(), index);
            index += 1;
        }
        Loader {
            pbf_path,
            srtm_path,
            edge_filter,
            tag_metrics,
            node_metrics,
            cost_metrics,
            internal_metrics,
            metrics_indices,
            grid,
        }
    }

    /// Loads the graph from a pbf file.
    pub fn load_graph(&self) -> (Vec<Node>, Vec<Edge>) {
        println!("Extracting data out of: {}", self.pbf_path);
        let fs = File::open(&self.pbf_path).unwrap();
        let mut reader = OsmPbfReader::new(fs);

        let (id_sender, id_receiver) = channel();
        let set_receiver = self.collect_node_ids(id_receiver);

        let mut edges: Vec<Edge> = reader
            .par_iter()
            .flat_map(|obj| {
                if let Ok(OsmObj::Way(w)) = obj {
                    self.process_way(&w, &id_sender)
                } else {
                    Vec::new()
                }
            })
            .collect();
        println!("Collected {} edges", edges.len());
        reader.rewind().expect("Can't rewind pbf file!");
        drop(id_sender);

        let id_set = set_receiver.recv().expect("Did not get node ids");
        let srtm = self.metrics_indices.contains_key(&HeightAscent.name());

        let mut nodes: Vec<Node> = reader
            .par_iter()
            .filter_map(|obj| {
                if let Ok(OsmObj::Node(n)) = obj {
                    if id_set.contains(&n.id) {
                        let lat = f64::from(n.decimicro_lat) / 10_000_000.0;
                        let lng = f64::from(n.decimicro_lon) / 10_000_000.0;
                        let height = if srtm { self.srtm(lat, lng) } else { 0.0 };
                        Some(Node::new(n.id.0 as usize, lat, lng, height))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        {
            let mut grid = (*self.grid).borrow_mut();
            nodes.iter().for_each(|n| grid.add(&n));
        }

        println!("Collected {} nodes", nodes.len());

        println!("Calculating Metrics");

        self.rename_node_ids_and_calculate_node_metrics(&mut nodes, &mut edges);
        self.calculate_cost_metrics(&mut edges);

        println!("Deleting duplicate and dominated edges");

        self.delete_duplicate_edges(&mut edges);
        edges = self.delete_dominated_edges(edges);

        println!("{} edges left", edges.len());
        (nodes, edges)
    }
    fn internal_metric_count(&self) -> usize {
        self.node_metrics.len() + self.cost_metrics.len() + self.tag_metrics.len()
    }
    pub fn metric_count(&self) -> usize {
        self.internal_metric_count() - self.internal_metrics.len()
    }

    fn collect_node_ids(
        &self,
        ids: Receiver<osmpbfreader::NodeId>,
    ) -> Receiver<HashSet<osmpbfreader::NodeId>> {
        let (send, recv) = channel();

        spawn(move || {
            let mut set = HashSet::new();
            for id in ids {
                set.insert(id);
            }
            send.send(set)
                .expect("Cannot send node ids back to main thread");
        });
        recv
    }

    fn calculate_cost_metrics(&self, edges: &mut [Edge]) {
        for e in edges {
            for c in &self.cost_metrics {
                let index = self.metrics_indices[&c.name()];
                let value = c.calc(&e.costs, &self.metrics_indices).unwrap();
                e.costs[index] = value;
            }
        }
    }

    fn process_way(&self, w: &Way, id_sender: &Sender<osmpbfreader::NodeId>) -> Vec<Edge> {
        let mut edges = Vec::new();
        if self.edge_filter.is_invalid(&w.tags) {
            return edges;
        }

        let tag_costs: Vec<(usize, f64)> = self
            .tag_metrics
            .iter()
            .map(|t| (self.metrics_indices[&t.name()], t.calc(&w.tags).unwrap()))
            .collect();
        let is_one_way = self.is_one_way(&w);
        for (index, node) in w.nodes[0..(w.nodes.len() - 1)].iter().enumerate() {
            id_sender.send(*node).expect("could not send id to id set");
            let mut edge = Edge::new(
                node.0 as NodeId,
                w.nodes[index + 1].0 as NodeId,
                self.internal_metric_count(),
            );
            for (i, t) in &tag_costs {
                edge.costs[*i] = *t;
            }
            edges.push(edge);
            if !is_one_way {
                let mut edge = Edge::new(
                    w.nodes[index + 1].0 as NodeId,
                    node.0 as NodeId,
                    self.internal_metric_count(),
                );
                for (i, t) in &tag_costs {
                    edge.costs[*i] = *t;
                }
                edges.push(edge);
            }
        }

        id_sender
            .send(*w.nodes.last().unwrap())
            .expect("could not send id to id set");
        edges
    }
    fn is_one_way(&self, way: &Way) -> bool {
        let one_way = way.tags.get("oneway");
        match one_way.map(smartstring::SmartString::as_ref) {
            Some("yes") | Some("true") => true,
            Some("no") | Some("false") => false,
            _ => way
                .tags
                .get("highway")
                .map(|h| h == "motorway")
                .unwrap_or(false),
        }
    }

    fn rename_node_ids_and_calculate_node_metrics(&self, nodes: &mut [Node], edges: &mut [Edge]) {
        use std::collections::hash_map::HashMap;

        let map: HashMap<OsmNodeId, (usize, &Node)> =
            nodes.iter().enumerate().map(|n| (n.1.osm_id, n)).collect();
        for e in edges.iter_mut() {
            let (source_id, source) = map[&e.source];
            let (dest_id, dest) = map[&e.dest];
            e.source = source_id;
            e.dest = dest_id;
            for n in &self.node_metrics {
                let index = self.metrics_indices[&n.name()];
                let value = n.calc(source, dest).unwrap();
                e.costs[index] = value;
            }
        }
    }

    fn srtm(&self, lat: Latitude, lng: Longitude) -> f64 {
        use byteorder::{BigEndian, ReadBytesExt};
        use std::io::{Seek, SeekFrom};

        let second = 1.0 / 3600.0;

        let north = self.f64_to_whole_number(lat);
        let east = self.f64_to_whole_number(lng);

        let file_name = if east > 0 {
            format!("/N{:02}E{:03}.hgt", north, east)
        } else {
            format!("/N{:02}W{:03}.hgt", north, east.abs())
        };

        let mut srtm_file = String::new();
        srtm_file.push_str(self.srtm_path);
        srtm_file.push_str(&file_name);
        let mut f = match File::open(&srtm_file) {
            Ok(f) => f,
            Err(_) => {
                println!("could not find file: {}", file_name);
                return 0.0;
            }
        };
        let lat_offset = 3601.0 - lat.fract() / second;
        let lng_offset = lng.abs().fract() / second;

        let lat_offset_floor = lat_offset.floor() as u64;
        let lat_offset_ceil = lat_offset.ceil() as u64;
        let long_offset_floor = lng_offset.floor() as u64;
        let long_offset_ceil = lng_offset.ceil() as u64;

        let mut read_offsets = |lat_offset: u64, long_offset: u64| -> f64 {
            let seek_val = ((lat_offset - 1) * 3601 + (long_offset)) * 2;
            f.seek(SeekFrom::Start(seek_val)).unwrap_or_else(|_| {
                panic!(
                    "Seeking to value failed. latoff: {}, lngoff: {}, seekval: {}",
                    lat_offset, lng_offset, seek_val,
                )
            });

            f64::from(
                f.read_i16::<BigEndian>()
                    .unwrap_or_else(|_| panic!("Reading failed at {}, {}", lat, lng)),
            )
        };

        let h1 = read_offsets(lat_offset_floor, long_offset_floor);
        let h2 = read_offsets(lat_offset_ceil, long_offset_floor);
        let h3 = read_offsets(lat_offset_floor, long_offset_ceil);
        let h4 = read_offsets(lat_offset_ceil, long_offset_ceil);

        let h1_weight = (1.0 - lat_offset.fract()) * (1.0 - lng_offset.fract());
        let h2_weight = lat_offset.fract() * (1.0 - lng_offset.fract());
        let h3_weight = (1.0 - lat_offset.fract()) * lng_offset.fract();
        let h4_weight = lat_offset.fract() * lng_offset.fract();

        h1 * h1_weight + h2 * h2_weight + h3 * h3_weight + h4 * h4_weight
    }

    fn f64_to_whole_number(&self, x: f64) -> i64 {
        x.trunc() as i64
    }

    fn delete_duplicate_edges(&self, edges: &mut Vec<Edge>) {
        edges.sort_by(|e1, e2| {
            let mut result = e1.source.cmp(&e2.source);
            if result == Ordering::Equal {
                result = e1.dest.cmp(&e2.dest);
            }
            if result == Ordering::Equal {
                for (c1, c2) in e1.costs.iter().zip(e2.costs.iter()) {
                    result = c1.partial_cmp(c2).unwrap_or(Ordering::Equal);
                    if result != Ordering::Equal {
                        break;
                    }
                }
            }
            result
        });
        edges.dedup();
    }

    fn delete_dominated_edges(&self, edges: Vec<Edge>) -> Vec<Edge> {
        let mut indices = ::std::collections::BTreeSet::new();
        for i in 1..edges.len() {
            let first = &edges[i - 1];
            let second = &edges[i];
            if !(first.source == second.source && first.dest == second.dest) {
                continue;
            }
            if first
                .costs
                .iter()
                .zip(second.costs.iter())
                .all(|(f, s)| f <= s)
            {
                indices.insert(i);
            }
        }
        edges
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !indices.contains(i))
            .map(|(_, e)| e)
            .collect()
    }
}

pub type NodeId = usize;
pub type OsmNodeId = usize;
pub type Latitude = f64;
pub type Longitude = f64;

pub struct Node {
    pub osm_id: OsmNodeId,
    pub lat: Latitude,
    pub long: Longitude,
    pub height: f64,
}

impl Node {
    pub fn new(osm_id: OsmNodeId, lat: Latitude, long: Longitude, height: f64) -> Node {
        Node {
            osm_id,
            lat,
            long,
            height,
        }
    }
}

pub struct Edge {
    pub source: NodeId,
    pub dest: NodeId,
    costs: Vec<f64>,
}

impl Edge {
    pub fn new(source: NodeId, dest: NodeId, cost_count: usize) -> Edge {
        let costs = vec![0.0; cost_count];
        Edge {
            source,
            dest,
            costs,
        }
    }

    pub fn costs(&self, indices: &MetricIndices, internal_only: &InternalMetrics) -> Vec<f64> {
        let mut costs = Vec::new();
        for (metric, index) in indices.iter() {
            if internal_only.contains(metric) {
                continue;
            }
            costs.push(self.costs[*index]);
        }

        costs
    }
}

impl PartialEq for Edge {
    fn eq(&self, rhs: &Self) -> bool {
        self.source == rhs.source
            && self.dest == rhs.dest
            && self.costs.iter().zip(rhs.costs.iter()).all(|(a, b)| a == b)
    }
}
