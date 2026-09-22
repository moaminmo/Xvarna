# XV Shadow Mask

Category: `XVARNA > 03 Solar`

`XV Shadow Mask` extracts one zero-based sensor row from `XV Sky View`. It is deliberately separate from the aggregate component: thousands of per-direction values do not flood the Grasshopper document unless the user requests a specific sensor.

## Inputs

- **Sky Result**: complete `XV Sky View` Result.
- **Sensor Index**: zero-based row.
- **Radius**: positive model-space length for equal-radius dome rays.
- **Preview**: enables or disables viewport drawing.

## Outputs

- selected sensor **Origin**;
- ordered world-space **Directions** and **Visible** states;
- first-hit **Distance** and **Hit Points**;
- exact **Object IDs**, **Instance IDs**, **Mesh IDs**, and **Triangles**;
- equal-radius **Mask Rays** and **State Colours**;
- selected summary and result identity in **Report**.

Cyan rays reach open sky. Magenta rays are blocked. Visible rows use empty IDs, `NaN` distance, unset hit point, and triangle `-1`; blocked rows retain the exact first ZAMYAD hit. The output can drive custom diagrams, object-selection tools, CSV writers, or research visualizations without retracing the scene.
