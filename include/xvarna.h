#ifndef XVARNA_H
#define XVARNA_H

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#define XV_API __declspec(dllimport)
#else
#define XV_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef enum xv_status {
    XV_STATUS_SUCCESS = 0,
    XV_STATUS_NULL_POINTER = 1,
    XV_STATUS_INVALID_LENGTH = 2,
    XV_STATUS_INVALID_ARGUMENT = 3,
    XV_STATUS_INVALID_HANDLE = 4,
    XV_STATUS_INVALID_STATE = 5,
    XV_STATUS_CANCELLED = 6,
    XV_STATUS_PANIC = 255
} xv_status;

typedef uint64_t xv_scene_handle;
typedef uint64_t xv_job_handle;
typedef uint64_t xv_optimizer_handle;
typedef uint64_t xv_compute_handle;

typedef struct xv_job_progress_info {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t total_units;
    uint64_t completed_units;
    uint64_t reserved;
} xv_job_progress_info;

typedef struct xv_scene_options {
    uint32_t structure_size;
    uint32_t maximum_leaf_size;
    uint32_t thread_count;
    uint32_t reserved;
    double unit_scale_to_meters;
    double absolute_tolerance_meters;
} xv_scene_options;

typedef struct xv_ray {
    double origin_x;
    double origin_y;
    double origin_z;
    double direction_x;
    double direction_y;
    double direction_z;
    double t_min;
    double t_max;
    uint64_t category_mask;
} xv_ray;

typedef struct xv_hit {
    uint32_t structure_size;
    uint32_t flags;
    double distance;
    double barycentric_u;
    double barycentric_v;
    uint64_t object_id;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint32_t triangle_id;
    uint32_t reserved;
} xv_hit;

typedef struct xv_scene_stats {
    uint32_t structure_size;
    uint32_t thread_count;
    uint64_t mesh_resource_count;
    uint64_t instance_count;
    uint64_t unique_triangle_count;
    uint64_t instanced_triangle_count;
    uint64_t blas_node_count;
    uint64_t tlas_node_count;
    uint64_t maximum_bvh_depth;
    uint64_t approximate_memory_bytes;
    uint64_t build_time_microseconds;
    double rebase_origin_x;
    double rebase_origin_y;
    double rebase_origin_z;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_scene_stats;

typedef enum xv_scene_layer {
    XV_SCENE_LAYER_STATIC = 0,
    XV_SCENE_LAYER_DYNAMIC = 1
} xv_scene_layer;

typedef enum xv_scene_delta_operation {
    XV_SCENE_DELTA_UPDATE = 0,
    XV_SCENE_DELTA_ADD = 1,
    XV_SCENE_DELTA_REMOVE = 2
} xv_scene_delta_operation;

/* Update flags: bit 0 transform, bit 1 object, bit 2 category, bit 3 layer. */
typedef struct xv_scene_instance_delta {
    uint32_t structure_size;
    uint32_t operation;
    uint32_t flags;
    uint32_t layer;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint64_t object_id;
    uint64_t category_mask;
    double transform[16];
} xv_scene_instance_delta;

typedef struct xv_scene_update_options {
    uint32_t structure_size;
    uint32_t maximum_consecutive_refits;
    double maximum_refit_quality_ratio;
} xv_scene_update_options;

/* flags: static action bits 0..1, dynamic action bits 2..3; 0 reuse, 1 refit, 2 rebuild. */
typedef struct xv_scene_update_report {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t delta_count;
    uint64_t static_change_count;
    uint64_t dynamic_change_count;
    uint64_t reused_blas_count;
    uint64_t rebuilt_blas_count;
    uint64_t update_time_microseconds;
    double maximum_refit_quality_ratio;
    uint64_t previous_hash[4];
    uint64_t current_hash[4];
} xv_scene_update_report;

typedef enum xv_compute_preference {
    XV_COMPUTE_AUTO = 0,
    XV_COMPUTE_REQUIRE_PORTABLE_GPU = 1,
    XV_COMPUTE_CPU = 2
} xv_compute_preference;

typedef enum xv_compute_backend {
    XV_BACKEND_CPU = 0,
    XV_BACKEND_PORTABLE_GPU = 1
} xv_compute_backend;

typedef struct xv_compute_options {
    uint32_t structure_size;
    uint32_t preference;
    uint32_t power_preference;
    uint32_t flags;
    double maximum_precision_error_meters;
    uint64_t maximum_expanded_triangle_count;
    uint64_t maximum_rays_per_dispatch;
    uint64_t maximum_geometry_chunk_bytes;
    uint64_t maximum_gpu_memory_bytes;
    uint64_t reserved;
} xv_compute_options;

typedef struct xv_compute_info {
    uint32_t structure_size;
    uint32_t backend;
    uint32_t flags;
    uint32_t adapter_backend;
    uint32_t device_type;
    uint32_t vendor_id;
    uint32_t device_id;
    uint32_t geometry_chunk_count;
    uint64_t triangle_count;
    uint64_t node_count;
    uint64_t approximate_gpu_bytes;
    uint64_t initialization_microseconds;
    double scene_precision_error_meters;
    uint8_t adapter_name[128];
    uint8_t message[256];
} xv_compute_info;

typedef struct xv_compute_batch_stats {
    uint32_t structure_size;
    uint32_t backend;
    uint32_t flags;
    uint32_t reserved;
    uint64_t ray_count;
    uint64_t dispatch_count;
    uint64_t upload_microseconds;
    uint64_t execution_microseconds;
    uint64_t readback_microseconds;
    double precision_error_meters;
} xv_compute_batch_stats;

/* flags bit 0: device healthy. Counters are cumulative and never reset. */
typedef struct xv_compute_runtime_stats {
    uint32_t structure_size;
    uint32_t flags;
    uint32_t reserved_0;
    uint32_t reserved_1;
    uint64_t trace_count;
    uint64_t ray_count;
    uint64_t dispatch_count;
    uint64_t reusable_buffer_batch_count;
    uint64_t runtime_fallback_count;
    uint64_t scratch_generation_count;
    uint64_t scratch_capacity_rays;
    uint64_t scratch_gpu_bytes;
    uint64_t uploaded_bytes;
    uint64_t readback_bytes;
    uint64_t device_loss_count;
    uint64_t execution_error_count;
    uint64_t geometry_upload_count;
    uint64_t geometry_uploaded_bytes;
    uint8_t last_error[256];
} xv_compute_runtime_stats;

typedef struct xv_cache_options {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t memory_budget_bytes;
    uint64_t disk_budget_bytes;
} xv_cache_options;

typedef struct xv_cache_stats {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t memory_hit_count;
    uint64_t disk_hit_count;
    uint64_t miss_count;
    uint64_t write_count;
    uint64_t eviction_count;
    uint64_t corruption_count;
    uint64_t memory_bytes;
    uint64_t disk_bytes;
    uint64_t memory_entry_count;
    uint64_t disk_entry_count;
    uint64_t memory_budget_bytes;
    uint64_t disk_budget_bytes;
    uint8_t directory[256];
    uint8_t last_event[256];
} xv_cache_stats;

typedef struct xv_adapter_info {
    uint32_t structure_size;
    uint32_t backend;
    uint32_t device_type;
    uint32_t flags;
    uint32_t vendor_id;
    uint32_t device_id;
    uint32_t reserved_0;
    uint32_t reserved_1;
    uint8_t name[128];
    uint8_t driver[64];
} xv_adapter_info;

typedef struct xv_daena_execution_info {
    uint32_t structure_size;
    uint32_t backend;
    uint32_t flags;
    uint32_t reserved;
    uint64_t runtime_fallback_batch_count;
    uint64_t batch_count;
    uint64_t ray_count;
    uint64_t dispatch_count;
    uint64_t upload_microseconds;
    uint64_t execution_microseconds;
    uint64_t readback_microseconds;
    double maximum_precision_error_meters;
    uint8_t adapter_name[128];
    uint8_t fallback_reason[256];
} xv_daena_execution_info;

typedef struct xv_solar_options {
    uint32_t structure_size;
    uint32_t reserved;
    double latitude_degrees;
    double longitude_degrees;
    double elevation_meters;
    double delta_t_seconds;
    double pressure_millibars;
    double temperature_celsius;
    double north_rotation_degrees;
    double minimum_altitude_degrees;
} xv_solar_options;

typedef struct xv_time_sample {
    int64_t unix_seconds_utc;
    double duration_hours;
    double weight;
} xv_time_sample;

typedef struct xv_sun_sample {
    int64_t unix_seconds_utc;
    double direction_x;
    double direction_y;
    double direction_z;
    double altitude_degrees;
    double azimuth_degrees;
    double duration_hours;
    double weight;
    uint32_t flags;
    uint32_t reserved;
} xv_sun_sample;

typedef struct xv_sun_set_metadata {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t sample_count;
    uint64_t active_sample_count;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_sun_set_metadata;

typedef struct xv_solar_sensor {
    uint64_t sensor_id;
    double position_x;
    double position_y;
    double position_z;
    double normal_x;
    double normal_y;
    double normal_z;
} xv_solar_sensor;

typedef struct xv_direct_sun_options {
    uint32_t structure_size;
    uint32_t reserved;
    double sensor_offset_meters;
    double maximum_distance_meters;
    double minimum_incidence_cosine;
    uint64_t category_mask;
} xv_direct_sun_options;

typedef struct xv_direct_sun_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_id;
    double direct_sun_hours;
    double shadow_hours;
    double eligible_hours;
    double solar_access_ratio;
    uint64_t visible_count;
    uint64_t blocked_count;
    uint64_t back_facing_count;
    uint64_t dominant_occluder_object_id;
    double dominant_occluder_hours;
} xv_direct_sun_summary;

typedef struct xv_sun_timeline_entry {
    uint32_t structure_size;
    uint32_t state;
    uint64_t object_id;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint32_t triangle_id;
    uint32_t reserved;
    double distance_meters;
} xv_sun_timeline_entry;

typedef struct xv_direct_sun_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_count;
    uint64_t sun_count;
    uint64_t timeline_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_direct_sun_metadata;

typedef struct xv_sky_view_options {
    uint32_t structure_size;
    uint32_t sample_count;
    uint64_t seed;
    double sensor_offset_meters;
    double maximum_distance_meters;
    uint64_t category_mask;
} xv_sky_view_options;

typedef struct xv_sky_view_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_id;
    double visible_hemisphere_fraction;
    double cosine_weighted_svf;
    double visible_solid_angle_steradians;
    double cosine_convergence_delta;
    double unweighted_convergence_delta;
    uint64_t visible_count;
    uint64_t blocked_count;
    uint64_t dominant_occluder_object_id;
    double dominant_occluder_solid_angle_steradians;
    double dominant_occluder_projected_fraction;
} xv_sky_view_summary;

