# Pbfextractor

Pbfextractor is a tool to extract graph files from OSM and SRTM data for the [cycle-routing](https://github.com/lesstat/cycle-routing) project.
It extracts a graph for cyclists that contains data on the distance, height ascent and road suitability for bicycles.
The data for road suitability is based on the "highway", "cycleway", "bicycle" and "sidewalk" tags from OSM and has been tested for German graphs.


# Usage

Pbfextractor takes three arguments:
	- a pbf file
	- the path to a folder with the necassary SRTM files
	- the path to a file in which to write the graph

``` shell
pbfextractor [path/to/pbf-file] [folder/with/srtm/files] [path/to/output/file]
```

# Installation

To Compile and install Pbfextractor you need a current installation of [rust](https://www.rust-lang.org/en-US/install.html).
After cloning the package, it can be installed with

``` shell
cargo install --path [path/to/your/bin]
```

Or you compile it and run it directly from the repository:

``` shell
cargo build --release
./target/release/pbfextractor [path/to/pbf-file] [folder/with/srtm/files] [path/to/output/file]
```





