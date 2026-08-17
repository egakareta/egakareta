/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    XNeg,
    YNeg,
    ZNeg,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoDragKind {
    Move,
    Resize,
    Rotate,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GizmoPart {
    MoveX,
    MoveY,
    MoveZ,
    MoveXNeg,
    MoveYNeg,
    MoveZNeg,
    ResizeX,
    ResizeY,
    ResizeZ,
    ResizeXNeg,
    ResizeYNeg,
    ResizeZNeg,
    RotateX,
    RotateY,
    RotateZ,
}

impl GizmoPart {
    pub fn from_axis_kind(axis: GizmoAxis, kind: GizmoDragKind) -> Self {
        match (axis, kind) {
            (GizmoAxis::X, GizmoDragKind::Move) => GizmoPart::MoveX,
            (GizmoAxis::Y, GizmoDragKind::Move) => GizmoPart::MoveY,
            (GizmoAxis::Z, GizmoDragKind::Move) => GizmoPart::MoveZ,
            (GizmoAxis::XNeg, GizmoDragKind::Move) => GizmoPart::MoveXNeg,
            (GizmoAxis::YNeg, GizmoDragKind::Move) => GizmoPart::MoveYNeg,
            (GizmoAxis::ZNeg, GizmoDragKind::Move) => GizmoPart::MoveZNeg,
            (GizmoAxis::X, GizmoDragKind::Resize) => GizmoPart::ResizeX,
            (GizmoAxis::Y, GizmoDragKind::Resize) => GizmoPart::ResizeY,
            (GizmoAxis::Z, GizmoDragKind::Resize) => GizmoPart::ResizeZ,
            (GizmoAxis::XNeg, GizmoDragKind::Resize) => GizmoPart::ResizeXNeg,
            (GizmoAxis::YNeg, GizmoDragKind::Resize) => GizmoPart::ResizeYNeg,
            (GizmoAxis::ZNeg, GizmoDragKind::Resize) => GizmoPart::ResizeZNeg,
            (GizmoAxis::X | GizmoAxis::XNeg, GizmoDragKind::Rotate) => GizmoPart::RotateX,
            (GizmoAxis::Y | GizmoAxis::YNeg, GizmoDragKind::Rotate) => GizmoPart::RotateY,
            (GizmoAxis::Z | GizmoAxis::ZNeg, GizmoDragKind::Rotate) => GizmoPart::RotateZ,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
    pub uv_norm: [f32; 2],
    pub texture_layer: f32,
    pub color_outline: [f32; 4],
    pub render_profile: f32,
    pub animation_anchor: [f32; 4],
    pub animation_params: [f32; 4],
    pub animation_params2: [f32; 4],
}

impl Vertex {
    pub fn untextured(position: [f32; 3], color: [f32; 4]) -> Self {
        Self {
            position,
            color,
            uv: [0.0, 0.0],
            uv_norm: [0.0, 0.0],
            texture_layer: -1.0,
            color_outline: [0.0, 0.0, 0.0, 0.0],
            render_profile: 0.0,
            animation_anchor: [0.0, 0.0, 0.0, 0.0],
            animation_params: [0.0, 0.0, 0.0, 0.0],
            animation_params2: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn textured(position: [f32; 3], color: [f32; 4], uv: [f32; 2], texture_layer: u32) -> Self {
        Self {
            position,
            color,
            uv,
            uv_norm: [0.0, 0.0],
            texture_layer: texture_layer as f32,
            color_outline: [0.0, 0.0, 0.0, 0.0],
            render_profile: 0.0,
            animation_anchor: [0.0, 0.0, 0.0, 0.0],
            animation_params: [0.0, 0.0, 0.0, 0.0],
            animation_params2: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn textured_with_outline(
        position: [f32; 3],
        color: [f32; 4],
        uv: [f32; 2],
        uv_norm: [f32; 2],
        texture_layer: u32,
        color_outline: [f32; 4],
    ) -> Self {
        Self {
            position,
            color,
            uv,
            uv_norm,
            texture_layer: texture_layer as f32,
            color_outline,
            render_profile: 0.0,
            animation_anchor: [0.0, 0.0, 0.0, 0.0],
            animation_params: [0.0, 0.0, 0.0, 0.0],
            animation_params2: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn set_render_profile(&mut self, render_profile: f32) {
        self.render_profile = render_profile;
    }

    pub fn set_animation(&mut self, anchor: [f32; 4], params: [f32; 4], params2: [f32; 4]) {
        self.animation_anchor = anchor;
        self.animation_params = params;
        self.animation_params2 = params2;
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x4,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32,
            5 => Float32x4,
            6 => Float32,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlockInstance {
    pub model_rows: [[f32; 4]; 3],
    pub color_tint_and_noise: [f32; 4],
    pub animation_anchor: [f32; 4],
}

impl BlockInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            10 => Float32x4,
            11 => Float32x4,
            12 => Float32x4,
            13 => Float32x4,
            14 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BlockInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRS,
        }
    }

    pub fn transform_position(&self, local_position: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = local_position;
        [
            self.model_rows[0][0] * x
                + self.model_rows[0][1] * y
                + self.model_rows[0][2] * z
                + self.model_rows[0][3],
            self.model_rows[1][0] * x
                + self.model_rows[1][1] * y
                + self.model_rows[1][2] * z
                + self.model_rows[1][3],
            self.model_rows[2][0] * x
                + self.model_rows[2][1] * y
                + self.model_rows[2][2] * z
                + self.model_rows[2][3],
        ]
    }
}

pub mod advanced_shapes;
pub mod blocks;
pub mod builders;
pub mod chunked;
pub mod geometry;
pub mod lighting;
pub mod noise;
pub mod shapes;
#[cfg(test)]
mod tests;
pub mod transforms;

pub use blocks::*;
pub use builders::*;
pub use chunked::*;
pub use geometry::*;
