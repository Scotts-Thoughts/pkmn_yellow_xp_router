//! RGBA pixel buffers and the box downsampling used for the LOD pyramid.

#[derive(Debug, Clone)]
pub struct Pixmap {
    pub w: usize,
    pub h: usize,
    /// RGBA8, row-major, no padding
    pub data: Vec<u8>,
}

impl Pixmap {
    pub fn new(w: usize, h: usize) -> Pixmap {
        Pixmap { w, h, data: vec![0; w * h * 4] }
    }

    pub fn filled(w: usize, h: usize, rgba: [u8; 4]) -> Pixmap {
        let mut p = Pixmap::new(w, h);
        p.fill(rgba);
        p
    }

    pub fn fill(&mut self, rgba: [u8; 4]) {
        for px in self.data.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
    }

    #[inline]
    pub fn put(&mut self, x: usize, y: usize, rgba: [u8; 4]) {
        let i = (y * self.w + x) * 4;
        self.data[i..i + 4].copy_from_slice(&rgba);
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.w + x) * 4;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    /// Copy a rectangle of `src` (src_x, src_y, w, h) to (dst_x, dst_y),
    /// clipped to both images. Opaque copy (no blending).
    pub fn blit(&mut self, dst_x: i32, dst_y: i32, src: &Pixmap, src_x: i32, src_y: i32, w: i32, h: i32) {
        for row in 0..h {
            let sy = src_y + row;
            let dy = dst_y + row;
            if sy < 0 || dy < 0 || sy >= src.h as i32 || dy >= self.h as i32 {
                continue;
            }
            let x_start = (-dst_x).max(-src_x).max(0);
            let x_end = w.min(self.w as i32 - dst_x).min(src.w as i32 - src_x);
            if x_end <= x_start {
                continue;
            }
            let si = ((sy as usize) * src.w + (src_x + x_start) as usize) * 4;
            let di = ((dy as usize) * self.w + (dst_x + x_start) as usize) * 4;
            let n = (x_end - x_start) as usize * 4;
            self.data[di..di + n].copy_from_slice(&src.data[si..si + n]);
        }
    }

    /// Box-downsample `src` by `factor` into this image at (dst_x, dst_y).
    pub fn downsample_into(&mut self, dst_x: usize, dst_y: usize, src: &Pixmap, factor: usize) {
        let ow = src.w / factor;
        let oh = src.h / factor;
        let n = (factor * factor) as u32;
        for oy in 0..oh {
            if dst_y + oy >= self.h {
                break;
            }
            for ox in 0..ow {
                if dst_x + ox >= self.w {
                    break;
                }
                let mut acc = [0u32; 4];
                for sy in 0..factor {
                    let row = ((oy * factor + sy) * src.w + ox * factor) * 4;
                    for sx in 0..factor {
                        let i = row + sx * 4;
                        acc[0] += src.data[i] as u32;
                        acc[1] += src.data[i + 1] as u32;
                        acc[2] += src.data[i + 2] as u32;
                        acc[3] += src.data[i + 3] as u32;
                    }
                }
                self.put(dst_x + ox, dst_y + oy, [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, (acc[3] / n) as u8]);
            }
        }
    }

    /// Encode as PNG (for goldens and the export feature).
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        use image::ImageEncoder;
        let mut out = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut out);
        enc.write_image(&self.data, self.w as u32, self.h as u32, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
        Ok(out)
    }
}
