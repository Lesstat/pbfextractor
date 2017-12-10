extern crate osmpbfreader;

mod pbf;

use std::fs::File;
use std::io::{Write, BufWriter};
use std::time::SystemTime;
use std::env::args;

fn main() {

    let mut a = args();
    a.next();
    let input = a.next().expect("No input file given");
    let output = a.next().expect("No output file given");


    let f = File::create(&output).unwrap();
    let mut b = BufWriter::new(f);

    let (nodes, edges) = pbf::load_graph(input);

    println!("Writing to: {}", output);

    write!(&mut b, "# Build by: pbfextractor\n").unwrap();
    write!(&mut b, "# Build on: {:?}\n", SystemTime::now()).unwrap();
    write!(&mut b, "\n").unwrap();
    write!(&mut b, "{}\n", nodes.len()).unwrap();
    write!(&mut b, "{}\n", edges.len()).unwrap();

    for (i, node) in nodes.iter().enumerate() {
        write!(
            &mut b,
            "{} {} {} {} 0\n",
            i,
            node.osm_id,
            node.lat,
            node.long
        ).unwrap();
    }
    for edge in &edges {
        write!(
            &mut b,
            "{} {} {} {} {} -1 -1\n",
            edge.source,
            edge.dest,
            edge.length,
            edge.height,
            edge.unsuitability
        ).unwrap();
    }

    b.flush().unwrap();

}