typedef struct xv_sky_ray_entry {
    uint32_t structure_size;
    uint32_t state;
    double direction_x;
    double direction_y;
    double direction_z;
    uint64_t object_id;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint32_t triangle_id;
    uint32_t reserved;
    double distance_meters;
} xv_sky_ray_entry;

typedef struct xv_sky_view_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_count;
    uint64_t sample_count;
    uint64_t timeline_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_sky_view_metadata;

typedef struct xv_epw_metadata {
    uint32_t structure_size;
    uint32_t records_per_hour;
    uint64_t weather_count;
    uint64_t missing_global_horizontal_count;
    uint64_t missing_direct_normal_count;
    uint64_t missing_diffuse_horizontal_count;
    double latitude_degrees;
    double longitude_degrees;
    double time_zone_hours;
    double elevation_meters;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_epw_metadata;

typedef struct xv_annual_irradiance_options {
    uint32_t structure_size;
    uint32_t flags;
    double sensor_offset_meters;
    double maximum_distance_meters;
    uint64_t category_mask;
    double ground_albedo;
    double delta_t_seconds;
    double pressure_millibars;
    double temperature_celsius;
    double north_rotation_degrees;
    double minimum_altitude_degrees;
} xv_annual_irradiance_options;

typedef struct xv_irradiance_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_id;
    double direct_wh_m2;
    double diffuse_sky_wh_m2;
    double ground_reflected_wh_m2;
    double global_wh_m2;
    double peak_global_w_m2;
    int64_t peak_unix_seconds_utc;
    double dome_visibility_ratio;
    double horizon_visibility_ratio;
    uint64_t visible_count;
    uint64_t blocked_count;
    uint64_t back_facing_count;
    uint64_t dominant_solar_occluder_object_id;
    double dominant_solar_occluder_loss_wh_m2;
    uint64_t dominant_sky_occluder_object_id;
    double dominant_sky_occluder_projected_fraction;
} xv_irradiance_summary;

typedef struct xv_irradiance_timeline_entry {
    uint32_t structure_size;
    uint32_t state;
    int64_t unix_seconds_utc;
    double direct_wh_m2;
    double diffuse_dome_wh_m2;
    double diffuse_circumsolar_wh_m2;
    double diffuse_horizon_wh_m2;
    double diffuse_sky_wh_m2;
    double ground_reflected_wh_m2;
    double global_wh_m2;
    double global_w_m2;
    double attributed_loss_wh_m2;
    uint64_t object_id;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint32_t triangle_id;
    uint32_t reserved;
    double distance_meters;
} xv_irradiance_timeline_entry;

typedef struct xv_annual_irradiance_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t sensor_count;
    uint64_t weather_count;
    uint64_t timeline_count;
    uint64_t analysis_time_microseconds;
    uint64_t missing_global_horizontal_count;
    uint64_t missing_direct_normal_count;
    uint64_t missing_diffuse_horizontal_count;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_annual_irradiance_metadata;

