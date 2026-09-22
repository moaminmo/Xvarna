//! Area-weighted solar-surface intelligence and transparent PV potential proxy.

use std::collections::{BTreeMap, BTreeSet};
use xvarna_geometry::{SurfaceCell, Vec3};
use xvarna_types::SensorId;

/// Explicit assumptions for the non-bankable photovoltaic potential proxy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PvPotentialOptions {
    /// Minimum annual plane-of-array energy required for an eligible cell, Wh/m².
    pub minimum_irradiance_wh_m2: f64,
    /// Nameplate module efficiency at standard test conditions, from zero through one.
    pub module_efficiency: f64,
    /// Fraction of eligible geometric area assumed to be covered by modules.
    pub coverage_ratio: f64,
    /// Combined downstream system loss fraction, from zero through one.
    pub system_loss_fraction: f64,
}

impl PvPotentialOptions {
    /// Validates and creates PV proxy assumptions.
    pub fn try_new(
        minimum_irradiance_wh_m2: f64,
        module_efficiency: f64,
        coverage_ratio: f64,
        system_loss_fraction: f64,
    ) -> Result<Self, PvPotentialError> {
        if !minimum_irradiance_wh_m2.is_finite() || minimum_irradiance_wh_m2 < 0.0 {
            return Err(PvPotentialError::InvalidMinimumIrradiance);
        }
        if !module_efficiency.is_finite() || !(0.0..=1.0).contains(&module_efficiency) {
            return Err(PvPotentialError::InvalidModuleEfficiency);
        }
        if !coverage_ratio.is_finite() || !(0.0..=1.0).contains(&coverage_ratio) {
            return Err(PvPotentialError::InvalidCoverageRatio);
        }
        if !system_loss_fraction.is_finite() || !(0.0..=1.0).contains(&system_loss_fraction) {
            return Err(PvPotentialError::InvalidSystemLoss);
        }
        Ok(Self {
            minimum_irradiance_wh_m2,
            module_efficiency,
            coverage_ratio,
            system_loss_fraction,
        })
    }
}

impl Default for PvPotentialOptions {
    fn default() -> Self {
        Self {
            minimum_irradiance_wh_m2: 800_000.0,
            module_efficiency: 0.22,
            coverage_ratio: 0.85,
            system_loss_fraction: 0.14,
        }
    }
}

/// Solar and PV proxy values corresponding one-to-one with an analysis cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PvPotentialCell {
    /// Stable source sensor identifier.
    pub sensor_id: SensorId,
    /// Connected eligible-region identifier, or zero for an ineligible cell.
    pub region_id: u64,
    /// True when annual irradiance meets the configured threshold.
    pub is_eligible: bool,
    /// Cell area in m².
    pub area_m2: f64,
    /// Annual plane-of-array energy density in Wh/m².
    pub irradiance_wh_m2: f64,
    /// Total incident solar energy on the complete cell, kWh.
    pub incident_energy_kwh: f64,
    /// Proxy module output after coverage, efficiency, and aggregate loss, kWh.
    pub proxy_yield_kwh: f64,
}

/// Area and energy aggregation for one edge-connected eligible region.
#[derive(Clone, Debug, PartialEq)]
pub struct PvPotentialRegion {
    /// Stable one-based region identifier ordered by first source cell.
    pub region_id: u64,
    /// Number of eligible cells in this region.
    pub cell_count: usize,
    /// Eligible geometric surface area in m².
    pub area_m2: f64,
    /// Area-weighted annual irradiance, Wh/m².
    pub mean_irradiance_wh_m2: f64,
    /// Minimum annual cell irradiance, Wh/m².
    pub minimum_irradiance_wh_m2: f64,
    /// Maximum annual cell irradiance, Wh/m².
    pub maximum_irradiance_wh_m2: f64,
    /// Incident energy on this complete geometric region, kWh.
    pub incident_energy_kwh: f64,
    /// Proxy module output for this region, kWh.
    pub proxy_yield_kwh: f64,
}

