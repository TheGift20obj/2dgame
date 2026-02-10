use bevy::prelude::*;
use bevy::mesh::{Mesh, VertexAttributeValues, Indices, PrimitiveTopology};
use crate::resourses::physics_resources::*;

use rapier2d::prelude::*;
use rapier2d::na::Point2;

pub struct ObjectsLoaderPlugin;

impl Plugin for ObjectsLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init)
            .add_systems(Update, inspect);
    }
}

fn init(
    mut commands: Commands,
    mut atlas_handles: ResMut<AtlasHandles>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let handle_0 = AnimationIndices { first: 0, last: 3 };
    atlas_handles.0.insert("walk".to_string(), handle_0);
    let handle_1 = AnimationIndices { first: 4, last: 8 };
    atlas_handles.0.insert("attack".to_string(), handle_1);
}

fn inspect(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    query: Query<(Entity, &Mesh2d, &Transform, Option<&Player>, Option<&Wall>, Option<&Floor>), With<Pending>>,
    mut rigid_bodies: ResMut<ResRigidBodySet>,
    mut colliders: ResMut<ResColliderSet>,
) {
    for (entity, mesh_handle, transform, player, wall, floor) in &query {
        if let Some(mesh) = meshes.get(&mesh_handle.0) {
            if let Some((vertices, indices)) = handle_mesh(mesh, transform) {
                let rigid_body = if player.is_some() {
                    RigidBodyBuilder::dynamic().soft_ccd_prediction(0.0).lock_rotations()
                } else if wall.is_some() || floor.is_some() {
                    RigidBodyBuilder::fixed()
                } else {
                    RigidBodyBuilder::dynamic().soft_ccd_prediction(0.0).lock_rotations()
                }.translation(vector![transform.translation.x, transform.translation.y])
                    .build();
                let rb_handle = rigid_bodies.0.insert(rigid_body);
                let collider = ColliderBuilder::trimesh_with_flags(vertices, indices, TriMeshFlags::MERGE_DUPLICATE_VERTICES).expect("REASON")
                    .restitution(0.0)
                    .friction(0.5)
                    .restitution_combine_rule(CoefficientCombineRule::Average)
                    .friction_combine_rule(CoefficientCombineRule::Average)
                    .build();
                let col_handle = colliders.0.insert_with_parent(collider, rb_handle, &mut rigid_bodies.0);
                commands.entity(entity).insert((
                    RigidBodyHandleComponent(rb_handle),
                ));
                commands.entity(entity).remove::<Pending>();
            }
        }
    }
}

fn handle_mesh(mesh: &Mesh, transform: &Transform) -> Option<(Vec<Point2<f32>>, Vec<[u32; 3]>)> {
    let scale = transform.scale;

    let positions: Vec<Point2<f32>> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|&[x, y, _z]| {
                Point2::new(
                    x * scale.x,
                    y * scale.y,
                )
            })
            .collect(),
        _ => return None,
    };

    let indices: Vec<u32> = match mesh.indices() {
        Some(Indices::U32(indices)) => indices.clone(),
        Some(Indices::U16(indices)) => indices.iter().map(|&i| i as u32).collect(),
        _ => return None,
    };

    let triangles: Vec<[u32; 3]> = indices
        .chunks(3)
        .filter_map(|chunk| {
            if chunk.len() == 3 {
                Some([chunk[0], chunk[1], chunk[2]])
            } else {
                None
            }
        })
        .collect();

    Some((positions, triangles))
}