# Weather validation fixtures

`tehran-mini.epw` is synthetic and deliberately contains only eight hourly intervals. It verifies EPW syntax, fixed UTC+03:30 midpoint conversion, changing albedo, solar calculation, annual-engine aggregation, CLI JSON, and CSV alignment quickly. It is not meteorological evidence and must never be used as a representative Tehran climate file.

Full-year third-party EPW files are not vendored because their licensing and provenance vary. Release validation should additionally run a documented, legally redistributable full-year source and record its SHA-256 hash without committing ambiguous weather data.
