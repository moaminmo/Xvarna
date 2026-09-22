struct Node {
    minimum: vec4<f32>,
    maximum: vec4<f32>,
    data: vec4<u32>,
}

struct Triangle {
    first: vec4<f32>,
    edge_one: vec4<f32>,
    edge_two: vec4<f32>,
    data: vec4<u32>,
}

struct Instance {
    inverse_0: vec4<f32>,
    inverse_1: vec4<f32>,
    inverse_2: vec4<f32>,
    object_instance: vec4<u32>,
    mesh_category: vec4<u32>,
    data: vec4<u32>,
}

struct Ray {
    origin_minimum: vec4<f32>,
    direction_maximum: vec4<f32>,
    category: vec4<u32>,
}

struct Hit {
    values: vec4<f32>,
    object_instance: vec4<u32>,
    mesh_data: vec4<u32>,
}

struct Parameters {
    ray_count: u32,
    tlas_node_count: u32,
    blas_node_count: u32,
    triangle_count: u32,
    instance_count: u32,
    flags: u32,
    reserved_0: u32,
    reserved_1: u32,
}

@group(0) @binding(0) var<storage, read> tlas_nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tlas_indices: array<u32>;
@group(0) @binding(2) var<storage, read> instances: array<Instance>;
@group(0) @binding(3) var<storage, read> blas_nodes: array<Node>;
@group(0) @binding(4) var<storage, read> blas_indices: array<u32>;
@group(0) @binding(5) var<storage, read> triangles: array<Triangle>;
@group(0) @binding(6) var<storage, read> rays: array<Ray>;
@group(0) @binding(7) var<storage, read_write> hits: array<Hit>;
@group(0) @binding(8) var<uniform> parameters: Parameters;

const HIT_FLAG: u32 = 1u;
const FRONT_FACE_FLAG: u32 = 2u;
const MIRRORED_FLAG: u32 = 1u;
const STACK_LIMIT: u32 = 96u;

fn category_matches(ray: vec2<u32>, instance_category: vec2<u32>) -> bool {
    return ((ray.x & instance_category.x) | (ray.y & instance_category.y)) != 0u;
}

fn intersects_bounds(ray: Ray, node: Node, maximum: f32) -> bool {
    var near = ray.origin_minimum.w;
    var far = maximum;
    for (var axis = 0u; axis < 3u; axis += 1u) {
        let origin = ray.origin_minimum[axis];
        let direction = ray.direction_maximum[axis];
        let minimum = node.minimum[axis];
        let upper = node.maximum[axis];
        if (abs(direction) <= 1.0e-12) {
            if (origin < minimum || origin > upper) {
                return false;
            }
        } else {
            let inverse = 1.0 / direction;
            let first = (minimum - origin) * inverse;
            let second = (upper - origin) * inverse;
            near = max(near, min(first, second));
            far = min(far, max(first, second));
            if (near > far) {
                return false;
            }
        }
    }
    return true;
}

fn miss_hit() -> Hit {
    return Hit(
        vec4<f32>(bitcast<f32>(0x7f800000u), 0.0, 0.0, 0.0),
        vec4<u32>(0u),
        vec4<u32>(0u, 0u, 0xffffffffu, 0u),
    );
}

fn transform_ray(ray: Ray, instance: Instance) -> Ray {
    let origin = ray.origin_minimum.xyz;
    let direction = ray.direction_maximum.xyz;
    return Ray(
        vec4<f32>(
            dot(instance.inverse_0.xyz, origin) + instance.inverse_0.w,
            dot(instance.inverse_1.xyz, origin) + instance.inverse_1.w,
            dot(instance.inverse_2.xyz, origin) + instance.inverse_2.w,
            ray.origin_minimum.w,
        ),
        vec4<f32>(
            dot(instance.inverse_0.xyz, direction),
            dot(instance.inverse_1.xyz, direction),
            dot(instance.inverse_2.xyz, direction),
            ray.direction_maximum.w,
        ),
        ray.category,
    );
}

