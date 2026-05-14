use calva::nav::HeightMap;
use core::f32;
use rand::{seq::IndexedRandom, Rng};

#[derive(Clone, Copy, Debug)]
enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

type ModuleConstraint<const SIZE: usize> = [Option<i8>; SIZE];

#[derive(Clone, Copy, Debug)]
struct ModuleConstraints<const SIZE: usize> {
    north: ModuleConstraint<SIZE>,
    east: ModuleConstraint<SIZE>,
    south: ModuleConstraint<SIZE>,
    west: ModuleConstraint<SIZE>,
}

impl<const SIZE: usize> ModuleConstraints<SIZE> {
    fn rotate(&self) -> Self {
        ModuleConstraints {
            north: self.west,
            east: self.north,
            south: self.east,
            west: self.south,
        }
    }

    fn get(&self, direction: Direction) -> ModuleConstraint<SIZE> {
        match direction {
            Direction::North => self.north,
            Direction::East => self.east,
            Direction::South => self.south,
            Direction::West => self.west,
        }
    }
}

impl<const SIZE: usize> Default for ModuleConstraints<SIZE> {
    fn default() -> Self {
        Self {
            north: [None; SIZE],
            east: [None; SIZE],
            south: [None; SIZE],
            west: [None; SIZE],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum ModuleOrientation {
    #[default]
    Zero,
    One,
    Two,
    Three,
}

impl ModuleOrientation {
    const ALL: [Self; 4] = [Self::Zero, Self::One, Self::Two, Self::Three];

    fn rotate(&self) -> Self {
        match self {
            Self::Zero => Self::One,
            Self::One => Self::Two,
            Self::Two => Self::Three,
            Self::Three => Self::Zero,
        }
    }

    fn angle(&self) -> f32 {
        match self {
            Self::Zero => 0.0,
            Self::One => -f32::consts::FRAC_PI_2,
            Self::Two => f32::consts::PI,
            Self::Three => f32::consts::FRAC_PI_2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Module<const SIZE: usize> {
    id: usize,
    constraints: ModuleConstraints<SIZE>,
    orientation: ModuleOrientation,
    elevation: i8,
}

impl<const SIZE: usize> Module<SIZE> {
    fn new<const HEIGHT_MAP_SIZE: usize>(
        id: usize,
        height_map: &HeightMap<HEIGHT_MAP_SIZE>,
    ) -> Self {
        let block_size = HEIGHT_MAP_SIZE / SIZE;
        let half_block_size = block_size / 2;

        let constraints: ModuleConstraints<SIZE> = ModuleConstraints {
            north: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(offset, 0))
                    .map(|height| height.round() as i8)
            }),
            east: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(HEIGHT_MAP_SIZE - 1, offset))
                    .map(|height| height.round() as i8)
            }),
            south: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(
                        HEIGHT_MAP_SIZE - 1 - offset,
                        HEIGHT_MAP_SIZE - 1,
                    ))
                    .map(|height| height.round() as i8)
            }),
            west: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(0, HEIGHT_MAP_SIZE - 1 - offset))
                    .map(|height| height.round() as i8)
            }),
        };

        Self {
            id,
            constraints,
            ..Default::default()
        }
    }

    fn get_constraint(&self, direction: Direction) -> ModuleConstraint<SIZE> {
        self.constraints.get(direction)
    }

    fn is_compatible(&self, other: &Self, direction: Direction) -> bool {
        let other_constraint = other.get_constraint(direction.opposite());

        self.get_constraint(direction)
            .iter()
            .rev()
            .enumerate()
            .all(|(i, c)| {
                other_constraint[i].map(|h| h + other.elevation) == c.map(|h| h + self.elevation)
            })
    }

    fn rotate(&self) -> Self {
        Module {
            constraints: self.constraints.rotate(),
            orientation: self.orientation.rotate(),
            ..*self
        }
    }

    fn build_variants(self, elevation: i8) -> [Self; const { ModuleOrientation::ALL.len() }] {
        let mut variant = self;
        variant.elevation = elevation;

        std::array::from_fn(|_| {
            variant = variant.rotate();
            variant
        })
    }
}

type GridCell<'a, const MODULE_SIZE: usize> = Vec<&'a Module<MODULE_SIZE>>;

type GridConstraint<'a, const SIZE: usize, const MODULE_SIZE: usize> =
    [GridCell<'a, MODULE_SIZE>; SIZE];

#[derive(Clone, Debug)]
struct GridConstraints<'a, const SIZE: usize, const MODULE_SIZE: usize> {
    north: GridConstraint<'a, SIZE, MODULE_SIZE>,
    east: GridConstraint<'a, SIZE, MODULE_SIZE>,
    south: GridConstraint<'a, SIZE, MODULE_SIZE>,
    west: GridConstraint<'a, SIZE, MODULE_SIZE>,
}

