//! Deterministic flattened binary BVH with binned SAH construction.

use xvarna_geometry::{Aabb, Vec3};

const BIN_COUNT: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub index: u32,
    pub bounds: Aabb,
    centroid: Vec3,
}

impl Primitive {
    pub fn new(index: u32, bounds: Aabb) -> Self {
        Self {
            index,
            bounds,
            centroid: bounds.center(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub bounds: Aabb,
    pub left: u32,
    pub right: u32,
    pub start: u32,
    pub count: u32,
}

impl Node {
    pub const fn is_leaf(self) -> bool {
        self.count > 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Bvh {
    pub nodes: Vec<Node>,
    pub primitive_indices: Vec<u32>,
    pub maximum_depth: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct Bin {
    count: usize,
    bounds: Option<Aabb>,
}

impl Bvh {
    pub fn build(mut primitives: Vec<Primitive>, maximum_leaf_size: usize) -> Self {
        if primitives.is_empty() {
            return Self::default();
        }
        let mut bvh = Self::default();
        build_node(&mut bvh, &mut primitives, maximum_leaf_size, 0);
        bvh
    }

    /// Recomputes leaf and internal bounds without changing topology or primitive order.
    pub fn refit(&mut self, primitive_bounds: &[Aabb]) -> bool {
        if self.nodes.is_empty() {
            return self.primitive_indices.is_empty();
        }
        if self
            .primitive_indices
            .iter()
            .any(|index| *index as usize >= primitive_bounds.len())
        {
            return false;
        }
        for node_index in (0..self.nodes.len()).rev() {
            let node = self.nodes[node_index];
            let bounds = if node.is_leaf() {
                let start = node.start as usize;
                let end = start.saturating_add(node.count as usize);
                let Some(indices) = self.primitive_indices.get(start..end) else {
                    return false;
                };
                let Some((&first, rest)) = indices.split_first() else {
                    return false;
                };
                rest.iter()
                    .fold(primitive_bounds[first as usize], |bounds, index| {
                        bounds.union(primitive_bounds[*index as usize])
                    })
            } else {
                let Some(left) = self.nodes.get(node.left as usize) else {
                    return false;
                };
                let Some(right) = self.nodes.get(node.right as usize) else {
                    return false;
                };
                left.bounds.union(right.bounds)
            };
            self.nodes[node_index].bounds = bounds;
        }
        true
    }

    /// Surface-area proxy used to reject degraded refits deterministically.
    pub fn quality_cost(&self) -> f64 {
        self.nodes
            .iter()
            .map(|node| {
                let weight = if node.is_leaf() {
                    f64::from(node.count.max(1))
                } else {
                    1.0
                };
                node.bounds.surface_area() * weight
            })
            .sum()
    }
}

fn build_node(
    bvh: &mut Bvh,
    primitives: &mut [Primitive],
    maximum_leaf_size: usize,
    depth: usize,
) -> u32 {
    bvh.maximum_depth = bvh.maximum_depth.max(depth);
    let bounds = primitive_bounds(primitives);
    let node_index = u32::try_from(bvh.nodes.len()).expect("BVH node count fits u32");
    bvh.nodes.push(Node {
        bounds,
        left: 0,
        right: 0,
        start: 0,
        count: 0,
    });

    let split = if primitives.len() <= maximum_leaf_size {
        None
    } else {
        choose_sah_split(primitives, bounds)
    };
    let Some((axis, split_bin, centroid_min, centroid_extent)) = split else {
        make_leaf(bvh, node_index, primitives);
        return node_index;
    };

    let mut middle = partition(primitives, axis, split_bin, centroid_min, centroid_extent);
    if middle == 0 || middle == primitives.len() {
        primitives.sort_by(|left, right| {
            component(left.centroid, axis).total_cmp(&component(right.centroid, axis))
        });
        middle = primitives.len() / 2;
    }
    if middle == 0 || middle == primitives.len() {
        make_leaf(bvh, node_index, primitives);
        return node_index;
    }

    let (left_primitives, right_primitives) = primitives.split_at_mut(middle);
    let left = build_node(bvh, left_primitives, maximum_leaf_size, depth + 1);
    let right = build_node(bvh, right_primitives, maximum_leaf_size, depth + 1);
    bvh.nodes[node_index as usize].left = left;
    bvh.nodes[node_index as usize].right = right;
    node_index
}

fn make_leaf(bvh: &mut Bvh, node_index: u32, primitives: &[Primitive]) {
    let start = u32::try_from(bvh.primitive_indices.len()).expect("primitive count fits u32");
    bvh.primitive_indices
        .extend(primitives.iter().map(|primitive| primitive.index));
    bvh.nodes[node_index as usize].start = start;
    bvh.nodes[node_index as usize].count =
        u32::try_from(primitives.len()).expect("leaf count fits u32");
}

fn primitive_bounds(primitives: &[Primitive]) -> Aabb {
    primitives
        .iter()
        .skip(1)
        .fold(primitives[0].bounds, |bounds, primitive| {
            bounds.union(primitive.bounds)
        })
}

fn centroid_bounds(primitives: &[Primitive]) -> Aabb {
    let mut bounds = Aabb::from_point(primitives[0].centroid);
    for primitive in &primitives[1..] {
        bounds.include(primitive.centroid);
    }
    bounds
}

fn choose_sah_split(
    primitives: &[Primitive],
    parent_bounds: Aabb,
) -> Option<(usize, usize, f64, f64)> {
    let parent_area = parent_bounds.surface_area();
    if parent_area <= 0.0 {
        return None;
    }
    let centroids = centroid_bounds(primitives);
    let mut best: Option<(f64, usize, usize, f64, f64)> = None;

    for axis in 0..3 {
        let minimum = component(centroids.min, axis);
        let extent = component(centroids.max, axis) - minimum;
        if extent <= 0.0 {
            continue;
        }
        let mut bins = [Bin::default(); BIN_COUNT];
        for primitive in primitives {
            let index = bin_index(component(primitive.centroid, axis), minimum, extent);
            bins[index].count += 1;
            bins[index].bounds = Some(
                bins[index]
                    .bounds
                    .map_or(primitive.bounds, |bounds| bounds.union(primitive.bounds)),
            );
        }

        let mut left_counts = [0_usize; BIN_COUNT - 1];
        let mut right_counts = [0_usize; BIN_COUNT - 1];
        let mut left_bounds = [None; BIN_COUNT - 1];
        let mut right_bounds = [None; BIN_COUNT - 1];
        let mut count = 0;
        let mut bounds = None;
        for split in 0..BIN_COUNT - 1 {
            count += bins[split].count;
            bounds = union_optional(bounds, bins[split].bounds);
            left_counts[split] = count;
            left_bounds[split] = bounds;
        }
        count = 0;
        bounds = None;
        for split in (0..BIN_COUNT - 1).rev() {
            count += bins[split + 1].count;
            bounds = union_optional(bounds, bins[split + 1].bounds);
            right_counts[split] = count;
            right_bounds[split] = bounds;
        }

        for split in 0..BIN_COUNT - 1 {
            if left_counts[split] == 0 || right_counts[split] == 0 {
                continue;
            }
            let left_area = left_bounds[split]
                .expect("non-empty left bin")
                .surface_area();
            let right_area = right_bounds[split]
                .expect("non-empty right bin")
                .surface_area();
            let left_count = f64::from(
                u32::try_from(left_counts[split]).expect("left primitive count fits u32"),
            );
            let right_count = f64::from(
                u32::try_from(right_counts[split]).expect("right primitive count fits u32"),
            );
            let weighted_area = right_area.mul_add(right_count, left_area * left_count);
            let cost = 1.0 + weighted_area / parent_area;
            if best.is_none_or(|current| cost < current.0) {
                best = Some((cost, axis, split, minimum, extent));
            }
        }
    }

    let leaf_cost = f64::from(u32::try_from(primitives.len()).expect("primitive count fits u32"));
    best.filter(|candidate| candidate.0 < leaf_cost)
        .map(|(_, axis, split, minimum, extent)| (axis, split, minimum, extent))
}

fn partition(
    primitives: &mut [Primitive],
    axis: usize,
    split_bin: usize,
    minimum: f64,
    extent: f64,
) -> usize {
    let mut middle = 0;
    for current in 0..primitives.len() {
        let index = bin_index(
            component(primitives[current].centroid, axis),
            minimum,
            extent,
        );
        if index <= split_bin {
            primitives.swap(current, middle);
            middle += 1;
        }
    }
    middle
}

fn bin_index(value: f64, minimum: f64, extent: f64) -> usize {
    let normalized = ((value - minimum) / extent).clamp(0.0, 1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = (normalized * 16.0) as usize;
    index.min(BIN_COUNT - 1)
}

const fn component(value: Vec3, axis: usize) -> f64 {
    match axis {
        0 => value.x,
        1 => value.y,
        _ => value.z,
    }
}

const fn union_optional(left: Option<Aabb>, right: Option<Aabb>) -> Option<Aabb> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.union(right)),
        (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binned_sah_builds_deterministic_hierarchy() {
        let primitives = (0..100)
            .map(|index| {
                let x = f64::from(index);
                Primitive::new(
                    index,
                    Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 1.0, 1.0)),
                )
            })
            .collect::<Vec<_>>();
        let first = Bvh::build(primitives.clone(), 4);
        let second = Bvh::build(primitives, 4);
        assert_eq!(first.primitive_indices, second.primitive_indices);
        assert_eq!(first.nodes.len(), second.nodes.len());
        assert!(first.nodes.len() > 1);
        assert!(first.maximum_depth > 1);
    }

    #[test]
    fn refit_preserves_topology_and_updates_root_bounds() {
        let primitives = (0..8)
            .map(|index| {
                let x = f64::from(index);
                Primitive::new(
                    index,
                    Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 1.0, 1.0)),
                )
            })
            .collect::<Vec<_>>();
        let mut bvh = Bvh::build(primitives, 2);
        let topology = bvh
            .nodes
            .iter()
            .map(|node| (node.left, node.right, node.start, node.count))
            .collect::<Vec<_>>();
        let moved = (0..8)
            .map(|index| {
                let x = f64::from(index) + 100.0;
                Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 1.0, 1.0))
            })
            .collect::<Vec<_>>();
        assert!(bvh.refit(&moved));
        assert_eq!(
            topology,
            bvh.nodes
                .iter()
                .map(|node| (node.left, node.right, node.start, node.count))
                .collect::<Vec<_>>()
        );
        assert!(bvh.nodes[0].bounds.min.x >= 100.0);
    }
}