/// Complete area-aware solar potential result.
#[derive(Clone, Debug, PartialEq)]
pub struct PvPotentialResult {
    /// Validated transparent model assumptions.
    pub options: PvPotentialOptions,
    /// Geometric area of all cells in m².
    pub total_area_m2: f64,
    /// Geometric area meeting the threshold in m².
    pub eligible_area_m2: f64,
    /// Area-weighted annual irradiance over all cells, Wh/m².
    pub mean_irradiance_wh_m2: f64,
    /// Area-weighted tenth percentile, Wh/m².
    pub p10_irradiance_wh_m2: f64,
    /// Area-weighted median, Wh/m².
    pub p50_irradiance_wh_m2: f64,
    /// Area-weighted ninetieth percentile, Wh/m².
    pub p90_irradiance_wh_m2: f64,
    /// Incident energy on all complete cells, kWh.
    pub total_incident_energy_kwh: f64,
    /// Incident energy on complete eligible cells, kWh.
    pub eligible_incident_energy_kwh: f64,
    /// Estimated installed DC nameplate capacity at 1 kW/m² STC, kWp.
    pub capacity_kwp: f64,
    /// Transparent annual output proxy after declared assumptions, kWh.
    pub proxy_yield_kwh: f64,
    /// Proxy annual energy divided by proxy nameplate capacity, kWh/kWp.
    pub specific_yield_kwh_kwp: f64,
    /// Values corresponding one-to-one with source cells.
    pub cells: Vec<PvPotentialCell>,
    /// Edge-connected eligible regions.
    pub regions: Vec<PvPotentialRegion>,
    /// Deterministic BLAKE3 identity of cells, irradiance, and assumptions.
    pub content_hash: [u8; 32],
}

/// Solar-potential validation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PvPotentialError {
    /// At least one analysis cell is required.
    EmptyCells,
    /// Irradiance count must equal cell count.
    ValueCountMismatch,
    /// Irradiance values must be finite and non-negative.
    InvalidIrradiance,
    /// Threshold must be finite and non-negative.
    InvalidMinimumIrradiance,
    /// Module efficiency must be from zero through one.
    InvalidModuleEfficiency,
    /// Coverage ratio must be from zero through one.
    InvalidCoverageRatio,
    /// Combined loss must be from zero through one.
    InvalidSystemLoss,
}

impl core::fmt::Display for PvPotentialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyCells => "PV potential requires at least one surface cell",
            Self::ValueCountMismatch => "irradiance count must match surface-cell count",
            Self::InvalidIrradiance => "irradiance values must be finite and non-negative",
            Self::InvalidMinimumIrradiance => "minimum irradiance must be finite and non-negative",
            Self::InvalidModuleEfficiency => "module efficiency must be between zero and one",
            Self::InvalidCoverageRatio => "coverage ratio must be between zero and one",
            Self::InvalidSystemLoss => "system loss must be between zero and one",
        })
    }
}

impl std::error::Error for PvPotentialError {}