type Grid<'a, const SIZE: usize, const MODULE_SIZE: usize> =
    [[GridCell<'a, MODULE_SIZE>; SIZE]; SIZE];

fn propagate<const SIZE: usize, const MODULE_SIZE: usize>(
    grid: &mut Grid<SIZE, MODULE_SIZE>,
    constraints: &GridConstraints<SIZE, MODULE_SIZE>,
) {
    let mut finished = false;

    while !finished {
        finished = true;

        for (y, x) in itertools::iproduct!(0..SIZE, 0..SIZE) {
            let mut cell = grid[y][x].clone();

            cell.retain(|module| {
                Direction::ALL.iter().all(|direction| {
                    let neighbours_options = match direction {
                        Direction::North if y == 0 => &constraints.north[x],
                        Direction::East if x == SIZE - 1 => &constraints.east[y],
                        Direction::South if y == SIZE - 1 => &constraints.south[x],
                        Direction::West if x == 0 => &constraints.west[y],

                        Direction::North => &grid[y - 1][x],
                        Direction::East => &grid[y][x + 1],
                        Direction::South => &grid[y + 1][x],
                        Direction::West => &grid[y][x - 1],
                    };

                    neighbours_options
                        .iter()
                        .any(|neighbour| module.is_compatible(neighbour, *direction))
                })
            });

            if grid[y][x].len() != cell.len() {
                grid[y][x] = cell;
                finished = false;
            }
        }
    }
}

fn find_min_enthropy<const SIZE: usize, const MODULE_SIZE: usize>(
    grid: &Grid<SIZE, MODULE_SIZE>,
) -> Option<(usize, glam::USizeVec2)> {
    itertools::iproduct!(0..SIZE, 0..SIZE)
        .filter_map(|(y, x)| {
            let enthropy = grid[y][x].len();
            (enthropy > 1).then_some((enthropy, glam::usizevec2(x, y)))
        })
        .reduce(|acc, item| if item.0 < acc.0 { item } else { acc })
}

#[derive(Clone, Copy, Debug)]
pub struct WfcConstraints<const SIZE: usize, const MODULE_SIZE: usize> {
    pub north: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub east: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub south: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub west: [[Option<f32>; MODULE_SIZE]; SIZE],
}

pub struct WfcConfig<const SIZE: usize, const MODULE_SIZE: usize> {
    pub constraints: WfcConstraints<SIZE, MODULE_SIZE>,
    pub elevations: usize,
    pub elevations_increments: i8,
    pub rng: Box<dyn Rng>,
}

pub fn wfc<'t, const SIZE: usize, const MODULE_SIZE: usize>(
    mut config: WfcConfig<SIZE, MODULE_SIZE>,
    height_maps: &mut impl Iterator<Item = (usize, &'t HeightMap)>,
) -> [[(usize, f32, i8); SIZE]; SIZE] {
    let modules = height_maps
        .map(|(id, height_map)| Module::<MODULE_SIZE>::new(id, height_map))
        .flat_map(|module| {
            (0..(config.elevations))
                .map(|i| i as i8 * config.elevations_increments)
                .flat_map(|elevation| module.build_variants(elevation))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut grid: Grid<SIZE, MODULE_SIZE> =
        std::array::from_fn(|_| std::array::from_fn(|_| modules.iter().collect()));

    let (north, east, south, west) = (
        config.constraints.north.map(|mut c| {
            c.reverse();
            vec![Module {
                constraints: ModuleConstraints {
                    south: c.map(|o| o.map(|h| h as i8)),
                    ..Default::default()
                },
                ..Default::default()
            }]
        }),
        config.constraints.east.map(|mut c| {
            c.reverse();
            vec![Module {
                constraints: ModuleConstraints {
                    west: c.map(|o| o.map(|h| h as i8)),
                    ..Default::default()
                },
                ..Default::default()
            }]
        }),
        config.constraints.south.map(|c| {
            vec![Module {
                constraints: ModuleConstraints {
                    north: c.map(|o| o.map(|h| h as i8)),
                    ..Default::default()
                },
                ..Default::default()
            }]
        }),
        config.constraints.west.map(|c| {
            vec![Module {
                constraints: ModuleConstraints {
                    east: c.map(|o| o.map(|h| h as i8)),
                    ..Default::default()
                },
                ..Default::default()
            }]
        }),
    );

    let constraints = GridConstraints {
        north: std::array::from_fn(|i| north[i].iter().collect()),
        east: std::array::from_fn(|i| east[i].iter().collect()),
        south: std::array::from_fn(|i| south[i].iter().collect()),
        west: std::array::from_fn(|i| west[i].iter().collect()),
    };

    propagate(&mut grid, &constraints); // propagate grid constraints first
    while let Some((_, coord)) = find_min_enthropy(&grid) {
        let collapse = *grid[coord.y][coord.x].choose(&mut config.rng).unwrap();
        grid[coord.y][coord.x] = vec![collapse];
        propagate(&mut grid, &constraints);
    }

    std::array::from_fn(|y| {
        std::array::from_fn(|x| {
            let module = grid[y][x].choose(&mut config.rng).unwrap();
            let angle = module.orientation.angle();
            let elevation = module.elevation;

            (module.id, angle, elevation)
        })
    })
}