fn intersect_triangle(ray: Ray, triangle: Triangle, instance: Instance, maximum: f32) -> Hit {
    let direction = ray.direction_maximum.xyz;
    let p = cross(direction, triangle.edge_two.xyz);
    let determinant = dot(triangle.edge_one.xyz, p);
    let epsilon = 1.0e-7 * length(triangle.edge_one.xyz) * length(triangle.edge_two.xyz);
    if (abs(determinant) <= epsilon) {
        return miss_hit();
    }
    let inverse_determinant = 1.0 / determinant;
    let delta = ray.origin_minimum.xyz - triangle.first.xyz;
    let u = dot(delta, p) * inverse_determinant;
    if (u < 0.0 || u > 1.0) {
        return miss_hit();
    }
    let q = cross(delta, triangle.edge_one.xyz);
    let v = dot(direction, q) * inverse_determinant;
    if (v < 0.0 || u + v > 1.0) {
        return miss_hit();
    }
    let distance = dot(triangle.edge_two.xyz, q) * inverse_determinant;
    if (distance < ray.origin_minimum.w || distance > maximum) {
        return miss_hit();
    }
    var front = determinant > 0.0;
    if ((instance.data.y & MIRRORED_FLAG) != 0u) {
        front = !front;
    }
    var flags = HIT_FLAG;
    if (front) {
        flags |= FRONT_FACE_FLAG;
    }
    return Hit(
        vec4<f32>(distance, u, v, bitcast<f32>(flags)),
        instance.object_instance,
        vec4<u32>(instance.mesh_category.xy, triangle.data.x, flags),
    );
}

fn trace_instance(world_ray: Ray, instance: Instance, maximum: f32) -> Hit {
    let ray = transform_ray(world_ray, instance);
    var best = miss_hit();
    best.values.x = maximum;
    var stack: array<u32, 96>;
    var stack_length = 1u;
    stack[0] = instance.data.x;
    loop {
        if (stack_length == 0u) {
            break;
        }
        stack_length -= 1u;
        let node_index = stack[stack_length];
        if (node_index >= parameters.blas_node_count) {
            continue;
        }
        let node = blas_nodes[node_index];
        if (!intersects_bounds(ray, node, min(best.values.x, maximum))) {
            continue;
        }
        if (node.data.w > 0u) {
            for (var offset = 0u; offset < node.data.w; offset += 1u) {
                let primitive_offset = node.data.z + offset;
                let triangle_index = blas_indices[primitive_offset];
                if (triangle_index >= parameters.triangle_count) {
                    continue;
                }
                let candidate = intersect_triangle(
                    ray,
                    triangles[triangle_index],
                    instance,
                    min(best.values.x, maximum),
                );
                if ((candidate.mesh_data.w & HIT_FLAG) != 0u) {
                    best = candidate;
                    if ((parameters.flags & 1u) != 0u) {
                        return best;
                    }
                }
            }
        } else {
            if (stack_length + 2u > STACK_LIMIT) {
                return miss_hit();
            }
            stack[stack_length] = node.data.y;
            stack[stack_length + 1u] = node.data.x;
            stack_length += 2u;
        }
    }
    return best;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let ray_index = global_id.x;
    if (ray_index >= parameters.ray_count) {
        return;
    }
    let ray = rays[ray_index];
    var best = miss_hit();
    var stack: array<u32, 96>;
    var stack_length = 1u;
    stack[0] = 0u;
    loop {
        if (stack_length == 0u) {
            break;
        }
        stack_length -= 1u;
        let node_index = stack[stack_length];
        if (node_index >= parameters.tlas_node_count) {
            continue;
        }
        let node = tlas_nodes[node_index];
        if (!intersects_bounds(ray, node, min(best.values.x, ray.direction_maximum.w))) {
            continue;
        }
        if (node.data.w > 0u) {
            for (var offset = 0u; offset < node.data.w; offset += 1u) {
                let primitive_offset = node.data.z + offset;
                let instance_index = tlas_indices[primitive_offset];
                if (instance_index >= parameters.instance_count) {
                    continue;
                }
                let instance = instances[instance_index];
                if (!category_matches(ray.category.xy, instance.mesh_category.zw)) {
                    continue;
                }
                let candidate = trace_instance(
                    ray,
                    instance,
                    min(best.values.x, ray.direction_maximum.w),
                );
                if ((candidate.mesh_data.w & HIT_FLAG) != 0u) {
                    best = candidate;
                    if ((parameters.flags & 1u) != 0u) {
                        hits[ray_index] = best;
                        return;
                    }
                }
            }
        } else {
            if (stack_length + 2u > STACK_LIMIT) {
                hits[ray_index] = miss_hit();
                return;
            }
            stack[stack_length] = node.data.y;
            stack[stack_length + 1u] = node.data.x;
            stack_length += 2u;
        }
    }
    hits[ray_index] = best;
}
