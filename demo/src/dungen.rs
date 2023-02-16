use rand::prelude::*;
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct Room {
    pos: glam::UVec2,
    size: glam::UVec2,
}

impl Room {
    const SIZE_RANGE: Range<u32> = 2..8;

    fn rand(rng: &mut impl Rng) -> Self {
        let mut size = glam::uvec2(
            rng.gen_range(Self::SIZE_RANGE),
            rng.gen_range(Self::SIZE_RANGE),
        );
        size += 1 - size % 2;

        let pos = glam::uvec2(
            rng.gen_range(0..(Dungen::WORLD_SIZE - size.x)),
            rng.gen_range(0..(Dungen::WORLD_SIZE - size.y)),
        );

        Self { pos, size }
    }

    fn contain(&self, p: glam::UVec2) -> bool {
        p.x >= self.pos.x
            && p.x < self.pos.x + self.size.x
            && p.y >= self.pos.y
            && p.y < self.pos.y + self.size.y
    }

    fn overlap(&self, other: &Self) -> bool {
        !(((other.pos.x >= self.pos.x + self.size.x + 1)
            || (other.pos.x + other.size.x <= self.pos.x.saturating_sub(1)))
            || ((other.pos.y >= self.pos.y + self.size.y + 1)
                || (other.pos.y + other.size.y <= self.pos.y.saturating_sub(1))))
    }
}

pub struct Dungen;
// https://en.wikipedia.org/wiki/Box-drawing_character
// https://vazgriz.com/119/procedurally-generated-dungeons/

impl Dungen {
    const WORLD_SIZE: u32 = 25;
    const ROOMS_COUNT: u32 = 10;

    pub fn gen() {
        let mut rng = rand::thread_rng();
        let mut rooms: Vec<Room> = Vec::with_capacity(Self::ROOMS_COUNT as _);

        let mut it = 0;
        'a: while rooms.len() < Self::ROOMS_COUNT as _ && it < 200 {
            it += 1;

            let room = Room::rand(&mut rng);

            for r in &rooms {
                if room.overlap(r) {
                    continue 'a;
                }
            }

            rooms.push(room);
        }

        dbg!(rooms.len());

        let mut s = String::from("╭") + &"─".repeat(Self::WORLD_SIZE as _) + "╮\n";
        for x in 0_u32..Self::WORLD_SIZE {
            s += "│";

            'y: for y in 0_u32..Self::WORLD_SIZE {
                for room in &rooms {
                    if room.contain(glam::uvec2(x, y)) {
                        s += "█";
                        continue 'y;
                    }
                }
                s += " ";
            }
            s += "│\n";
        }
        s += "╰";
        s += "─".repeat(Self::WORLD_SIZE as _).as_str();
        s += "╯";

        println!("{s}");
    }
}
