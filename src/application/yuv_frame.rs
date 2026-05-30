//! YUV 4:2:0 (I420) video frame DTO carried across the `FrameSink` port.
//!
//! Owns plane bytes so the value can cross threads without lifetime
//! gymnastics. Strides are bytes-per-row, which may exceed `width` (Y) or
//! `width/2` (U/V) when the producer adds row padding.

pub struct YuvFrame {
    pub width: u32,
    pub height: u32,
    pub y_plane: Vec<u8>,
    pub u_plane: Vec<u8>,
    pub v_plane: Vec<u8>,
    pub y_stride: u32,
    pub u_stride: u32,
    pub v_stride: u32,
}

impl YuvFrame {
    #[cfg(test)]
    pub fn chroma_width(&self) -> u32 {
        self.width.div_ceil(2)
    }

    pub fn chroma_height(&self) -> u32 {
        self.height.div_ceil(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_frame(w: u32, h: u32) -> YuvFrame {
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        YuvFrame {
            width: w,
            height: h,
            y_plane: vec![0u8; (w * h) as usize],
            u_plane: vec![0u8; (cw * ch) as usize],
            v_plane: vec![0u8; (cw * ch) as usize],
            y_stride: w,
            u_stride: cw,
            v_stride: cw,
        }
    }

    #[test]
    fn chroma_dims_for_even_resolution() {
        let f = sample_frame(1920, 1080);
        assert_eq!(f.chroma_width(), 960);
        assert_eq!(f.chroma_height(), 540);
    }

    #[test]
    fn chroma_dims_round_up_for_odd_resolution() {
        let f = sample_frame(1281, 721);
        assert_eq!(f.chroma_width(), 641);
        assert_eq!(f.chroma_height(), 361);
    }

    #[test]
    fn plane_lengths_match_stride_times_height() {
        let f = sample_frame(640, 480);
        assert_eq!(f.y_plane.len(), (f.y_stride * f.height) as usize);
        assert_eq!(f.u_plane.len(), (f.u_stride * f.chroma_height()) as usize);
        assert_eq!(f.v_plane.len(), (f.v_stride * f.chroma_height()) as usize);
    }
}
