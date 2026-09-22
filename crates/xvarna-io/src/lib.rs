//! Auditable mesh import and export for XVARNA.

#![forbid(unsafe_code)]

mod gltf;
mod ply;
mod stl;

use core::fmt;
use std::fmt::Write;
use xvarna_geometry::{Mesh, Vec3};

pub use gltf::{
    GltfError, GltfImportReport, parse_glb, parse_gltf, write_glb, write_gltf_embedded,
};
pub use ply::{
    PlyEncoding, PlyError, PlyImportReport, parse_ply, write_ply_binary, write_ply_text,
};
pub use stl::{StlError, StlImportReport, parse_stl, write_stl_binary, write_stl_text};

/// Successful OBJ import with explicit conversion statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjImportReport {
    /// Canonical triangle mesh.
    pub mesh: Mesh,
    /// Number of source OBJ face statements.
    pub source_face_count: usize,
    /// Number of quad faces deterministically split into two triangles.
    pub triangulated_quad_count: usize,
    /// Number of syntactically valid but unsupported directive lines ignored.
    pub ignored_directive_count: usize,
}

/// Structured OBJ parse error with source line attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjError {
    /// One-based source line number.
    pub line: usize,
    /// Specific parse failure.
    pub kind: ObjErrorKind,
}

impl fmt::Display for ObjError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "OBJ line {}: {}", self.line, self.kind)
    }
}

impl std::error::Error for ObjError {}

/// Specific OBJ parse failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjErrorKind {
    /// A vertex did not contain three valid finite coordinates.
    InvalidVertex,
    /// A face did not contain exactly three or four valid references.
    InvalidFace,
    /// OBJ vertex index zero is invalid.
    ZeroIndex,
    /// A face referenced a vertex not yet defined in the OBJ stream.
    IndexOutOfRange(i64),
    /// Polygons above four vertices require an explicit triangulation policy.
    UnsupportedPolygon(usize),
    /// The mesh exceeds the canonical u32 index range.
    TooManyVertices,
}

impl fmt::Display for ObjErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVertex => formatter.write_str("invalid or non-finite vertex"),
            Self::InvalidFace => {
                formatter.write_str("face must contain three or four valid vertex references")
            }
            Self::ZeroIndex => formatter.write_str("vertex index 0 is forbidden by the OBJ format"),
            Self::IndexOutOfRange(index) => {
                write!(formatter, "vertex index {index} is out of range")
            }
            Self::UnsupportedPolygon(count) => write!(
                formatter,
                "{count}-gon requires an explicit triangulation policy; only triangles and quads are accepted"
            ),
            Self::TooManyVertices => {
                formatter.write_str("vertex count exceeds the canonical u32 index range")
            }
        }
    }
}

/// Parses OBJ vertices and triangle/quad faces into a canonical mesh.
///
/// Position, texture, and normal face forms are accepted; texture and normal
/// references are ignored. Positive and relative-negative vertex indices are
/// supported. N-gons above four vertices fail rather than being silently fan
/// triangulated because a fan can corrupt concave polygons.
pub fn parse_obj(source: &str) -> Result<ObjImportReport, ObjError> {
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    let mut source_face_count = 0;
    let mut triangulated_quad_count = 0;
    let mut ignored_directive_count = 0;

    for (zero_based_line, source_line) in source.lines().enumerate() {
        let line_number = zero_based_line + 1;
        let line_without_comment = source_line.split('#').next().unwrap_or_default().trim();
        if line_without_comment.is_empty() {
            continue;
        }
        let mut tokens = line_without_comment.split_whitespace();
        let directive = tokens.next().unwrap_or_default();
        match directive {
            "v" => positions.push(parse_vertex(tokens, line_number)?),
            "f" => {
                source_face_count += 1;
                let references = tokens
                    .map(|token| parse_face_index(token, positions.len(), line_number))
                    .collect::<Result<Vec<_>, _>>()?;
                match references.as_slice() {
                    [first, second, third] => triangles.push([*first, *second, *third]),
                    [first, second, third, fourth] => {
                        triangles.push([*first, *second, *third]);
                        triangles.push([*first, *third, *fourth]);
                        triangulated_quad_count += 1;
                    }
                    [] | [_] | [_, _] => {
                        return Err(ObjError {
                            line: line_number,
                            kind: ObjErrorKind::InvalidFace,
                        });
                    }
                    polygon => {
                        return Err(ObjError {
                            line: line_number,
                            kind: ObjErrorKind::UnsupportedPolygon(polygon.len()),
                        });
                    }
                }
            }
            "o" | "g" | "s" | "usemtl" | "mtllib" | "vt" | "vn" => {
                ignored_directive_count += 1;
            }
            _ => ignored_directive_count += 1,
        }
    }

    Ok(ObjImportReport {
        mesh: Mesh {
            positions,
            triangles,
        },
        source_face_count,
        triangulated_quad_count,
        ignored_directive_count,
    })
}

