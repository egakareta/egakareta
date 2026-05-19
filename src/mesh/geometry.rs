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

    pub(crate) fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub(crate) fn draw_count(&self) -> usize {
        self.indices.as_ref().map_or(self.vertices.len(), Vec::len)
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

#[cfg(test)]
mod tests {
    use super::MeshGeometry;
    use crate::types::Vertex;

    fn vertex(x: f32) -> Vertex {
        Vertex::untextured([x, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0])
    }

    #[test]
    fn append_indexed_offsets_indices_by_existing_vertices() {
        let mut geometry = MeshGeometry::from_vertices(vec![vertex(0.0), vertex(1.0)]);

        geometry.append_indexed(vec![vertex(2.0), vertex(3.0), vertex(4.0)], &[2, 0, 1]);

        assert_eq!(geometry.vertex_count(), 5);
        assert_eq!(geometry.draw_count(), 5);
        assert_eq!(geometry.indices, Some(vec![0, 1, 4, 2, 3]));
    }

    #[test]
    fn append_geometry_keeps_unindexed_meshes_unindexed() {
        let mut geometry = MeshGeometry::default();
        geometry.append_geometry(MeshGeometry::from_vertices(vec![vertex(0.0), vertex(1.0)]));

        assert_eq!(geometry.vertex_count(), 2);
        assert_eq!(geometry.draw_count(), 2);
        assert!(geometry.indices.is_none());
    }

    #[test]
    fn clear_resets_vertices_and_indices() {
        let mut geometry = MeshGeometry {
            vertices: vec![vertex(0.0)],
            indices: Some(vec![0]),
        };

        geometry.clear();

        assert!(geometry.is_empty());
        assert!(geometry.vertices.is_empty());
        assert!(geometry.indices.is_none());
    }
}
