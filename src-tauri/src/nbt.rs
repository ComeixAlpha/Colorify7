//! 流式 NBT 写入

use std::collections::HashMap;
use std::io::{self, Write};

const TAG_END: u8 = 0;
const TAG_INT: u8 = 3;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;

pub struct NbtWriter<'a, W: Write> {
    w: &'a mut W,
}

impl<'a, W: Write> NbtWriter<'a, W> {
    pub fn new(w: &'a mut W) -> Self {
        Self { w }
    }

    #[inline]
    fn write_u8(&mut self, b: u8) -> io::Result<()> {
        self.w.write_all(&[b])
    }

    #[inline]
    fn write_u16(&mut self, v: u16) -> io::Result<()> {
        self.w.write_all(&v.to_le_bytes())
    }

    #[inline]
    fn write_i32(&mut self, v: i32) -> io::Result<()> {
        self.w.write_all(&v.to_le_bytes())
    }

    fn write_string_data(&mut self, s: &str) -> io::Result<()> {
        let bytes = s.as_bytes();
        self.write_u16(bytes.len() as u16)?;
        self.w.write_all(bytes)
    }

    pub fn compound_header(&mut self, name: &str) -> io::Result<()> {
        self.write_u8(TAG_COMPOUND)?;
        self.write_string_data(name)
    }

    pub fn compound_end(&mut self) -> io::Result<()> {
        self.write_u8(TAG_END)
    }

    pub fn int_tag(&mut self, name: &str, value: i32) -> io::Result<()> {
        self.write_u8(TAG_INT)?;
        self.write_string_data(name)?;
        self.write_i32(value)
    }

    pub fn string_tag(&mut self, name: &str, value: &str) -> io::Result<()> {
        self.write_u8(TAG_STRING)?;
        self.write_string_data(name)?;
        self.write_string_data(value)
    }

    pub fn list_header(&mut self, name: &str, element_type: u8, len: i32) -> io::Result<()> {
        self.write_u8(TAG_LIST)?;
        self.write_string_data(name)?;
        self.write_u8(element_type)?;
        self.write_i32(len)
    }

    pub fn nameless_list_header(&mut self, element_type: u8, len: i32) -> io::Result<()> {
        self.write_u8(element_type)?;
        self.write_i32(len)
    }

    pub fn nameless_int(&mut self, value: i32) -> io::Result<()> {
        self.write_i32(value)
    }

    pub fn int_list(&mut self, data: &[i32]) -> io::Result<()> {
        const CHUNK: usize = 1 << 16; // 65536 个 i32 = 256KB
        let mut buf = [0u8; CHUNK * 4];
        for chunk in data.chunks(CHUNK) {
            for (i, &v) in chunk.iter().enumerate() {
                buf[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            self.w.write_all(&buf[..chunk.len() * 4])?;
        }
        Ok(())
    }

    pub fn int_neg_ones(&mut self, count: usize) -> io::Result<()> {
        const CHUNK: usize = 1 << 16;
        let neg = [-1i32; CHUNK];
        let mut left = count;
        while left > 0 {
            let n = left.min(CHUNK);
            self.int_list(&neg[..n])?;
            left -= n;
        }
        Ok(())
    }
}

pub struct McStructure {
    size: [i32; 3],
    block_indices: Vec<i32>,
    palette: Vec<String>,
    palette_map: HashMap<String, usize>,
}

impl McStructure {
    pub fn new(size: [i32; 3]) -> Self {
        let amount = (size[0].max(0) as usize)
            .saturating_mul(size[1].max(0) as usize)
            .saturating_mul(size[2].max(0) as usize);
        McStructure {
            size,
            block_indices: vec![-1; amount],
            palette: Vec::new(),
            palette_map: HashMap::new(),
        }
    }

    pub fn set_block(&mut self, pos: [i32; 3], type_id: &str) {
        let (x, y, z) = (pos[0], pos[1], pos[2]);
        if x > self.size[0] || y > self.size[1] || z > self.size[2] || x < 0 || y < 0 || z < 0 {
            return;
        }
        let idx = Self::index_from_pos(self.size, x, y, z);
        if idx >= self.block_indices.len() {
            return;
        }
        let palette_index = self.palette_index(type_id);
        self.block_indices[idx] = palette_index as i32;
    }

    fn index_from_pos(size: [i32; 3], x: i32, y: i32, z: i32) -> usize {
        let mut amount: i64 = 0;
        if x > 0 {
            amount += (x as i64 - 1) * size[1] as i64 * size[2] as i64;
        }
        if y > 0 {
            amount += (y as i64 - 1) * size[2] as i64;
        }
        if z > 0 {
            amount += z as i64 - 1;
        }
        amount as usize
    }

    fn palette_index(&mut self, type_id: &str) -> usize {
        if let Some(&i) = self.palette_map.get(type_id) {
            return i;
        }
        let i = self.palette.len();
        self.palette.push(type_id.to_string());
        self.palette_map.insert(type_id.to_string(), i);
        i
    }

    /// 流式写出
    pub fn write_file<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let mut n = NbtWriter::new(w);

        // Root
        n.compound_header("")?;

        n.int_tag("format_version", 1)?;

        n.list_header("size", TAG_INT, 3)?;
        for &s in &self.size {
            n.nameless_int(s)?;
        }

        n.compound_header("structure")?;

        n.list_header("block_indices", TAG_LIST, 2)?;
        n.nameless_list_header(TAG_INT, self.block_indices.len() as i32)?;
        n.int_list(&self.block_indices)?;
        n.nameless_list_header(TAG_INT, self.block_indices.len() as i32)?;
        n.int_neg_ones(self.block_indices.len())?;

        n.list_header("entities", TAG_END, 0)?;

        n.compound_header("palette")?;
        n.compound_header("default")?;

        n.list_header("block_palette", TAG_COMPOUND, self.palette.len() as i32)?;
        for type_id in &self.palette {
            n.string_tag("name", type_id)?;
            n.compound_header("states")?;
            n.compound_end()?;
            n.int_tag("version", 18090528)?;
            n.compound_end()?;
        }

        n.compound_header("block_position_data")?;
        n.compound_end()?;

        n.compound_end()?; // default
        n.compound_end()?; // palette
        n.compound_end()?; // structure

        n.list_header("structure_world_origin", TAG_INT, 3)?;
        n.nameless_int(2)?;
        n.nameless_int(2)?;
        n.nameless_int(2)?;

        n.compound_end()?; // root

        Ok(())
    }
}
