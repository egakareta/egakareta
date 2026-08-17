/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Bevy material infrastructure for block presentation.
//!
//! This is intentionally separate from the temporary custom-wgpu
//! [`super::plugin::RenderPlugin`]. The full-Bevy root application installs
//! [`BlockMaterialPlugin`] alongside `DefaultPlugins`; at that point block
//! chunks can use `Mesh3d` + `MeshMaterial3d<BlockMaterial>` directly.

use bevy::{
    app::{App, Plugin, Startup},
    asset::{Asset, AssetPath, Assets, Handle, RenderAssetUsages, embedded_asset, embedded_path},
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Mesh, MeshVertexBufferLayoutRef},
    pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin},
    prelude::{Image, ResMut},
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
        TextureDataOrder, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
    shader::ShaderRef,
};
use eg_blocks::BlockTextureAtlas;
use eg_mesh::{
    ATTRIBUTE_ANIMATION_ANCHOR, ATTRIBUTE_ANIMATION_PARAMS, ATTRIBUTE_ANIMATION_PARAMS2,
    ATTRIBUTE_COLOR_OUTLINE, ATTRIBUTE_RENDER_PROFILE, ATTRIBUTE_TEXTURE_LAYER, ATTRIBUTE_UV_NORM,
};

use super::bevy_presentation::BlockPresentationMaterial;

/// Texture-array material used by generated block chunks.
///
/// A texture array keeps all block layers in one material and one bind group,
/// preserving the batching property of the existing renderer. The source mesh
/// supplies the selected layer and all editor/animation attributes.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BlockMaterial {
    /// Texture array containing the block atlas layers.
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub texture_array: Handle<Image>,
}

impl Material for BlockMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("block_material.wgsl")).with_source("embedded"),
        )
    }

    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ATTRIBUTE_UV_NORM.at_shader_location(3),
            ATTRIBUTE_TEXTURE_LAYER.at_shader_location(4),
            ATTRIBUTE_COLOR_OUTLINE.at_shader_location(5),
            ATTRIBUTE_RENDER_PROFILE.at_shader_location(6),
            ATTRIBUTE_ANIMATION_ANCHOR.at_shader_location(7),
            ATTRIBUTE_ANIMATION_PARAMS.at_shader_location(8),
            ATTRIBUTE_ANIMATION_PARAMS2.at_shader_location(9),
        ])?];
        Ok(())
    }
}

/// Registers the embedded block shader and the matching Bevy material type.
///
/// Add this only in the full-Bevy root composition, after `DefaultPlugins`
/// has installed the asset and render sub-apps.
pub struct BlockMaterialPlugin;

impl Plugin for BlockMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "block_material.wgsl");
        app.add_plugins(MaterialPlugin::<BlockMaterial>::default());
    }
}

/// Creates the generated block texture-array image and its shared material.
///
/// Install after [`BlockMaterialPlugin`] and the Bevy asset plugins. The
/// material handle is retained by [`BlockPresentationMaterial`], which keeps
/// the image and material resident for all block chunk entities.
pub struct BlockMaterialAssetsPlugin;

impl Plugin for BlockMaterialAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, initialize_block_material_assets_system);
    }
}

fn initialize_block_material_assets_system(
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BlockMaterial>>,
    mut presentation_material: ResMut<BlockPresentationMaterial>,
) {
    if presentation_material.0.is_some() {
        return;
    }

    let texture_array = images.add(build_block_texture_array_image());
    presentation_material.0 = Some(materials.add(BlockMaterial { texture_array }));
}

/// Build the generated block atlas as a Bevy 2D texture-array image.
///
/// The custom renderer avoids an exact multiple of six layers for WebGL
/// compatibility. Retain that policy here so WebGPU and forced-WebGL select
/// identical layers.
pub fn build_block_texture_array_image() -> Image {
    let atlas = BlockTextureAtlas::get();
    let layer_count = block_texture_array_layer_count(atlas.layers.len());
    let bytes_per_layer = (atlas.width * atlas.height * 4) as usize;
    let mut data = vec![0; bytes_per_layer * layer_count as usize];
    for (layer_index, layer) in atlas.layers.iter().enumerate() {
        let start = layer_index * bytes_per_layer;
        data[start..start + bytes_per_layer].copy_from_slice(&layer.rgba);
    }

    let mut sampler = ImageSamplerDescriptor::nearest();
    sampler.set_address_mode(ImageAddressMode::Repeat);
    Image {
        data: Some(data),
        data_order: TextureDataOrder::default(),
        texture_descriptor: TextureDescriptor {
            label: Some("Egakareta block texture array"),
            size: Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: layer_count,
            },
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        sampler: ImageSampler::Descriptor(sampler),
        texture_view_descriptor: None,
        asset_usage: RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        copy_on_resize: false,
    }
}

fn block_texture_array_layer_count(source_layers: usize) -> u32 {
    let mut layer_count = source_layers.max(2) as u32;
    if layer_count.is_multiple_of(6) {
        layer_count += 1;
    }
    layer_count
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::{AssetApp, AssetPlugin, Assets, Handle},
        prelude::Image,
    };

    use super::{
        BlockMaterial, BlockMaterialAssetsPlugin, block_texture_array_layer_count,
        build_block_texture_array_image,
    };
    use crate::state::BlockPresentationMaterial;

    #[test]
    fn block_material_keeps_a_strong_texture_array_handle() {
        let material = BlockMaterial {
            texture_array: Handle::<Image>::default(),
        };

        assert_eq!(material.texture_array, Handle::<Image>::default());
    }

    #[test]
    fn generated_texture_array_preserves_atlas_dimensions_and_webgl_padding() {
        let image = build_block_texture_array_image();
        let atlas = eg_blocks::BlockTextureAtlas::get();

        assert_eq!(image.texture_descriptor.size.width, atlas.width);
        assert_eq!(image.texture_descriptor.size.height, atlas.height);
        assert_eq!(
            image.texture_descriptor.size.depth_or_array_layers,
            block_texture_array_layer_count(atlas.layers.len())
        );
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some(
                (atlas.width * atlas.height * 4) as usize
                    * block_texture_array_layer_count(atlas.layers.len()) as usize
            )
        );
    }

    #[test]
    fn material_asset_startup_installs_shared_presentation_handle() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<BlockMaterial>()
            .init_resource::<BlockPresentationMaterial>()
            .add_plugins(BlockMaterialAssetsPlugin);

        app.update();

        let handle = app
            .world()
            .resource::<BlockPresentationMaterial>()
            .0
            .clone()
            .expect("startup must create the block material");
        assert!(
            app.world()
                .resource::<Assets<BlockMaterial>>()
                .get(&handle)
                .is_some()
        );
    }
}
