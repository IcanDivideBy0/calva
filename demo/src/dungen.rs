use noise::NoiseFn;
use rand::prelude::*;

pub struct Dungen;
// https://en.wikipedia.org/wiki/Box-drawing_character
// https://vazgriz.com/119/procedurally-generated-dungeons/

impl Dungen {
    const WORLD_SIZE: u32 = 50;

    pub fn gen() {
        let mut rng = rand::thread_rng();

        let noise = noise::Value::new(rng.gen::<u32>());
        // let scale = 0.275;
        let scale = 0.0275;

        // let noise: noise::Billow<noise::Value> = noise::Billow::new(rng.gen::<u32>());
        // let scale = 10.0;

        // let noise: noise::Fbm<noise::Value> = noise::Fbm::new(rng.gen::<u32>());
        // let scale = 10.0;

        // let noise = noise::Checkerboard::new(1);
        // let scale = 10.0;

        let width = Self::WORLD_SIZE as usize;

        println!("╭{:─<width$}╮", "");
        for x in 0..Self::WORLD_SIZE {
            let mut s = String::new();

            for y in 0..Self::WORLD_SIZE {
                let x = x as f64 * scale;
                let y = y as f64 * scale;

                let elevation: u32 = ((noise.get([x, y]) * 0.5 + 0.5) * 7.0) as _;
                // let elevation: u32 = (noise.get([x, y]) * 5.0) as _;

                s += match elevation {
                    // 6.. => "█",
                    5.. => "▓",
                    4.. => "▒",
                    2.. => "░",
                    _ => " ",
                };
            }

            println!("│{s}│");
        }
        println!("╰{:─<width$}╯", "");
    }
}
