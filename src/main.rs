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

use std::env::args;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::SystemTime;

fn main() {
    let mut a = args();
    a.next();
    let pbf_input = a.next().expect("No pbf input file given");
    let srtm_input = a.next().expect("No srtm input file given");
    let output = a.next().expect("No output file given");

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

    let l = pbf::Loader::new(pbf_input, srtm_input);

    let (nodes, edges) = l.load_graph();

    println!("Writing to: {}", output);

    write!(&mut complete_b, "# Build by: pbfextractor\n").unwrap();
    write!(&mut complete_b, "# Build on: {:?}\n", SystemTime::now()).unwrap();
    write!(&mut complete_b, "\n").unwrap();

    write!(&mut complete_b, "{}\n", nodes.len()).unwrap();
    write!(&mut complete_b, "{}\n", edges.len()).unwrap();

    write!(&mut graph_b, "{}\n", nodes.len()).unwrap();
    write!(&mut graph_b, "{}\n", edges.len()).unwrap();

    for (i, node) in nodes.iter().enumerate() {
        write!(
            &mut graph_b,
            "{} {} {} {} {} 0\n",
            i, node.osm_id, node.lat, node.long, node.height,
        ).unwrap();
        write!(
            &mut complete_b,
            "{} {} {} {} {} 0\n",
            i, node.osm_id, node.lat, node.long, node.height,
        ).unwrap();
    }
    for edge in &edges {
        write!(&mut graph_b, "{} {}\n", edge.source, edge.dest,).unwrap();
        write!(
            &mut metric_b,
            "{} {} {}\n",
            edge.length, edge.height, edge.unsuitability
        ).unwrap();
        write!(
            &mut complete_b,
            "{} {} {} {} {} -1 -1\n",
            edge.source, edge.dest, edge.length, edge.height, edge.unsuitability
        ).unwrap();
    }

    graph_b.flush().unwrap();
    metric_b.flush().unwrap();
    complete_b.flush().unwrap();
}
