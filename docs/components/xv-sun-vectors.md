# XV Sun Vectors

Category: `XVARNA > 02 Time`

`XV Sun Vectors` calculates apparent solar altitude and azimuth from WGS84 latitude, longitude, elevation, UTC-normalized timestamps, Delta T, pressure, and temperature. It accepts either `XV Period` or manual timestamp lists with zero/one/exact data matching for durations and weights.

The output vector points from a sensor toward the sun. Before the user-supplied true-north rotation, +X is east, +Y is true north, and +Z is up. Rotation is counter-clockwise about +Z from model +Y. Minimum Altitude and zero weights control the Active output but do not delete source rows, so every result stays aligned to its schedule.

The Sun Set retains native-aligned data for `XV Sun Hours` and carries a BLAKE3 identity over every calculation input and output.