typedef struct xv_mesh_audit_summary {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t vertex_count;
    uint64_t face_count;
    uint64_t accepted_face_count;
    uint64_t non_finite_vertex_count;
    uint64_t invalid_index_face_count;
    uint64_t non_finite_face_count;
    uint64_t degenerate_face_count;
    uint64_t duplicate_face_count;
    uint64_t isolated_vertex_count;
    uint64_t boundary_edge_count;
    uint64_t non_manifold_edge_count;
    uint64_t inconsistent_winding_edge_count;
    uint64_t connected_component_count;
    double surface_area;
    double signed_volume;
    double bounds_min_x;
    double bounds_min_y;
    double bounds_min_z;
    double bounds_max_x;
    double bounds_max_y;
    double bounds_max_z;
    double maximum_coordinate_magnitude;
    double estimated_f32_error;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_mesh_audit_summary;

typedef struct xv_surface_grid_options {
    uint32_t structure_size;
    uint32_t reserved;
    double target_edge_length;
    double sensor_offset;
    uint64_t first_sensor_id;
    uint64_t maximum_cell_count;
} xv_surface_grid_options;

typedef struct xv_surface_cell {
    uint32_t structure_size;
    uint32_t source_face_index;
    uint64_t sensor_id;
    double position_x;
    double position_y;
    double position_z;
    double normal_x;
    double normal_y;
    double normal_z;
    double area;
    double a_x;
    double a_y;
    double a_z;
    double b_x;
    double b_y;
    double b_z;
    double c_x;
    double c_y;
    double c_z;
    uint32_t subdivision_depth;
    uint32_t reserved;
} xv_surface_cell;

typedef struct xv_surface_grid_metadata {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t source_face_count;
    uint64_t cell_count;
    uint64_t skipped_face_count;
    uint64_t maximum_subdivision_depth;
    double source_area;
    double sampled_area;
    double maximum_cell_edge_length;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_surface_grid_metadata;

typedef struct xv_pv_potential_options {
    uint32_t structure_size;
    uint32_t reserved;
    double unit_scale_to_meters;
    double minimum_irradiance_wh_m2;
    double module_efficiency;
    double coverage_ratio;
    double system_loss_fraction;
} xv_pv_potential_options;

typedef struct xv_pv_potential_cell {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t sensor_id;
    uint64_t region_id;
    double area_m2;
    double irradiance_wh_m2;
    double incident_energy_kwh;
    double proxy_yield_kwh;
} xv_pv_potential_cell;

typedef struct xv_pv_potential_region {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t region_id;
    uint64_t cell_count;
    double area_m2;
    double mean_irradiance_wh_m2;
    double minimum_irradiance_wh_m2;
    double maximum_irradiance_wh_m2;
    double incident_energy_kwh;
    double proxy_yield_kwh;
} xv_pv_potential_region;

typedef struct xv_pv_potential_metadata {
    uint32_t structure_size;
    uint32_t flags;
    uint64_t cell_count;
    uint64_t eligible_cell_count;
    uint64_t region_count;
    double total_area_m2;
    double eligible_area_m2;
    double mean_irradiance_wh_m2;
    double p10_irradiance_wh_m2;
    double p50_irradiance_wh_m2;
    double p90_irradiance_wh_m2;
    double total_incident_energy_kwh;
    double eligible_incident_energy_kwh;
    double capacity_kwp;
    double proxy_yield_kwh;
    double specific_yield_kwh_kwp;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_pv_potential_metadata;

typedef struct xv_viewpoint {
    uint64_t viewpoint_id;
    double position_x;
    double position_y;
    double position_z;
    double plane_normal_x;
    double plane_normal_y;
    double plane_normal_z;
    double forward_x;
    double forward_y;
    double forward_z;
} xv_viewpoint;

typedef struct xv_isovist_options {
    uint32_t structure_size;
    uint32_t sample_count;
    double field_of_view_radians;
    double maximum_distance_meters;
    double eye_offset_meters;
    uint64_t category_mask;
} xv_isovist_options;

typedef struct xv_isovist_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t viewpoint_id;
    double area_square_meters;
    double perimeter_meters;
    double centroid_distance_meters;
    double mean_radial_meters;
    double minimum_radial_meters;
    double maximum_radial_meters;
    double radial_standard_deviation_meters;
    double radial_skewness;
    double compactness;
    double area_convergence_delta_square_meters;
    uint64_t occluded_count;
    uint64_t open_count;
    uint64_t dominant_occluder_object_id;
    double dominant_occluder_fraction;
} xv_isovist_summary;

typedef struct xv_isovist_ray {
    uint32_t structure_size;
    uint32_t state;
    double direction_x;
    double direction_y;
    double direction_z;
    double endpoint_x;
    double endpoint_y;
    double endpoint_z;
    double distance_meters;
    uint64_t object_id;
    uint64_t instance_id;
    uint64_t mesh_id;
    uint32_t triangle_id;
    uint32_t reserved;
} xv_isovist_ray;

typedef struct xv_isovist_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t viewpoint_count;
    uint64_t sample_count;
    uint64_t ray_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_isovist_metadata;

typedef struct xv_visibility_observer {
    uint64_t observer_id;
    double position_x;
    double position_y;
    double position_z;
    double weight;
} xv_visibility_observer;

typedef struct xv_visibility_target {
    uint64_t target_id;
    double position_x;
    double position_y;
    double position_z;
    double facing_x;
    double facing_y;
    double facing_z;
    double sensitivity;
    uint32_t flags;
    uint32_t reserved;
} xv_visibility_target;

typedef struct xv_intervisibility_options {
    uint32_t structure_size;
    uint32_t reserved;
    double endpoint_clearance_meters;
    double maximum_distance_meters;
    double privacy_reference_distance_meters;
    double facing_exponent;
    uint64_t category_mask;
} xv_intervisibility_options;

typedef struct xv_intervisibility_entry {
    uint32_t structure_size;
    uint32_t state;
    double distance_meters;
    double direction_x;
    double direction_y;
    double direction_z;
    double privacy_risk;
    double facing_factor;
    double distance_factor;
    uint64_t blocker_object_id;
    uint64_t blocker_instance_id;
    uint64_t blocker_mesh_id;
    uint32_t blocker_triangle_id;
    uint32_t reserved;
} xv_intervisibility_entry;

typedef struct xv_observer_visibility_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t observer_id;
    uint64_t visible_count;
    double visible_fraction;
    double total_privacy_risk;
    double peak_privacy_risk;
} xv_observer_visibility_summary;

typedef struct xv_target_visibility_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t target_id;
    uint64_t visible_observer_count;
    double exposure_fraction;
    double cumulative_privacy_risk;
    double combined_privacy_risk;
} xv_target_visibility_summary;

typedef struct xv_intervisibility_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t observer_count;
    uint64_t target_count;
    uint64_t entry_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_intervisibility_metadata;

typedef struct xv_visibility_node {
    uint64_t node_id;
    double position_x;
    double position_y;
    double position_z;
} xv_visibility_node;

typedef struct xv_visibility_graph_options {
    uint32_t structure_size;
    uint32_t flags;
    double endpoint_clearance_meters;
    double maximum_distance_meters;
    uint64_t category_mask;
} xv_visibility_graph_options;

typedef struct xv_sparse_visibility_graph_options {
    uint32_t structure_size;
    uint32_t flags;
    double endpoint_clearance_meters;
    double maximum_distance_meters;
    uint64_t category_mask;
    uint32_t maximum_neighbors;
    uint32_t reserved;
} xv_sparse_visibility_graph_options;

typedef struct xv_visibility_graph_pair {
    uint32_t structure_size;
    uint32_t state;
    uint64_t first_index;
    uint64_t second_index;
    double distance_meters;
    uint64_t blocker_object_id;
} xv_visibility_graph_pair;

typedef struct xv_visibility_node_metrics {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t node_id;
    uint64_t degree;
    double degree_centrality;
    uint64_t component_index;
    double harmonic_closeness;
    double betweenness_centrality;
} xv_visibility_node_metrics;

typedef struct xv_visibility_graph_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t node_count;
    uint64_t pair_count;
    uint64_t visible_edge_count;
    uint64_t connected_component_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0;
    uint64_t content_hash_1;
    uint64_t content_hash_2;
    uint64_t content_hash_3;
} xv_visibility_graph_metadata;

/* DAENA solid-angle Target / Weighted / Green View */
typedef struct xv_view_observer {
    uint64_t observer_id;
    double position_x, position_y, position_z;
    double forward_x, forward_y, forward_z;
    double up_x, up_y, up_z;
    double weight;
} xv_view_observer;

typedef struct xv_view_target_patch {
    uint64_t target_id;
    double first_x, first_y, first_z;
    double second_x, second_y, second_z;
    double third_x, third_y, third_z;
    uint64_t category_mask;
    double category_weight;
} xv_view_target_patch;

typedef struct xv_target_view_options {
    uint32_t structure_size;
    uint32_t flags; /* bit 0: two-sided target patches */
    uint32_t samples_per_patch;
    uint32_t reserved;
    double horizontal_fov_radians;
    double vertical_fov_radians;
    double maximum_distance_meters;
    double endpoint_clearance_meters;
    double distance_reference_meters;
    double distance_exponent;
    double direction_exponent;
    uint64_t occluder_category_mask;
    uint64_t target_category_mask;
    uint64_t green_category_mask;
} xv_target_view_options;

typedef struct xv_target_view_entry {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t observer_id;
    uint64_t target_id;
    uint64_t category_mask;
    double potential_solid_angle_steradians;
    double visible_solid_angle_steradians;
    double visibility_fraction;
    double fov_fraction;
    double weighted_fov_score;
    double convergence_delta_steradians;
    uint64_t eligible_sample_count;
    uint64_t visible_sample_count;
    uint64_t dominant_blocker_object_id;
    double dominant_blocked_solid_angle_steradians;
} xv_target_view_entry;