/// Calculates area-weighted distribution statistics, connected hotspot regions,
/// and a deliberately transparent non-bankable PV output proxy.
#[allow(clippy::too_many_lines)]
pub fn analyze_pv_potential(
    cells: &[SurfaceCell],
    irradiance_wh_m2: &[f64],
    options: PvPotentialOptions,
) -> Result<PvPotentialResult, PvPotentialError> {
    let options = PvPotentialOptions::try_new(
        options.minimum_irradiance_wh_m2,
        options.module_efficiency,
        options.coverage_ratio,
        options.system_loss_fraction,
    )?;
    if cells.is_empty() {
        return Err(PvPotentialError::EmptyCells);
    }
    if irradiance_wh_m2.len() != cells.len() {
        return Err(PvPotentialError::ValueCountMismatch);
    }
    if irradiance_wh_m2
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(PvPotentialError::InvalidIrradiance);
    }

    let eligible: Vec<bool> = irradiance_wh_m2
        .iter()
        .map(|value| *value >= options.minimum_irradiance_wh_m2)
        .collect();
    let region_ids = connected_region_ids(cells, &eligible);
    let output_factor =
        options.module_efficiency * options.coverage_ratio * (1.0 - options.system_loss_fraction);
    let total_area_m2: f64 = cells.iter().map(|cell| cell.area).sum();
    let total_incident_energy_kwh: f64 = cells
        .iter()
        .zip(irradiance_wh_m2)
        .map(|(cell, value)| cell.area * value / 1000.0)
        .sum();
    let eligible_area_m2: f64 = cells
        .iter()
        .zip(&eligible)
        .filter(|(_, selected)| **selected)
        .map(|(cell, _)| cell.area)
        .sum();
    let eligible_incident_energy_kwh: f64 = cells
        .iter()
        .zip(irradiance_wh_m2)
        .zip(&eligible)
        .filter(|(_, selected)| **selected)
        .map(|((cell, value), _)| cell.area * value / 1000.0)
        .sum();
    let capacity_kwp = eligible_area_m2 * options.coverage_ratio * options.module_efficiency;
    let proxy_yield_kwh = eligible_incident_energy_kwh * output_factor;
    let specific_yield_kwh_kwp = if capacity_kwp > 0.0 {
        proxy_yield_kwh / capacity_kwp
    } else {
        0.0
    };
    let mean_irradiance_wh_m2 = if total_area_m2 > 0.0 {
        total_incident_energy_kwh * 1000.0 / total_area_m2
    } else {
        0.0
    };
    let distribution: Vec<(f64, f64)> = irradiance_wh_m2
        .iter()
        .copied()
        .zip(cells.iter().map(|cell| cell.area))
        .collect();
    let p10_irradiance_wh_m2 = weighted_quantile(&distribution, 0.10);
    let p50_irradiance_wh_m2 = weighted_quantile(&distribution, 0.50);
    let p90_irradiance_wh_m2 = weighted_quantile(&distribution, 0.90);

    let output_cells: Vec<PvPotentialCell> = cells
        .iter()
        .zip(irradiance_wh_m2)
        .zip(eligible.iter().zip(&region_ids))
        .map(|((cell, value), (selected, region_id))| {
            let incident = cell.area * value / 1000.0;
            PvPotentialCell {
                sensor_id: cell.sensor_id,
                region_id: *region_id,
                is_eligible: *selected,
                area_m2: cell.area,
                irradiance_wh_m2: *value,
                incident_energy_kwh: incident,
                proxy_yield_kwh: if *selected {
                    incident * output_factor
                } else {
                    0.0
                },
            }
        })
        .collect();
    let regions = summarize_regions(&output_cells);
    let content_hash = hash_potential(cells, irradiance_wh_m2, options, &region_ids);

    Ok(PvPotentialResult {
        options,
        total_area_m2,
        eligible_area_m2,
        mean_irradiance_wh_m2,
        p10_irradiance_wh_m2,
        p50_irradiance_wh_m2,
        p90_irradiance_wh_m2,
        total_incident_energy_kwh,
        eligible_incident_energy_kwh,
        capacity_kwp,
        proxy_yield_kwh,
        specific_yield_kwh_kwp,
        cells: output_cells,
        regions,
        content_hash,
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VertexKey([u64; 3]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EdgeKey(VertexKey, VertexKey);

fn vertex_key(point: Vec3) -> VertexKey {
    let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
    VertexKey([bits(point.x), bits(point.y), bits(point.z)])
}

fn edge_key(first: Vec3, second: Vec3) -> EdgeKey {
    let first = vertex_key(first);
    let second = vertex_key(second);
    if first <= second {
        EdgeKey(first, second)
    } else {
        EdgeKey(second, first)
    }
}

fn connected_region_ids(cells: &[SurfaceCell], eligible: &[bool]) -> Vec<u64> {
    let mut union = UnionFind::new(cells.len());
    let mut first_by_edge = BTreeMap::<EdgeKey, usize>::new();
    for (index, cell) in cells.iter().enumerate() {
        if !eligible[index] {
            continue;
        }
        for edge in [
            edge_key(cell.a, cell.b),
            edge_key(cell.b, cell.c),
            edge_key(cell.c, cell.a),
        ] {
            if let Some(other) = first_by_edge.get(&edge).copied() {
                union.join(index, other);
            } else {
                first_by_edge.insert(edge, index);
            }
        }
    }
    let mut region_by_root = BTreeMap::<usize, u64>::new();
    let mut next_region = 1_u64;
    let mut output = vec![0_u64; cells.len()];
    for index in 0..cells.len() {
        if !eligible[index] {
            continue;
        }
        let root = union.root(index);
        let region_id = *region_by_root.entry(root).or_insert_with(|| {
            let current = next_region;
            next_region += 1;
            current
        });
        output[index] = region_id;
    }
    output
}

fn summarize_regions(cells: &[PvPotentialCell]) -> Vec<PvPotentialRegion> {
    let region_ids: BTreeSet<u64> = cells
        .iter()
        .filter(|cell| cell.region_id != 0)
        .map(|cell| cell.region_id)
        .collect();
    region_ids
        .into_iter()
        .map(|region_id| {
            let members: Vec<_> = cells
                .iter()
                .filter(|cell| cell.region_id == region_id)
                .collect();
            let area_m2: f64 = members.iter().map(|cell| cell.area_m2).sum();
            let incident_energy_kwh: f64 =
                members.iter().map(|cell| cell.incident_energy_kwh).sum();
            PvPotentialRegion {
                region_id,
                cell_count: members.len(),
                area_m2,
                mean_irradiance_wh_m2: if area_m2 > 0.0 {
                    incident_energy_kwh * 1000.0 / area_m2
                } else {
                    0.0
                },
                minimum_irradiance_wh_m2: members
                    .iter()
                    .map(|cell| cell.irradiance_wh_m2)
                    .fold(f64::INFINITY, f64::min),
                maximum_irradiance_wh_m2: members
                    .iter()
                    .map(|cell| cell.irradiance_wh_m2)
                    .fold(0.0_f64, f64::max),
                incident_energy_kwh,
                proxy_yield_kwh: members.iter().map(|cell| cell.proxy_yield_kwh).sum(),
            }
        })
        .collect()
}

fn weighted_quantile(values: &[(f64, f64)], quantile: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|first, second| first.0.total_cmp(&second.0));
    let total_weight: f64 = sorted.iter().map(|(_, weight)| weight).sum();
    if total_weight <= 0.0 {
        return 0.0;
    }
    let target = total_weight * quantile;
    let mut accumulated = 0.0;
    for (value, weight) in &sorted {
        accumulated += weight;
        if accumulated >= target {
            return *value;
        }
    }
    sorted.last().map_or(0.0, |value| value.0)
}

fn hash_potential(
    cells: &[SurfaceCell],
    irradiance: &[f64],
    options: PvPotentialOptions,
    region_ids: &[u64],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"xvarna.pv-potential-proxy.v1\0");
    for value in [
        options.minimum_irradiance_wh_m2,
        options.module_efficiency,
        options.coverage_ratio,
        options.system_loss_fraction,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    for ((cell, value), region_id) in cells.iter().zip(irradiance).zip(region_ids) {
        hasher.update(&cell.sensor_id.get().to_le_bytes());
        hasher.update(&cell.source_face_index.to_le_bytes());
        hasher.update(&cell.area.to_bits().to_le_bytes());
        for point in [cell.a, cell.b, cell.c] {
            for coordinate in [point.x, point.y, point.z] {
                hasher.update(&coordinate.to_bits().to_le_bytes());
            }
        }
        hasher.update(&value.to_bits().to_le_bytes());
        hasher.update(&region_id.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(length: usize) -> Self {
        Self {
            parent: (0..length).collect(),
            rank: vec![0; length],
        }
    }

    fn root(&mut self, index: usize) -> usize {
        if self.parent[index] != index {
            self.parent[index] = self.root(self.parent[index]);
        }
        self.parent[index]
    }

    fn join(&mut self, first: usize, second: usize) {
        let mut first_root = self.root(first);
        let mut second_root = self.root(second);
        if first_root == second_root {
            return;
        }
        if self.rank[first_root] < self.rank[second_root] {
            core::mem::swap(&mut first_root, &mut second_root);
        }
        self.parent[second_root] = first_root;
        if self.rank[first_root] == self.rank[second_root] {
            self.rank[first_root] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xvarna_geometry::{Mesh, SurfaceGridOptions, generate_surface_grid};

    fn grid() -> Vec<SurfaceCell> {
        let mesh = Mesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        generate_surface_grid(
            &mesh,
            SurfaceGridOptions::try_new(2.0, 0.0, 1, 10).expect("options"),
        )
        .expect("grid")
        .cells
    }

    #[test]
    fn potential_is_area_weighted_and_regions_are_connected() {
        let cells = grid();
        let result = analyze_pv_potential(
            &cells,
            &[1_000_000.0, 500_000.0],
            PvPotentialOptions::try_new(400_000.0, 0.20, 0.80, 0.10).expect("options"),
        )
        .expect("potential");
        assert!((result.total_area_m2 - 1.0).abs() < 1.0e-12);
        assert!((result.mean_irradiance_wh_m2 - 750_000.0).abs() < 1.0e-9);
        assert!((result.total_incident_energy_kwh - 750.0).abs() < 1.0e-12);
        assert!((result.capacity_kwp - 0.16).abs() < 1.0e-12);
        assert!((result.proxy_yield_kwh - 108.0).abs() < 1.0e-12);
        assert_eq!(result.regions.len(), 1);
        assert_eq!(result.regions[0].cell_count, 2);
    }

    #[test]
    fn threshold_splits_eligible_and_ineligible_cells() {
        let cells = grid();
        let result = analyze_pv_potential(
            &cells,
            &[1_000_000.0, 500_000.0],
            PvPotentialOptions::try_new(800_000.0, 0.2, 1.0, 0.0).expect("options"),
        )
        .expect("potential");
        assert_eq!(
            result.cells.iter().filter(|cell| cell.is_eligible).count(),
            1
        );
        assert_eq!(result.cells[1].region_id, 0);
        assert!((result.eligible_area_m2 - 0.5).abs() < 1.0e-12);
        assert!((result.proxy_yield_kwh - 100.0).abs() < 1.0e-12);
    }

    #[test]
    fn potential_rejects_mismatched_or_invalid_values() {
        let cells = grid();
        assert_eq!(
            analyze_pv_potential(&cells, &[1.0], PvPotentialOptions::default()),
            Err(PvPotentialError::ValueCountMismatch)
        );
        assert_eq!(
            analyze_pv_potential(&cells, &[1.0, f64::NAN], PvPotentialOptions::default()),
            Err(PvPotentialError::InvalidIrradiance)
        );
    }
}
