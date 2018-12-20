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

extern crate byteorder;
extern crate osmpbfreader;

mod pbf;

mod metrics;

use self::metrics::*;
use self::pbf::*;

use std::env::args;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::rc::Rc;
use std::time::SystemTime;

fn main() {
    let mut a = args();
    a.next();
    let pbf_input = a.next().expect("No pbf input file given");
    let srtm_input = a.next().expect("No srtm input file given");
    let output = a.next().expect("No output file given");
    let grid = Grid::new_ptr();

    let dist = Rc::new(Distance);
    let car = Rc::new(CarSpeed);
    let fast_car = Rc::new(FastCarSpeed);
    let truck = Rc::new(TruckSpeed);

    let _grid_x = Rc::new(GridX(grid.clone()));
    let _grid_y = Rc::new(GridY(grid.clone()));
    let _chess = Rc::new(ChessBoard(grid.clone()));

    let _car_time = Rc::new(TravelTime::new(dist.clone(), car.clone()));
    let _fast_car_time = Rc::new(TravelTime::new(dist.clone(), fast_car.clone()));
    let _truck_time = Rc::new(TravelTime::new(dist.clone(), truck.clone()));

    let internal_only_metrics: InternalMetrics = vec![].into_iter().collect();

    let tag_metrics: TagMetrics = vec![];
    let node_metrics: NodeMetrics = vec![dist];
    let cost_metrics: CostMetrics = vec![];

    let l = pbf::Loader::new(
        pbf_input,
        srtm_input,
        BicycleEdgeFilter,
        tag_metrics,
        node_metrics,
        cost_metrics,
        internal_only_metrics,
        grid,
    );

    let mut complete_output = output.clone();
    complete_output.push_str(".complete");

    let mut graph_output = output.clone();
    graph_output.push_str(".graph");

    let mut metric_output = output.clone();
    metric_output.push_str(".metric");

    let complete_f = File::create(&complete_output).unwrap();
    let mut complete_b = BufWriter::new(complete_f);

    let graph_f = File::create(&graph_output).unwrap();
    let mut graph_b = BufWriter::new(graph_f);

    let metric_f = File::create(&metric_output).unwrap();
    let mut metric_b = BufWriter::new(metric_f);

    let (nodes, edges) = l.load_graph();

    println!("Writing to: {}", output);

    writeln!(&mut complete_b, "# Build by: pbfextractor").unwrap();
    writeln!(&mut complete_b, "# Build on: {:?}", SystemTime::now()).unwrap();
    writeln!(&mut complete_b).unwrap();

    writeln!(&mut complete_b, "{}", l.metric_count()).unwrap();
    writeln!(&mut complete_b, "{}", nodes.len()).unwrap();
    writeln!(&mut complete_b, "{}", edges.len()).unwrap();

    writeln!(&mut graph_b, "{}", nodes.len()).unwrap();
    writeln!(&mut graph_b, "{}", edges.len()).unwrap();

    for (i, node) in nodes.iter().enumerate() {
        writeln!(
            &mut graph_b,
            "{} {} {} {} {} 0",
            i, node.osm_id, node.lat, node.long, node.height,
        )
        .unwrap();
        writeln!(
            &mut complete_b,
            "{} {} {} {} {} 0",
            i, node.osm_id, node.lat, node.long, node.height,
        )
        .unwrap();
    }
    for edge in &edges {
        writeln!(&mut graph_b, "{} {}", edge.source, edge.dest,).unwrap();
        write!(&mut complete_b, "{} {} ", edge.source, edge.dest).unwrap();
        for cost in &edge.costs(&l.metrics_indices, &l.internal_metrics) {
            write!(&mut metric_b, "{} ", cost).unwrap();
            write!(&mut complete_b, "{} ", cost).unwrap();
        }
        writeln!(&mut metric_b).unwrap();
        writeln!(&mut complete_b, "-1 -1").unwrap();
    }
    graph_b.flush().unwrap();
    metric_b.flush().unwrap();
    complete_b.flush().unwrap();
}