typedef struct xv_target_view_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t observer_id;
    double fov_solid_angle_steradians;
    double potential_target_solid_angle_steradians;
    double visible_target_solid_angle_steradians;
    double target_view_fraction;
    double target_universe_visibility_fraction;
    double weighted_view_score;
    double green_view_index;
    double green_share_of_visible_targets;
    uint64_t dominant_target_id;
    double convergence_delta_steradians;
} xv_target_view_summary;

typedef struct xv_target_view_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t observer_count;
    uint64_t target_count;
    uint64_t entry_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_target_view_metadata;

/* DAENA protected target-aperture corridors */
typedef struct xv_view_corridor {
    uint64_t corridor_id;
    double origin_x, origin_y, origin_z;
    double target_x, target_y, target_z;
    double up_x, up_y, up_z;
    double target_radius_meters;
} xv_view_corridor;

typedef struct xv_view_corridor_options {
    uint32_t structure_size;
    uint32_t sample_count;
    double endpoint_clearance_meters;
    uint64_t category_mask;
} xv_view_corridor_options;

typedef struct xv_view_corridor_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t corridor_id;
    double aperture_solid_angle_steradians;
    uint64_t open_sample_count;
    uint64_t blocked_sample_count;
    double open_fraction;
    double open_solid_angle_steradians;
    double convergence_delta;
    uint64_t dominant_blocker_object_id;
    double dominant_blocker_fraction;
    double nearest_blocker_distance_meters;
} xv_view_corridor_summary;

typedef struct xv_view_corridor_sample {
    uint32_t structure_size;
    uint32_t state;
    uint64_t corridor_id;
    double aperture_x, aperture_y, aperture_z;
    double direction_x, direction_y, direction_z;
    double aperture_distance_meters;
    double first_hit_distance_meters;
    uint64_t blocker_object_id;
    uint64_t blocker_instance_id;
    uint64_t blocker_mesh_id;
    uint32_t blocker_triangle_id;
    uint32_t reserved;
} xv_view_corridor_sample;

typedef struct xv_view_corridor_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t corridor_count;
    uint64_t samples_per_corridor;
    uint64_t sample_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_view_corridor_metadata;

/* DAENA Dynamic Observer Path */
typedef struct xv_point3 { double x, y, z; } xv_point3;

typedef struct xv_observer_path {
    uint64_t path_id;
    uint64_t vertex_offset;
    uint64_t vertex_count;
    double up_x, up_y, up_z;
    double weight;
} xv_observer_path;

typedef struct xv_observer_path_options {
    uint32_t structure_size;
    uint32_t reserved;
    double spacing_meters;
    xv_target_view_options view;
} xv_observer_path_options;

typedef struct xv_observer_path_sample {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t path_id;
    uint64_t sample_index;
    double distance_along_path_meters;
    double position_x, position_y, position_z;
    double forward_x, forward_y, forward_z;
    double target_view_fraction;
    double weighted_view_score;
    double green_view_index;
    uint64_t dominant_target_id;
    double convergence_delta_steradians;
} xv_observer_path_sample;

typedef struct xv_observer_path_summary {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t path_id;
    double path_length_meters;
    uint64_t sample_count;
    double mean_target_view_fraction;
    double mean_weighted_view_score;
    double mean_green_view_index;
    double minimum_weighted_view_score;
    double maximum_weighted_view_score;
    uint64_t worst_sample_index;
    uint64_t best_sample_index;
} xv_observer_path_summary;

typedef struct xv_observer_path_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t path_count;
    uint64_t sample_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_observer_path_metadata;

/* XVARNA Study: mixed-variable constraint-aware Pareto analysis and ask/tell optimization */
typedef struct xv_variable_spec {
    uint32_t structure_size;
    uint32_t kind; /* 0 continuous, 1 integer, 2 categorical */
    uint64_t variable_id;
    double lower_bound;
    double upper_bound;
} xv_variable_spec;

typedef struct xv_ranked_solution {
    uint32_t structure_size;
    uint32_t flags; /* bit 0: feasible */
    uint64_t candidate_id;
    uint64_t pareto_rank;
    double total_constraint_violation;
    double crowding_distance;
    uint64_t dominated_solution_count;
} xv_ranked_solution;

typedef struct xv_study_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t candidate_count;
    uint64_t objective_count;
    uint64_t constraint_count;
    uint64_t front_count;
    uint64_t pareto_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_study_metadata;

typedef struct xv_sensitivity_coefficient {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t variable_index;
    uint64_t objective_index;
    double spearman_rho;
} xv_sensitivity_coefficient;

typedef struct xv_optimizer_config {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t population_size;
    uint64_t offspring_size;
    uint64_t archive_capacity;
    uint64_t seed;
    double crossover_probability;
    double mutation_probability;
    double crossover_distribution_index;
    double mutation_distribution_index;
} xv_optimizer_config;

