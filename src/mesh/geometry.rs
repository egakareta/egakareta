/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::types::Vertex;

#[derive(Clone, Default)]
pub(crate) struct MeshGeometry {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Option<Vec<u32>>,
}

impl MeshGeometry {
    pub(crate) fn from_vertices(vertices: Vec<Vertex>) -> Self {
        Self {
            vertices,
            indices: None,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.vertices.is_empty()
            || self
                .indices
                .as_ref()
                .is_some_and(|indices| indices.is_empty())
    }

    pub(crate) fn clear(&mut self) {
        self.vertices.clear();
        self.indices = None;
    }

    pub(crate) fn append_vertices(&mut self, vertices: Vec<Vertex>) {
        if vertices.is_empty() {
            return;
        }

        let start = self.vertices.len() as u32;
        if let Some(indices) = self.indices.as_mut() {
            indices.extend(start..start + vertices.len() as u32);
        }
        self.vertices.extend(vertices);
    }

    pub(crate) fn append_indexed(&mut self, vertices: Vec<Vertex>, indices: &[u32]) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        let start = self.vertices.len() as u32;
        self.ensure_indices();
        if let Some(existing_indices) = self.indices.as_mut() {
            existing_indices.extend(indices.iter().map(|index| start + *index));
        }
        self.vertices.extend(vertices);
    }

    pub(crate) fn append_geometry(&mut self, geometry: MeshGeometry) {
        if geometry.is_empty() {
            return;
        }

        match geometry.indices {
            Some(indices) => self.append_indexed(geometry.vertices, &indices),
            None => self.append_vertices(geometry.vertices),
        }
    }

    pub(crate) fn to_triangle_vertices(&self) -> Vec<Vertex> {
        match &self.indices {
            Some(indices) => indices
                .iter()
                .filter_map(|index| self.vertices.get(*index as usize).copied())
                .collect(),
            None => self.vertices.clone(),
        }
    }

    fn ensure_indices(&mut self) {
        if self.indices.is_none() {
            self.indices = Some((0..self.vertices.len() as u32).collect());
        }
    }
}
