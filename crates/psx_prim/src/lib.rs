use psx_vram::{Clut, TexPage, TextureWindow};

pub const MAX_POLYGON_WIDTH: i32 = 1023;
pub const MAX_POLYGON_HEIGHT: i32 = 511;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vertex {
    pub x: i16,
    pub y: i16,
    pub color: [u8; 3],
    pub u: u8,
    pub v: u8,
}

impl Vertex {
    pub const fn new(x: i16, y: i16) -> Self {
        Self {
            x,
            y,
            color: [128, 128, 128],
            u: 0,
            v: 0,
        }
    }

    pub const fn with_color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }

    pub const fn with_uv(mut self, u: u8, v: u8) -> Self {
        self.u = u;
        self.v = v;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Half,
    Add,
    Subtract,
    AddQuarter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture {
    pub page: TexPage,
    pub clut: Clut,
    pub raw: bool,
    pub window: Option<TextureWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Triangle {
        vertices: [Vertex; 3],
        gouraud: bool,
        texture: Option<Texture>,
        blend: Option<BlendMode>,
    },
    Quad {
        vertices: [Vertex; 4],
        gouraud: bool,
        texture: Option<Texture>,
        blend: Option<BlendMode>,
    },
    Sprite {
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        u: u8,
        v: u8,
        color: [u8; 3],
        texture: Option<Texture>,
        blend: Option<BlendMode>,
    },
    Line {
        vertices: [Vertex; 2],
        gouraud: bool,
        blend: Option<BlendMode>,
    },
}

impl Primitive {
    pub fn vertices(&self) -> &[Vertex] {
        match self {
            Self::Triangle { vertices, .. } => vertices,
            Self::Quad { vertices, .. } => vertices,
            Self::Line { vertices, .. } => vertices,
            Self::Sprite { .. } => &[],
        }
    }
}

pub fn exceeds_gpu_limits(vertices: &[Vertex]) -> bool {
    let (min_x, max_x) = vertices
        .iter()
        .fold((i32::MAX, i32::MIN), |(low, high), v| {
            (low.min(v.x as i32), high.max(v.x as i32))
        });
    let (min_y, max_y) = vertices
        .iter()
        .fold((i32::MAX, i32::MIN), |(low, high), v| {
            (low.min(v.y as i32), high.max(v.y as i32))
        });
    max_x - min_x > MAX_POLYGON_WIDTH || max_y - min_y > MAX_POLYGON_HEIGHT
}

pub fn near_reject(screen_z: &[u16], near: u16) -> bool {
    screen_z.iter().any(|&z| z < near)
}

pub fn signed_area(a: Vertex, b: Vertex, c: Vertex) -> i32 {
    let (ax, ay) = (a.x as i32, a.y as i32);
    let (bx, by) = (b.x as i32, b.y as i32);
    let (cx, cy) = (c.x as i32, c.y as i32);
    ax * by + bx * cy + cx * ay - ax * cy - bx * ay - cx * by
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cull {
    None,
    BackFace,
}

pub fn accept(vertices: &[Vertex], screen_z: &[u16], near: u16, cull: Cull) -> bool {
    if near_reject(screen_z, near) || exceeds_gpu_limits(vertices) {
        return false;
    }
    match cull {
        Cull::None => true,
        Cull::BackFace => signed_area(vertices[0], vertices[1], vertices[2]) > 0,
    }
}

const END: u32 = u32::MAX;

pub struct OrderingTable {
    heads: Vec<u32>,
    next: Vec<u32>,
    primitives: Vec<Primitive>,
    shift: u32,
}

impl OrderingTable {
    pub fn new(buckets: usize, shift: u32) -> Self {
        assert!(
            (1..=4096).contains(&buckets),
            "ordering table size must be 1..=4096"
        );
        Self {
            heads: vec![END; buckets],
            next: Vec::new(),
            primitives: Vec::new(),
            shift,
        }
    }

    pub fn len(&self) -> usize {
        self.heads.len()
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    pub fn shift(&self) -> u32 {
        self.shift
    }

    pub fn set_shift(&mut self, shift: u32) {
        self.shift = shift;
    }

    pub fn clear(&mut self) {
        self.heads.fill(END);
        self.next.clear();
        self.primitives.clear();
    }

    pub fn bucket_for(&self, otz: u32) -> usize {
        ((otz >> self.shift) as usize).min(self.heads.len() - 1)
    }

    pub fn add_at(&mut self, bucket: usize, primitive: Primitive) {
        let bucket = bucket.min(self.heads.len() - 1);
        let index = self.primitives.len() as u32;
        self.primitives.push(primitive);
        self.next.push(self.heads[bucket]);
        self.heads[bucket] = index;
    }

    pub fn add(&mut self, otz: u32, primitive: Primitive) {
        self.add_at(self.bucket_for(otz), primitive);
    }

    pub fn draw_order(&self) -> impl Iterator<Item = &Primitive> {
        self.heads.iter().rev().flat_map(move |&head| {
            std::iter::successors((head != END).then_some(head), move |&index| {
                let next = self.next[index as usize];
                (next != END).then_some(next)
            })
            .map(move |index| &self.primitives[index as usize])
        })
    }

    pub fn flatten(&self, out: &mut Vec<Primitive>) {
        out.clear();
        out.extend(self.draw_order().copied());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle(x: i16) -> Primitive {
        Primitive::Triangle {
            vertices: [
                Vertex::new(x, 0),
                Vertex::new(x + 10, 0),
                Vertex::new(x, 10),
            ],
            gouraud: false,
            texture: None,
            blend: None,
        }
    }

    fn first_x(primitive: &Primitive) -> i16 {
        primitive.vertices()[0].x
    }

    #[test]
    fn near_plane_rejects_whole_polygon() {
        let vertices = [Vertex::new(0, 0), Vertex::new(10, 0), Vertex::new(0, 10)];
        assert!(!accept(&vertices, &[500, 20, 500], 32, Cull::None));
        assert!(accept(&vertices, &[500, 32, 500], 32, Cull::None));
    }

    #[test]
    fn oversized_polygons_rejected() {
        let wide = [
            Vertex::new(-512, 0),
            Vertex::new(512, 0),
            Vertex::new(0, 10),
        ];
        let fits = [
            Vertex::new(-512, 0),
            Vertex::new(511, 0),
            Vertex::new(0, 511),
        ];
        let tall = [
            Vertex::new(0, -256),
            Vertex::new(10, 256),
            Vertex::new(0, 0),
        ];
        assert!(exceeds_gpu_limits(&wide));
        assert!(!exceeds_gpu_limits(&fits));
        assert!(exceeds_gpu_limits(&tall));
        assert!(!accept(&wide, &[100; 3], 1, Cull::None));
    }

    #[test]
    fn backface_culling_uses_winding() {
        let clockwise = [Vertex::new(0, 0), Vertex::new(10, 0), Vertex::new(0, 10)];
        let counter = [Vertex::new(0, 0), Vertex::new(0, 10), Vertex::new(10, 0)];
        assert!(accept(&clockwise, &[100; 3], 1, Cull::BackFace));
        assert!(!accept(&counter, &[100; 3], 1, Cull::BackFace));
        assert!(accept(&counter, &[100; 3], 1, Cull::None));
    }

    #[test]
    fn draws_back_to_front_lifo_within_bucket() {
        let mut table = OrderingTable::new(16, 4);
        table.add(10, triangle(1));
        table.add(200, triangle(2));
        table.add(12, triangle(3));
        table.add(1000, triangle(4));
        let order: Vec<i16> = table.draw_order().map(first_x).collect();
        assert_eq!(order, vec![4, 2, 3, 1]);
        table.clear();
        assert_eq!(table.draw_order().count(), 0);
    }

    #[test]
    fn intersecting_quads_sort_wrong() {
        let mut table = OrderingTable::new(1024, 2);
        let near_but_long = [10u32, 10, 400, 400];
        let crossing = [190u32, 190, 230, 230];
        let average = |z: [u32; 4]| z.iter().sum::<u32>() / 4;
        table.add(average(near_but_long), triangle(1));
        table.add(average(crossing), triangle(2));
        let order: Vec<i16> = table.draw_order().map(first_x).collect();
        assert_eq!(
            order,
            vec![2, 1],
            "the long quad wins despite its far half sitting behind the crossing quad"
        );
    }
}