typedef struct xv_optimizer_metadata {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t generation;
    uint64_t evaluation_count;
    uint64_t pending_candidate_count;
    uint64_t population_count;
    uint64_t archive_count;
    double mean_finite_crowding_distance;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_optimizer_metadata;

/* XVARNA 0.16 persistent Study/Optimization/Report platform. */
typedef struct xv_study_report_options {
    uint32_t structure_size;
    uint32_t reserved;
    uint64_t bootstrap_samples;
    uint64_t permutation_samples;
    double confidence_level;
    double alpha;
    uint64_t seed;
} xv_study_report_options;

/* XVARNA 0.14 DAENA/HVARE material and intelligence contracts. */
typedef struct xv_analysis_material {
    uint64_t material_id;
    double visible_transmittance;
    double solar_transmittance;
    double reflectance;
} xv_analysis_material;

typedef struct xv_material_assignment {
    uint64_t object_id;
    uint64_t material_id;
} xv_material_assignment;

typedef struct xv_spatial_viewpoint {
    uint64_t viewpoint_id;
    double position_x, position_y, position_z;
} xv_spatial_viewpoint;

typedef struct xv_isovist_3d_options {
    uint32_t structure_size, sample_count;
    double maximum_distance_meters, eye_offset_meters;
    uint64_t category_mask;
    uint32_t top_k, counterfactual_count, maximum_material_layers, reserved;
    double minimum_transmission;
} xv_isovist_3d_options;

typedef struct xv_isovist_3d_summary {
    uint32_t structure_size, reserved;
    uint64_t viewpoint_id;
    double volume_cubic_meters, radial_surface_square_meters, mean_radial_meters;
    double minimum_radial_meters, maximum_radial_meters, visible_solid_angle_steradians;
    double openness_ratio, volume_convergence_delta_cubic_meters;
    uint64_t dominant_occluder_object_id;
    double dominant_occluder_fraction;
} xv_isovist_3d_summary;

typedef struct xv_isovist_3d_ray {
    uint32_t structure_size, triangle_id;
    double direction_x, direction_y, direction_z;
    double endpoint_x, endpoint_y, endpoint_z, distance_meters, transmission;
    uint64_t object_id, instance_id, mesh_id, category_mask;
} xv_isovist_3d_ray;

typedef struct xv_attribution_entry {
    uint32_t structure_size, rank;
    uint64_t observer_id, target_id, object_id, category_mask;
    double blocked_weight, fraction, mean_distance_meters;
    double counterfactual_recovered_weight, counterfactual_volume_delta_cubic_meters;
} xv_attribution_entry;

typedef struct xv_category_breakdown {
    uint32_t structure_size, reserved;
    uint64_t observer_id, target_id, category_mask;
    double blocked_weight, fraction;
} xv_category_breakdown;

typedef struct xv_isovist_3d_metadata {
    uint32_t structure_size, reserved;
    uint64_t viewpoint_count, sample_count, ray_count, attribution_count, category_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_isovist_3d_metadata;

typedef struct xv_landmark_observer {
    uint64_t observer_id;
    double position_x, position_y, position_z;
    double forward_x, forward_y, forward_z;
    double up_x, up_y, up_z;
} xv_landmark_observer;

typedef struct xv_landmark {
    uint64_t landmark_id;
    double position_x, position_y, position_z, radius_meters, weight;
} xv_landmark;

typedef struct xv_landmark_options {
    uint32_t structure_size, sample_count;
    double horizontal_field_of_view_radians, vertical_field_of_view_radians;
    double maximum_distance_meters, endpoint_clearance_meters;
    uint64_t category_mask;
    uint32_t top_k, maximum_material_layers, reserved;
    double minimum_transmission;
} xv_landmark_options;

typedef struct xv_landmark_visibility_entry {
    uint32_t structure_size, flags;
    uint64_t observer_id, landmark_id;
    double distance_meters, apparent_solid_angle_steradians, visible_fraction;
    double visible_solid_angle_steradians, weighted_visibility_score;
    uint64_t dominant_blocker_object_id;
    double dominant_blocker_fraction;
} xv_landmark_visibility_entry;

typedef struct xv_landmark_metadata {
    uint32_t structure_size, reserved;
    uint64_t observer_count, landmark_count, entry_count, attribution_count, category_count;
    uint64_t analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_landmark_metadata;

typedef struct xv_solar_scenario {
    uint64_t scenario_id, material_offset, material_count, assignment_offset, assignment_count;
} xv_solar_scenario;

typedef struct xv_solar_scenario_options {
    uint32_t structure_size, top_k;
    double sensor_offset_meters, maximum_distance_meters, minimum_incidence_cosine;
    uint64_t category_mask;
    uint32_t maximum_material_layers, reserved;
    double minimum_transmission;
} xv_solar_scenario_options;

typedef struct xv_solar_scenario_summary {
    uint32_t structure_size, reserved;
    uint64_t scenario_id, sensor_id;
    double received_sun_hours, lost_sun_hours, eligible_sun_hours, solar_access_ratio;
    uint64_t dominant_occluder_object_id;
    double dominant_occluder_hours;
} xv_solar_scenario_summary;

typedef struct xv_solar_scenario_timeline_entry {
    uint32_t structure_size, state;
    double transmission;
    uint64_t first_object_id;
    uint32_t layer_count, flags;
} xv_solar_scenario_timeline_entry;

typedef struct xv_solar_attribution_entry {
    uint32_t structure_size, rank;
    uint64_t scenario_id, sensor_id, object_id, category_mask;
    double lost_sun_hours, fraction;
} xv_solar_attribution_entry;

typedef struct xv_solar_category_breakdown {
    uint32_t structure_size, reserved;
    uint64_t scenario_id, sensor_id, category_mask;
    double lost_sun_hours, fraction;
} xv_solar_category_breakdown;

typedef struct xv_solar_scenario_delta {
    uint32_t structure_size, reserved;
    uint64_t scenario_id, sensor_id;
    double received_sun_hours_delta, lost_sun_hours_delta, solar_access_ratio_delta;
} xv_solar_scenario_delta;

typedef struct xv_solar_scenario_metadata {
    uint32_t structure_size, reserved;
    uint64_t scenario_count, sensor_count, sun_count, summary_count, timeline_count;
    uint64_t attribution_count, category_count, delta_count, analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_solar_scenario_metadata;

typedef struct xv_envelope_candidate {
    uint64_t candidate_id;
    double position_x, position_y, position_z;
} xv_envelope_candidate;

typedef struct xv_oriented_envelope_candidate {
    uint64_t candidate_id;
    double position_x, position_y, position_z;
    double axis_x, axis_y, axis_z;
} xv_oriented_envelope_candidate;

typedef struct xv_solar_envelope_options {
    uint32_t structure_size, maximum_material_layers;
    double candidate_radius_meters, required_preserved_fraction, target_shaded_fraction;
    double vertical_clearance_meters, sensor_offset_meters, maximum_distance_meters;
    uint64_t category_mask;
    double minimum_transmission;
} xv_solar_envelope_options;

typedef struct xv_envelope_control {
    uint64_t sensor_id;
    int64_t unix_seconds_utc;
    double ray_elevation_meters, weighted_hours;
} xv_envelope_control;

typedef struct xv_solar_envelope_cell {
    uint32_t structure_size, reserved;
    uint64_t candidate_id;
    double position_x, position_y, position_z;
    double maximum_solar_access_elevation_meters, maximum_solar_access_height_meters;
    double minimum_shading_elevation_meters, minimum_shading_height_meters;
    double considered_baseline_sun_hours;
    uint64_t constraint_count;
    xv_envelope_control access_control, shading_control;
} xv_solar_envelope_cell;

typedef struct xv_solar_envelope_metadata {
    uint32_t structure_size, reserved;
    uint64_t sensor_count, candidate_count, sun_count, analysis_time_microseconds;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_solar_envelope_metadata;

/* XVARNA 0.15 photometric daylight and Radiance interoperability contracts. */
typedef enum xv_optical_material_kind {
    XV_OPTICAL_MATERIAL_PLASTIC = 0,
    XV_OPTICAL_MATERIAL_GLASS = 1,
    XV_OPTICAL_MATERIAL_METAL = 2,
    XV_OPTICAL_MATERIAL_TRANS = 3,
    XV_OPTICAL_MATERIAL_MIRROR = 4
} xv_optical_material_kind;

typedef enum xv_daylight_sky_model {
    XV_DAYLIGHT_SKY_ISOTROPIC = 0,
    XV_DAYLIGHT_SKY_CIE_OVERCAST = 1
} xv_daylight_sky_model;

typedef struct xv_optical_material {
    uint64_t material_id;
    uint32_t kind, reserved;
    double reflectance_red, reflectance_green, reflectance_blue;
    double transmittance_red, transmittance_green, transmittance_blue;
    double specularity, roughness, refractive_index;
} xv_optical_material;

typedef struct xv_daylight_sensor {
    uint64_t sensor_id;
    double position_x, position_y, position_z;
    double normal_x, normal_y, normal_z;
    double area_square_meters;
} xv_daylight_sensor;

typedef struct xv_daylight_matrix_options {
    uint32_t structure_size, sky_model, sky_patch_count, maximum_material_layers;
    double sensor_offset_meters, maximum_distance_meters;
    uint64_t category_mask;
    double minimum_transmission;
} xv_daylight_matrix_options;

typedef struct xv_daylight_moment {
    int64_t unix_seconds_utc;
    double sun_direction_x, sun_direction_y, sun_direction_z;
    double direct_normal_illuminance_lux, diffuse_horizontal_illuminance_lux;
} xv_daylight_moment;

typedef struct xv_point_illuminance_entry {
    uint32_t structure_size, reserved;
    uint64_t sensor_id;
    double direct_lux, diffuse_lux, total_lux, direct_transmission;
} xv_point_illuminance_entry;

typedef struct xv_daylight_metadata {
    uint32_t structure_size, flags; /* bit 0: coefficient matrix reused */
    uint64_t sensor_count, analysis_time_microseconds;
    uint64_t matrix_hash_0, matrix_hash_1, matrix_hash_2, matrix_hash_3;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_daylight_metadata;

typedef struct xv_daylight_factor_entry {
    uint32_t structure_size, reserved;
    uint64_t sensor_id;
    double interior_illuminance_lux, daylight_factor_percent;
} xv_daylight_factor_entry;

typedef struct xv_annual_daylight_options {
    uint32_t structure_size, sky_model, sky_patch_count, maximum_material_layers;
    double sensor_offset_meters, maximum_distance_meters;
    uint64_t category_mask;
    double minimum_transmission;
    double delta_t_seconds, pressure_millibars, temperature_celsius;
    double north_rotation_degrees, minimum_altitude_degrees;
    double direct_luminous_efficacy_lm_per_w, diffuse_luminous_efficacy_lm_per_w;
    double occupied_start_hour, occupied_end_hour;
    double sda_threshold_lux, sda_required_fraction;
    double ase_threshold_lux, ase_maximum_hours;
    double udi_lower_lux, udi_preferred_lux, udi_upper_lux;
} xv_annual_daylight_options;

typedef struct xv_annual_daylight_summary {
    uint32_t structure_size, flags; /* bit 0: ASE failure */
    uint64_t sensor_id;
    double occupied_hours, sda_qualified_hours, sda_occupied_fraction;
    double ase_exceedance_hours;
    double udi_below_fraction, udi_supplemental_fraction;
    double udi_useful_fraction, udi_exceeded_fraction;
    double mean_occupied_lux, minimum_occupied_lux, maximum_occupied_lux;
} xv_annual_daylight_summary;

typedef struct xv_annual_daylight_timeline_entry {
    uint32_t structure_size, flags; /* bit 0: occupied */
    int64_t unix_seconds_utc;
    double direct_lux, diffuse_lux, total_lux;
} xv_annual_daylight_timeline_entry;

typedef struct xv_annual_daylight_metadata {
    uint32_t structure_size, flags; /* bit 0: coefficient matrix reused */
    uint64_t sensor_count, weather_count, timeline_count, analysis_time_microseconds;
    double sda_area_percent, ase_area_percent;
    double udi_below_percent, udi_supplemental_percent;
    double udi_useful_percent, udi_exceeded_percent;
    double total_sensor_area_square_meters;
    uint64_t matrix_hash_0, matrix_hash_1, matrix_hash_2, matrix_hash_3;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_annual_daylight_metadata;

typedef struct xv_daylight_validation_report {
    uint32_t structure_size, flags; /* bit 0: tolerance accepted */
    uint64_t count;
    double mean_bias_lux, mean_absolute_error_lux, root_mean_square_error_lux;
    double mean_absolute_percentage_error, maximum_absolute_error_lux;
    double r_squared, accepted_fraction;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_daylight_validation_report;

typedef struct xv_radiance_export_metadata {
    uint32_t structure_size, reserved;
    uint64_t required_bytes;
    uint64_t content_hash_0, content_hash_1, content_hash_2, content_hash_3;
} xv_radiance_export_metadata;

XV_API uint32_t xv_abi_version_major(void);
XV_API uint32_t xv_abi_version_minor(void);
XV_API uint32_t xv_abi_version_patch(void);
XV_API uint32_t xv_engine_version_major(void);
XV_API uint32_t xv_engine_version_minor(void);
XV_API uint32_t xv_engine_version_patch(void);

XV_API int32_t xv_job_create(xv_job_handle *output_handle);
XV_API int32_t xv_job_cancel(xv_job_handle handle);
XV_API int32_t xv_job_progress(
    xv_job_handle handle,
    xv_job_progress_info *output_progress);
XV_API int32_t xv_job_release(xv_job_handle handle);

XV_API int32_t xv_mesh_audit(
    const double *positions,
    size_t vertex_count,
    const uint32_t *triangles,
    size_t face_count,
    double absolute_tolerance,
    double maximum_f32_error_ratio,
    xv_mesh_audit_summary *output_summary,
    uint32_t *output_face_flags,
    size_t face_flags_capacity,
    uint32_t *output_vertex_flags,
    size_t vertex_flags_capacity);

/* Call once with output_cells=NULL/cell_capacity=0 to query cell_count. */
XV_API int32_t xv_surface_grid(
    const double *positions,
    size_t vertex_count,
    const uint32_t *triangles,
    size_t face_count,
    const xv_surface_grid_options *options,
    xv_surface_grid_metadata *output_metadata,
    xv_surface_cell *output_cells,
    size_t cell_capacity);

/* Call once with both output pointers NULL and capacities zero to query counts. */
XV_API int32_t xv_surface_pv_potential(
    const xv_surface_cell *cells,
    const double *irradiance_wh_m2,
    size_t cell_count,
    const xv_pv_potential_options *options,
    xv_pv_potential_metadata *output_metadata,
    xv_pv_potential_cell *output_cells,
    size_t cell_capacity,
    xv_pv_potential_region *output_regions,
    size_t region_capacity);

XV_API int32_t xv_scene_create(
    const xv_scene_options *options,
    xv_scene_handle *output_handle);

XV_API int32_t xv_scene_add_mesh(
    xv_scene_handle handle,
    const double *positions,
    size_t vertex_count,
    const uint32_t *triangles,
    size_t face_count,
    uint64_t *output_mesh_id);

XV_API int32_t xv_scene_add_instance(
    xv_scene_handle handle,
    uint64_t mesh_id,
    const double *row_major_transform,
    uint64_t object_id,
    uint64_t instance_id,
    uint64_t category_mask);

XV_API int32_t xv_scene_add_instance_layered(
    xv_scene_handle handle,
    uint64_t mesh_id,
    const double *row_major_transform,
    uint64_t object_id,
    uint64_t instance_id,
    uint64_t category_mask,
    uint32_t layer);

XV_API int32_t xv_scene_build(
    xv_scene_handle handle,
    xv_scene_stats *output_stats);

XV_API int32_t xv_scene_get_stats(
    xv_scene_handle handle,
    xv_scene_stats *output_stats);

XV_API int32_t xv_scene_apply_instance_deltas(
    xv_scene_handle handle,
    const xv_scene_instance_delta *deltas,
    size_t delta_count,
    const xv_scene_update_options *options,
    xv_scene_update_report *output_report,
    xv_scene_stats *output_stats);

XV_API int32_t xv_scene_replace_mesh(
    xv_scene_handle handle,
    uint64_t mesh_id,
    const double *positions,
    size_t vertex_count,
    const uint32_t *triangles,
    size_t face_count,
    const xv_scene_update_options *options,
    xv_scene_update_report *output_report,
    xv_scene_stats *output_stats);

XV_API int32_t xv_scene_trace_closest(
    xv_scene_handle handle,
    const xv_ray *rays,
    size_t ray_count,
    xv_hit *output_hits,
    size_t hit_capacity);

XV_API int32_t xv_scene_trace_any(
    xv_scene_handle handle,
    const xv_ray *rays,
    size_t ray_count,
    uint8_t *output_hits,
    size_t hit_capacity);

/* Cache flags are reserved. Directory is an exact UTF-8 byte span, not nul-terminated. */
XV_API int32_t xv_cache_configure(
    const xv_cache_options *options,
    const uint8_t *directory_utf8,
    size_t directory_length);

XV_API int32_t xv_cache_get_stats(xv_cache_stats *output_stats);
XV_API int32_t xv_cache_clear(void);

/* Call with output=NULL/capacity=0 to query the adapter count. */
XV_API int32_t xv_compute_enumerate_adapters(
    xv_adapter_info *output,
    size_t capacity,
    size_t *output_count);

XV_API int32_t xv_compute_create(
    xv_scene_handle scene_handle,
    const xv_compute_options *options,
    xv_compute_handle *output_handle,
    xv_compute_info *output_info);

/* The adapter filter is case-insensitive UTF-8 and must contain 1..256 bytes. */
XV_API int32_t xv_compute_create_for_adapter(
    xv_scene_handle scene_handle,
    const xv_compute_options *options,
    const uint8_t *adapter_name_filter,
    size_t adapter_name_filter_length,
    xv_compute_handle *output_handle,
    xv_compute_info *output_info);

XV_API int32_t xv_compute_trace_closest(
    xv_compute_handle handle,
    const xv_ray *rays,
    size_t ray_count,
    xv_hit *output_hits,
    size_t hit_capacity,
    xv_compute_batch_stats *output_stats);

XV_API int32_t xv_compute_trace_any(
    xv_compute_handle handle,
    const xv_ray *rays,
    size_t ray_count,
    uint8_t *output_hits,
    size_t hit_capacity,
    xv_compute_batch_stats *output_stats);

XV_API int32_t xv_compute_get_runtime_stats(
    xv_compute_handle handle,
    xv_compute_runtime_stats *output_stats);

XV_API int32_t xv_compute_release(xv_compute_handle handle);

XV_API int32_t xv_compute_target_view(
    xv_compute_handle compute_handle,
    const xv_view_observer *observers,
    size_t observer_count,
    const xv_view_target_patch *patches,
    size_t patch_count,
    const xv_target_view_options *options,
    xv_daena_execution_info *output_execution,
    xv_target_view_metadata *output_metadata,
    xv_target_view_summary *output_summaries,
    size_t summary_capacity,
    xv_target_view_entry *output_entries,
    size_t entry_capacity);

XV_API int32_t xv_compute_view_corridor(
    xv_compute_handle compute_handle,
    const xv_view_corridor *corridors,
    size_t corridor_count,
    const xv_view_corridor_options *options,
    xv_daena_execution_info *output_execution,
    xv_view_corridor_metadata *output_metadata,
    xv_view_corridor_summary *output_summaries,
    size_t summary_capacity,
    xv_view_corridor_sample *output_samples,
    size_t sample_capacity);

XV_API int32_t xv_compute_observer_path(
    xv_compute_handle compute_handle,
    const xv_observer_path *paths,
    size_t path_count,
    const xv_point3 *vertices,
    size_t vertex_count,
    const xv_view_target_patch *patches,
    size_t patch_count,
    const xv_observer_path_options *options,
    xv_daena_execution_info *output_execution,
    xv_observer_path_metadata *output_metadata,
    xv_observer_path_summary *output_summaries,
    size_t summary_capacity,
    xv_observer_path_sample *output_samples,
    size_t sample_capacity);

XV_API int32_t xv_scene_isovist(
    xv_scene_handle scene_handle,
    const xv_viewpoint *viewpoints,
    size_t viewpoint_count,
    const xv_isovist_options *options,
    xv_isovist_metadata *output_metadata,
    xv_isovist_summary *output_summaries,
    size_t summary_capacity,
    xv_isovist_ray *output_rays,
    size_t ray_capacity);

XV_API int32_t xv_scene_intervisibility(
    xv_scene_handle scene_handle,
    const xv_visibility_observer *observers,
    size_t observer_count,
    const xv_visibility_target *targets,
    size_t target_count,
    const xv_intervisibility_options *options,
    xv_intervisibility_metadata *output_metadata,
    xv_observer_visibility_summary *output_observer_summaries,
    size_t observer_summary_capacity,
    xv_target_visibility_summary *output_target_summaries,
    size_t target_summary_capacity,
    xv_intervisibility_entry *output_entries,
    size_t entry_capacity);

XV_API int32_t xv_scene_visibility_graph(
    xv_scene_handle scene_handle,
    const xv_visibility_node *nodes,
    size_t node_count,
    const xv_visibility_graph_options *options,
    xv_visibility_graph_metadata *output_metadata,
    xv_visibility_node_metrics *output_metrics,
    size_t metric_capacity,
    xv_visibility_graph_pair *output_pairs,
    size_t pair_capacity);

XV_API int32_t xv_scene_sparse_visibility_graph(
    xv_scene_handle scene_handle,
    const xv_visibility_node *nodes,
    size_t node_count,
    const xv_sparse_visibility_graph_options *options,
    xv_visibility_graph_metadata *output_metadata,
    xv_visibility_node_metrics *output_metrics,
    size_t metric_capacity,
    xv_visibility_graph_pair *output_pairs,
    size_t pair_capacity);

XV_API int32_t xv_scene_target_view(
    xv_scene_handle scene_handle,
    const xv_view_observer *observers,
    size_t observer_count,
    const xv_view_target_patch *patches,
    size_t patch_count,
    const xv_target_view_options *options,
    xv_target_view_metadata *output_metadata,
    xv_target_view_summary *output_summaries,
    size_t summary_capacity,
    xv_target_view_entry *output_entries,
    size_t entry_capacity);

XV_API int32_t xv_scene_view_corridor(
    xv_scene_handle scene_handle,
    const xv_view_corridor *corridors,
    size_t corridor_count,
    const xv_view_corridor_options *options,
    xv_view_corridor_metadata *output_metadata,
    xv_view_corridor_summary *output_summaries,
    size_t summary_capacity,
    xv_view_corridor_sample *output_samples,
    size_t sample_capacity);

XV_API int32_t xv_scene_observer_path(
    xv_scene_handle scene_handle,
    const xv_observer_path *paths,
    size_t path_count,
    const xv_point3 *vertices,
    size_t vertex_count,
    const xv_view_target_patch *patches,
    size_t patch_count,
    const xv_observer_path_options *options,
    xv_observer_path_metadata *output_metadata,
    xv_observer_path_summary *output_summaries,
    size_t summary_capacity,
    xv_observer_path_sample *output_samples,
    size_t sample_capacity);

XV_API int32_t xv_scene_isovist_3d(
    xv_scene_handle scene_handle,
    const xv_spatial_viewpoint *viewpoints, size_t viewpoint_count,
    const xv_analysis_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_isovist_3d_options *options,
    xv_isovist_3d_metadata *output_metadata,
    xv_isovist_3d_summary *output_summaries, size_t summary_capacity,
    xv_isovist_3d_ray *output_rays, size_t ray_capacity,
    xv_attribution_entry *output_attribution, size_t attribution_capacity,
    xv_category_breakdown *output_categories, size_t category_capacity);

XV_API int32_t xv_scene_landmark_visibility(
    xv_scene_handle scene_handle,
    const xv_landmark_observer *observers, size_t observer_count,
    const xv_landmark *landmarks, size_t landmark_count,
    const xv_analysis_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_landmark_options *options,
    xv_landmark_metadata *output_metadata,
    xv_landmark_visibility_entry *output_entries, size_t entry_capacity,
    xv_attribution_entry *output_attribution, size_t attribution_capacity,
    xv_category_breakdown *output_categories, size_t category_capacity);

XV_API int32_t xv_scene_release(xv_scene_handle handle);

XV_API int32_t xv_sun_positions(
    const xv_solar_options *options,
    const xv_time_sample *times,
    size_t time_count,
    xv_sun_set_metadata *output_metadata,
    xv_sun_sample *output_samples,
    size_t sample_capacity);

XV_API int32_t xv_epw_inspect(
    const uint8_t *epw_utf8,
    size_t epw_length,
    xv_epw_metadata *output_metadata);

XV_API int32_t xv_scene_direct_sun(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors,
    size_t sensor_count,
    const xv_sun_set_metadata *sun_metadata,
    const xv_sun_sample *sun_samples,
    size_t sun_count,
    const xv_direct_sun_options *options,
    xv_direct_sun_metadata *output_metadata,
    xv_direct_sun_summary *output_summaries,
    size_t summary_capacity,
    xv_sun_timeline_entry *output_timeline,
    size_t timeline_capacity);

XV_API int32_t xv_scene_compare_solar_scenarios(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors, size_t sensor_count,
    const xv_sun_set_metadata *sun_metadata,
    const xv_sun_sample *sun_samples, size_t sun_count,
    const xv_solar_scenario *scenarios, size_t scenario_count,
    const xv_analysis_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_solar_scenario_options *options,
    xv_solar_scenario_metadata *output_metadata,
    xv_solar_scenario_summary *output_summaries, size_t summary_capacity,
    xv_solar_scenario_timeline_entry *output_timeline, size_t timeline_capacity,
    xv_solar_attribution_entry *output_attribution, size_t attribution_capacity,
    xv_solar_category_breakdown *output_categories, size_t category_capacity,
    xv_solar_scenario_delta *output_deltas, size_t delta_capacity);

XV_API int32_t xv_scene_solar_envelope(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors, size_t sensor_count,
    const xv_envelope_candidate *candidates, size_t candidate_count,
    const xv_sun_set_metadata *sun_metadata,
    const xv_sun_sample *sun_samples, size_t sun_count,
    const xv_analysis_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_solar_envelope_options *options,
    xv_solar_envelope_metadata *output_metadata,
    xv_solar_envelope_cell *output_cells, size_t cell_capacity);

XV_API int32_t xv_scene_solar_envelope_oriented(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors, size_t sensor_count,
    const xv_oriented_envelope_candidate *candidates, size_t candidate_count,
    const xv_sun_set_metadata *sun_metadata,
    const xv_sun_sample *sun_samples, size_t sun_count,
    const xv_analysis_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_solar_envelope_options *options,
    xv_solar_envelope_metadata *output_metadata,
    xv_solar_envelope_cell *output_cells, size_t cell_capacity);

XV_API int32_t xv_scene_sky_view(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors,
    size_t sensor_count,
    const xv_sky_view_options *options,
    xv_job_handle job_handle,
    xv_sky_view_metadata *output_metadata,
    xv_sky_view_summary *output_summaries,
    size_t summary_capacity,
    xv_sky_ray_entry *output_timeline,
    size_t timeline_capacity);

XV_API int32_t xv_scene_annual_irradiance(
    xv_scene_handle scene_handle,
    const xv_solar_sensor *sensors,
    size_t sensor_count,
    const uint8_t *epw_utf8,
    size_t epw_length,
    const xv_annual_irradiance_options *options,
    xv_job_handle job_handle,
    xv_annual_irradiance_metadata *output_metadata,
    xv_irradiance_summary *output_summaries,
    size_t summary_capacity,
    xv_irradiance_timeline_entry *output_timeline,
    size_t timeline_capacity);

XV_API int32_t xv_scene_point_illuminance(
    xv_scene_handle scene_handle,
    const xv_daylight_sensor *sensors, size_t sensor_count,
    const xv_optical_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const xv_daylight_moment *moment,
    const xv_daylight_matrix_options *options,
    xv_job_handle job_handle,
    xv_daylight_metadata *output_metadata,
    xv_point_illuminance_entry *output_entries, size_t entry_capacity);

XV_API int32_t xv_scene_daylight_factor(
    xv_scene_handle scene_handle,
    const xv_daylight_sensor *sensors, size_t sensor_count,
    const xv_optical_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    double exterior_horizontal_illuminance_lux,
    const xv_daylight_matrix_options *options,
    xv_daylight_metadata *output_metadata,
    xv_daylight_factor_entry *output_entries, size_t entry_capacity);

XV_API int32_t xv_scene_annual_daylight(
    xv_scene_handle scene_handle,
    const xv_daylight_sensor *sensors, size_t sensor_count,
    const xv_optical_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    const uint8_t *epw_utf8, size_t epw_length,
    const xv_annual_daylight_options *options,
    xv_job_handle job_handle,
    xv_annual_daylight_metadata *output_metadata,
    xv_annual_daylight_summary *output_summaries, size_t summary_capacity,
    xv_annual_daylight_timeline_entry *output_timeline, size_t timeline_capacity);

XV_API int32_t xv_daylight_compare(
    const double *fast_lux,
    const double *radiance_lux,
    size_t count,
    double absolute_tolerance_lux,
    double relative_tolerance,
    xv_daylight_validation_report *output_report);

/* Sections: 0 materials.rad, 1 geometry.rad, 2 sensors.pts, 3 manifest.json.
   Call with output_utf8=NULL/output_capacity=0 to query required_bytes. */
XV_API int32_t xv_scene_radiance_export_section(
    xv_scene_handle scene_handle,
    const xv_daylight_sensor *sensors, size_t sensor_count,
    const xv_optical_material *materials, size_t material_count,
    const xv_material_assignment *assignments, size_t assignment_count,
    uint32_t section,
    uint8_t *output_utf8, size_t output_capacity,
    xv_radiance_export_metadata *output_metadata);

/* Matrices are row-major. Objective directions are 0=minimize, 1=maximize.
   Constraint residuals <= 0 are feasible. */
XV_API int32_t xv_study_rank(
    const xv_variable_spec *variables,
    size_t variable_count,
    const uint32_t *objective_directions,
    size_t objective_count,
    size_t constraint_count,
    const uint64_t *candidate_ids,
    const uint64_t *generations,
    const double *parameters,
    const double *objectives,
    const double *constraints,
    size_t candidate_count,
    xv_study_metadata *output_metadata,
    xv_ranked_solution *output_solutions,
    size_t solution_capacity,
    uint64_t *output_pareto_ids,
    size_t pareto_capacity);

XV_API int32_t xv_study_spearman(
    const xv_variable_spec *variables,
    size_t variable_count,
    const uint32_t *objective_directions,
    size_t objective_count,
    size_t constraint_count,
    const uint64_t *candidate_ids,
    const double *parameters,
    const double *objectives,
    const double *constraints,
    size_t candidate_count,
    xv_sensitivity_coefficient *output_coefficients,
    size_t coefficient_capacity);

XV_API int32_t xv_study_hypervolume_2d(
    const xv_variable_spec *variables,
    size_t variable_count,
    const uint32_t *objective_directions,
    size_t constraint_count,
    const uint64_t *candidate_ids,
    const double *parameters,
    const double *objectives,
    const double *constraints,
    size_t candidate_count,
    double reference_first,
    double reference_second,
    double *output_hypervolume);

XV_API int32_t xv_optimizer_create(
    const xv_variable_spec *variables,
    size_t variable_count,
    const uint32_t *objective_directions,
    size_t objective_count,
    size_t constraint_count,
    const xv_optimizer_config *config,
    xv_optimizer_handle *output_handle);

XV_API int32_t xv_optimizer_ask(
    xv_optimizer_handle handle,
    size_t variable_count,
    xv_optimizer_metadata *output_metadata,
    uint64_t *output_candidate_ids,
    uint64_t *output_generations,
    double *output_parameters,
    size_t candidate_capacity,
    size_t parameter_capacity);

XV_API int32_t xv_optimizer_tell(
    xv_optimizer_handle handle,
    size_t objective_count,
    size_t constraint_count,
    const uint64_t *candidate_ids,
    const double *objectives,
    const double *constraints,
    size_t candidate_count,
    xv_optimizer_metadata *output_metadata,
    xv_ranked_solution *output_population,
    size_t population_capacity,
    uint64_t *output_archive_ids,
    size_t archive_capacity);

XV_API int32_t xv_optimizer_release(xv_optimizer_handle handle);

/* Every JSON output supports a sizing pass with output_utf8=NULL and output_capacity=0.
   required_bytes includes the trailing NUL byte. */
XV_API int32_t xv_study_manifest_schema_json(
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_study_workspace_create(
    const uint8_t *root_utf8, size_t root_length,
    const uint8_t *manifest_utf8, size_t manifest_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_study_workspace_status(
    const uint8_t *root_utf8, size_t root_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_study_workspace_begin_batch(
    const uint8_t *root_utf8, size_t root_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_study_workspace_commit_batch(
    const uint8_t *root_utf8, size_t root_length,
    const uint8_t *evaluations_utf8, size_t evaluations_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_study_workspace_report(
    const uint8_t *root_utf8, size_t root_length,
    const uint8_t *output_directory_utf8, size_t output_directory_length,
    const xv_study_report_options *options,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

/* RASHNU/VAHMAN 0.18 scientific-evidence JSON APIs. Every output supports a
   sizing pass; required_bytes includes the trailing NUL byte. */
XV_API int32_t xv_evidence_passport_seal_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_sensitivity_design_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_sensitivity_analyze_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_robust_scenarios_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_uncertainty_rank_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

XV_API int32_t xv_multifidelity_recommend_json(
    const uint8_t *input_utf8, size_t input_length,
    uint8_t *output_utf8, size_t output_capacity, size_t *required_bytes);

#ifdef __cplusplus
}
#endif

#endif
