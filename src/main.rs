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

mod metrics;
mod pbf;
mod units;

use self::metrics::*;
use self::pbf::*;

use clap::Arg;
use clap::{arg, Command};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::rc::Rc;
use std::time::SystemTime;

fn main() {
    let app = Command::new("PBF Extractor")
        .author("Florian Barth")
        .about("Extracts Graphs with multidimensional costs from PBF files")
        .args(&[
            arg!(zipped: -z ... "saves graph gzipped"),
            Arg::new("PBF-FILE").help("PBF File to extract from").required(true),
            Arg::new("SRTM").help("Directory with srtm files").required(true),
            Arg::new("GRAPH").help("File to write graph to").required(true),
        ]);

    let matches = app
        .get_matches();


    let zip = matches.is_present("zipped");

    let pbf_input = matches
        .value_of("PBF-FILE")
        .expect("No PBF File to extract from");
    let srtm_input = matches.value_of("SRTM").expect("No srtm input file given");
    let output = matches.value_of("GRAPH").expect("No output file given");
    let grid = Grid::new_ptr();

    let dist = Rc::new(Distance);
    let car = Rc::new(CarSpeed);
    let fast_car = Rc::new(FastCarSpeed);
    let truck = Rc::new(TruckSpeed);

    let _grid_x = Rc::new(GridX(grid.clone()));
    let _grid_y = Rc::new(GridY(grid.clone()));
    let _chess = Rc::new(ChessBoard(grid.clone()));

    let _car_time = Rc::new(TravelTime::new(dist.clone(), car));
    let _fast_car_time = Rc::new(TravelTime::new(dist.clone(), fast_car));
    let _truck_time = Rc::new(TravelTime::new(dist.clone(), truck));

    let _random = Rc::new(RandomWeights);

    let internal_only_metrics: InternalMetrics = vec![].into_iter().collect();

    let tag_metrics: TagMetrics = vec![];
    let node_metrics: NodeMetrics = vec![dist];
    let cost_metrics: CostMetrics = vec![];

    let l = pbf::Loader::new(
        pbf_input,
        srtm_input,
        CarEdgeFilter,
        tag_metrics,
        node_metrics,
        cost_metrics,
        internal_only_metrics,
        grid,
    );

    let output_file = File::create(&output).unwrap();
    let graph = BufWriter::new(output_file);
    if zip {
        let graph = flate2::write::GzEncoder::new(graph, flate2::Compression::best());
        write_graph(&l, graph);
    } else {
        write_graph(&l, graph);
    }
}

fn write_graph<T: EdgeFilter, W: Write>(l: &Loader<T>, mut graph: W) {
    let (nodes, edges) = l.load_graph();

    writeln!(&mut graph, "# Build by: pbfextractor").unwrap();
    writeln!(&mut graph, "# Build on: {:?}", SystemTime::now()).unwrap();
    write!(&mut graph, "# metrics: ").unwrap();

    for metric in l.metrics_indices.keys() {
        if l.internal_metrics.contains(metric) {
            continue;
        }
        write!(&mut graph, "{}, ", metric).unwrap();
    }

    write!(&mut graph, "\n\n").unwrap();

    writeln!(&mut graph, "{}", l.metric_count()).unwrap();
    writeln!(&mut graph, "{}", nodes.len()).unwrap();
    writeln!(&mut graph, "{}", edges.len()).unwrap();

    for (i, node) in nodes.iter().enumerate() {
        writeln!(
            &mut graph,
            "{} {} {} {} {} 0",
            i, node.osm_id, node.lat, node.long, node.height,
        )
        .unwrap();
    }
    for edge in &edges {
        write!(&mut graph, "{} {} ", edge.source, edge.dest).unwrap();
        for cost in &edge.costs(&l.metrics_indices, &l.internal_metrics) {
            write!(&mut graph, "{} ", cost.round()).unwrap();
        }
        writeln!(&mut graph, "-1 -1").unwrap();
    }
    graph.flush().unwrap();
}
