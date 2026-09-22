# Xvarna — LinkedIn draft

## Post

Xvarna brings environmental and spatial analysis into a workflow where the assumptions and evidence remain attached to the result.

Its Rust core connects scene queries, environmental indicators, sensitivity and study tools to version-specific Grasshopper adapters. Fast calculations and reference calculations have distinct, documented roles.

The attached figure comes from the production Radiance export and runner. For a large diffuse ground plane, it returned 4,999.41 lux of reflected light against an analytical reference of approximately 5,000 lux. The upward sensor returned 9,998.81 lux under a 10,000-lux uniform sky.

This is an analytical acceptance case, not measured-building certification. The source, settings, CSV and executable example are included so the result can be reproduced and extended.

#EnvironmentalDesign #DaylightSimulation #Grasshopper3D #ResearchSoftware

## Image

Attach `evidence.png` as the main image.

**Alt text:** The actual Radiance runner returns about 9,998.81 lux under a 10,000-lux uniform sky and 4,999.41 lux of ground-reflected light against a 5,000-lux large-plane reference. The CSV is retained with the reproduction command.

**Posting note:** Add the actual repository link after the repository has been created. No URL or release DOI has been invented.
