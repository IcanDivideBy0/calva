use colored::{Colorize, CustomColor};
use itertools::Itertools;

pub fn debug_map<T, const W: usize, const H: usize, C: Into<CustomColor>>(
    map: &[[T; W]; H],
    color: impl Fn(&T) -> C,
) -> String {
    let (row_pairs, rest) = map.as_chunks::<2>();

    row_pairs
        .iter()
        .map(|[row_up, row_down]| {
            std::iter::zip(row_up, row_down)
                .map(|(up, down)| {
                    "▀"
                        .custom_color(color(up))
                        .on_custom_color(color(down))
                        .to_string()
                })
                .collect::<String>()
        })
        .chain(rest.first().map(|row| {
            row.iter()
                .map(|cell| "▀".custom_color(color(cell)).to_string())
                .collect::<String>()
        }))
        .join("\n")
}