/// One nonempty `o` section in an OBJ scene, in source face order.
/// Material assignment uses its one-based position in the returned array.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjSceneObject {
    /// Source object label (not required to be unique).
    pub name: String,
    /// Compact local mesh retaining exact positions and winding.
    pub mesh: Mesh,
}

/// Preserves OBJ object identity while reusing the canonical validated parser.
/// `g` remains a grouping hint inside an object; `usemtl` is not an optical catalog.
pub fn parse_obj_scene(source: &str) -> Result<Vec<ObjSceneObject>, ObjError> {
    let imported = parse_obj(source)?;
    let mut sections: Vec<(String, Vec<[u32; 3]>)> = vec![("default".into(), Vec::new())];
    let mut triangle_offset = 0;
    for line in source.lines() {
        let mut tokens = line
            .split('#')
            .next()
            .unwrap_or_default()
            .split_whitespace();
        match tokens.next() {
            Some("o") => sections.push((tokens.collect::<Vec<_>>().join(" "), Vec::new())),
            Some("f") => {
                let count = tokens.count() - 2; // parse_obj already rejects invalid faces.
                sections
                    .last_mut()
                    .expect("default section")
                    .1
                    .extend_from_slice(
                        &imported.mesh.triangles[triangle_offset..triangle_offset + count],
                    );
                triangle_offset += count;
            }
            _ => {}
        }
    }
    Ok(sections
        .into_iter()
        .filter(|(_, faces)| !faces.is_empty())
        .map(|(name, faces)| {
            let mut remap = std::collections::BTreeMap::new();
            let mut positions = Vec::new();
            let triangles = faces
                .into_iter()
                .map(|face| {
                    face.map(|old| {
                        *remap.entry(old).or_insert_with(|| {
                            let index = u32::try_from(positions.len())
                                .expect("unique source vertex IDs fit u32");
                            positions.push(imported.mesh.positions[old as usize]);
                            index
                        })
                    })
                })
                .collect();
            ObjSceneObject {
                name,
                mesh: Mesh {
                    positions,
                    triangles,
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod scene_identity_tests {
    use super::*;
    #[test]
    fn objects_share_global_vertices_without_losing_material_identity() {
        let source =
            "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\no roof\nf 1 2 3 4\no empty\no glass\nf -4 -3 -2\n";
        let parts = parse_obj_scene(source).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name, "roof");
        assert_eq!(parts[0].mesh.triangles.len(), 2);
        assert_eq!(parts[1].name, "glass");
        assert_eq!(parts[1].mesh.positions.len(), 3);
        assert_eq!(parts[1].mesh.positions[0], Vec3::new(0., 0., 0.));
    }
    #[test]
    fn unnamed_faces_and_repeated_names_have_stable_distinct_ids() {
        let parts =
            parse_obj_scene("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\no a\nf 1 2 3\no a\nf 1 2 3\n")
                .unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].name, "default");
    }
}

fn parse_vertex<'a>(
    mut tokens: impl Iterator<Item = &'a str>,
    line: usize,
) -> Result<Vec3, ObjError> {
    let invalid = || ObjError {
        line,
        kind: ObjErrorKind::InvalidVertex,
    };
    let x = tokens
        .next()
        .ok_or_else(invalid)?
        .parse::<f64>()
        .map_err(|_| invalid())?;
    let y = tokens
        .next()
        .ok_or_else(invalid)?
        .parse::<f64>()
        .map_err(|_| invalid())?;
    let z = tokens
        .next()
        .ok_or_else(invalid)?
        .parse::<f64>()
        .map_err(|_| invalid())?;
    let point = Vec3::new(x, y, z);
    if !point.is_finite() {
        return Err(invalid());
    }
    Ok(point)
}

fn parse_face_index(token: &str, vertex_count: usize, line: usize) -> Result<u32, ObjError> {
    let raw = token
        .split('/')
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(ObjError {
            line,
            kind: ObjErrorKind::InvalidFace,
        })?;
    if raw == 0 {
        return Err(ObjError {
            line,
            kind: ObjErrorKind::ZeroIndex,
        });
    }

    let vertex_count_i64 = i64::try_from(vertex_count).map_err(|_| ObjError {
        line,
        kind: ObjErrorKind::TooManyVertices,
    })?;
    let zero_based = if raw > 0 {
        raw - 1
    } else {
        vertex_count_i64.checked_add(raw).ok_or(ObjError {
            line,
            kind: ObjErrorKind::IndexOutOfRange(raw),
        })?
    };
    if zero_based < 0 || zero_based >= vertex_count_i64 {
        return Err(ObjError {
            line,
            kind: ObjErrorKind::IndexOutOfRange(raw),
        });
    }
    u32::try_from(zero_based).map_err(|_| ObjError {
        line,
        kind: ObjErrorKind::TooManyVertices,
    })
}

/// Serializes a canonical triangle mesh as a deterministic OBJ string.
#[must_use]
pub fn write_obj(mesh: &Mesh) -> String {
    let mut output = String::from("# XVARNA canonical triangle mesh\n");
    for position in &mesh.positions {
        writeln!(
            &mut output,
            "v {:.17} {:.17} {:.17}",
            position.x, position.y, position.z
        )
        .expect("writing to String cannot fail");
    }
    for triangle in &mesh.triangles {
        writeln!(
            &mut output,
            "f {} {} {}",
            u64::from(triangle[0]) + 1,
            u64::from(triangle[1]) + 1,
            u64::from(triangle[2]) + 1
        )
        .expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_and_quad_import_with_relative_indices() {
        let source = "\
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1/1/1 2/2/1 3/3/1
f -4 -2 -1 -3
";
        let report = parse_obj(source).expect("OBJ is valid");
        assert_eq!(report.mesh.positions.len(), 4);
        assert_eq!(report.source_face_count, 2);
        assert_eq!(report.triangulated_quad_count, 1);
        assert_eq!(report.mesh.triangles, vec![[0, 1, 2], [0, 2, 3], [0, 3, 1]]);
    }

    #[test]
    fn concave_ngon_is_not_silently_fan_triangulated() {
        let source = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0.5 0.5 0\nv 0 1 0\nf 1 2 3 4 5\n";
        let error = parse_obj(source).expect_err("n-gon must be rejected");
        assert_eq!(error.line, 6);
        assert_eq!(error.kind, ObjErrorKind::UnsupportedPolygon(5));
    }

    #[test]
    fn zero_and_out_of_range_indices_are_rejected_with_line_number() {
        let zero = parse_obj("v 0 0 0\nf 0 1 1\n").expect_err("zero is invalid");
        assert_eq!(zero.line, 2);
        assert_eq!(zero.kind, ObjErrorKind::ZeroIndex);

        let range = parse_obj("v 0 0 0\nf 1 2 1\n").expect_err("two is absent");
        assert_eq!(range.kind, ObjErrorKind::IndexOutOfRange(2));
    }

    #[test]
    fn writer_round_trips_canonical_triangle_mesh() {
        let source = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let first = parse_obj(source).expect("source parses");
        let serialized = write_obj(&first.mesh);
        let second = parse_obj(&serialized).expect("written OBJ parses");
        assert_eq!(first.mesh, second.mesh);
    }
}
