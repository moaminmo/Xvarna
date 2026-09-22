# XV Period

Category: `XVARNA > 02 Time`

`XV Period` is the first ZURVAN workflow component. It converts two explicit fixed-offset ISO 8601 boundaries and a positive step into interval-centred `SolarTimeSample` records. Start is inclusive, End is exclusive, and the last interval is shortened rather than extending past End.

The component refuses offset-free timestamps, mixed boundary offsets, negative weights, non-positive steps, and schedules above one million samples. A daylight-saving transition must be represented as separate fixed-offset periods so the time convention remains reviewable and independent of the host machine.

Outputs expose the immutable Period plus timestamps, actual durations, weights, total weighted hours, and a convention report.
