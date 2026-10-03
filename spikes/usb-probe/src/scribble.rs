pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 64;

pub struct Frame {
    lit: [bool; WIDTH * HEIGHT],
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Self {
        Self {
            lit: [false; WIDTH * HEIGHT],
        }
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x < WIDTH && y < HEIGHT {
            self.lit[y * WIDTH + x] = true;
        }
    }

    // Format read from goxlr-utility (scribbles/src/lib.rs, MIT):
    // 8 bands of 8 rows, one byte per column, bit = row within the band,
    // bit cleared = point lit.
    pub fn encode(&self) -> [u8; 1024] {
        let mut bytes = [0xFF_u8; 1024];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if self.lit[y * WIDTH + x] {
                    bytes[WIDTH * (y / 8) + x] &= !(1 << (y % 8));
                }
            }
        }
        bytes
    }
}

// Vertical bar, 8 points wide, bouncing from side to side.
pub fn animation_frame(step: u32) -> Frame {
    const BAR: usize = 8;
    let travel = WIDTH - BAR;
    let cycle = (step as usize) % (2 * travel);
    let left = if cycle <= travel {
        cycle
    } else {
        2 * travel - cycle
    };

    let mut frame = Frame::new();
    for y in 0..HEIGHT {
        for x in left..left + BAR {
            frame.set(x, y);
        }
    }
    frame
}

pub fn clamp_fps(requested: u32) -> u32 {
    requested.clamp(1, 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frame_is_all_ones() {
        assert_eq!(Frame::new().encode(), [0xFF; 1024]);
    }

    #[test]
    fn top_left_point_clears_bit_0_of_byte_0() {
        let mut frame = Frame::new();
        frame.set(0, 0);
        let bytes = frame.encode();
        assert_eq!(bytes[0], 0xFE);
        assert!(bytes[1..].iter().all(|b| *b == 0xFF));
    }

    #[test]
    fn bottom_right_point_clears_bit_7_of_last_byte() {
        let mut frame = Frame::new();
        frame.set(127, 63);
        assert_eq!(frame.encode()[1023], 0x7F);
    }

    #[test]
    fn point_on_second_row_band_lands_in_second_block() {
        let mut frame = Frame::new();
        frame.set(5, 9);
        assert_eq!(frame.encode()[128 + 5], 0xFD);
    }

    #[test]
    fn out_of_range_point_is_ignored() {
        let mut frame = Frame::new();
        frame.set(128, 0);
        frame.set(0, 64);
        assert_eq!(frame.encode(), [0xFF; 1024]);
    }

    #[test]
    fn animation_moves_between_steps() {
        assert_ne!(animation_frame(0).encode(), animation_frame(1).encode());
    }

    #[test]
    fn animation_always_lights_a_full_height_bar() {
        for step in [0, 1, 119, 120, 500] {
            let lit = animation_frame(step)
                .encode()
                .iter()
                .map(|b| b.count_zeros())
                .sum::<u32>();
            assert_eq!(lit, 8 * 64, "step {step}");
        }
    }

    #[test]
    fn fps_is_clamped_between_1_and_60() {
        assert_eq!(clamp_fps(0), 1);
        assert_eq!(clamp_fps(30), 30);
        assert_eq!(clamp_fps(1000), 60);
    }
}
