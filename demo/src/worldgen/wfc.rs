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
    fn rotate(&mut self) {
        *self = ModuleConstraints {
            north: self.west,
            east: self.north,
            south: self.east,
            west: self.south,
        };

        self.north.reverse();
        self.south.reverse();
    }

    fn elevate(&mut self, elevation: i8) {
        for constraint in [
            &mut self.north,
            &mut self.east,
            &mut self.south,
            &mut self.west,
        ] {
            for h in constraint {
                *h = h.map(|h| h + elevation);
            }
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
pub enum ModuleRotation {
    #[default]
    Cw0,
    Cw90,
    Cw180,
    Cw270,
}

impl ModuleRotation {
    const ALL: [Self; 4] = [Self::Cw0, Self::Cw90, Self::Cw180, Self::Cw270];

    fn rotate(&mut self) {
        *self = match self {
            Self::Cw0 => Self::Cw90,
            Self::Cw90 => Self::Cw180,
            Self::Cw180 => Self::Cw270,
            Self::Cw270 => Self::Cw0,
        };
    }

    pub fn angle(&self) -> f32 {
        match self {
            Self::Cw0 => 0.0,
            Self::Cw90 => -f32::consts::FRAC_PI_2,
            Self::Cw180 => f32::consts::PI,
            Self::Cw270 => f32::consts::FRAC_PI_2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Module<const SIZE: usize> {
    pub id: usize,
    pub rotation: ModuleRotation,
    pub elevation: i8,

    constraints: ModuleConstraints<SIZE>,
}

impl<const SIZE: usize> Module<SIZE> {
    fn new(id: usize, height_map: &HeightMap) -> Self {
        let block_size = HeightMap::SIZE / SIZE;
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
                    .get_height(&glam::usizevec2(HeightMap::SIZE - 1, offset))
                    .map(|height| height.round() as i8)
            }),
            south: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(offset, HeightMap::SIZE - 1))
                    .map(|height| height.round() as i8)
            }),
            west: std::array::from_fn(|i| {
                let offset = i * block_size + half_block_size;
                height_map
                    .get_height(&glam::usizevec2(0, offset))
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
        self.get_constraint(direction) == other.get_constraint(direction.opposite())
    }

    fn rotate(&mut self) {
        self.constraints.rotate();
        self.rotation.rotate();
    }

    fn elevate(&mut self, elevation: i8) {
        self.constraints.elevate(elevation);
        self.elevation += elevation;
    }

    fn build_variants(&self, elevation: i8) -> [Self; const { ModuleRotation::ALL.len() }] {
        let mut variant = *self;

        variant.elevate(-self.elevation);
        variant.elevate(elevation);

        std::array::from_fn(|_| {
            variant.rotate();
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

#[derive(Clone, Copy, Debug)]
pub struct WfcConstraints<const SIZE: usize, const MODULE_SIZE: usize> {
    pub north: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub east: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub south: [[Option<f32>; MODULE_SIZE]; SIZE],
    pub west: [[Option<f32>; MODULE_SIZE]; SIZE],
}

pub struct Wfc<const SIZE: usize, const MODULE_SIZE: usize> {
    pub elevations: usize,
    pub elevations_increments: i8,

    modules: Vec<Module<MODULE_SIZE>>,
}

impl<const SIZE: usize, const MODULE_SIZE: usize> Wfc<SIZE, MODULE_SIZE> {
    pub fn new<'a>(
        elevations: usize,
        elevations_increments: i8,
        height_maps: &mut impl Iterator<Item = (&'a usize, &'a HeightMap)>,
    ) -> Self {
        Self {
            elevations,
            elevations_increments,

            modules: height_maps
                .map(|(id, height_map)| Module::<MODULE_SIZE>::new(*id, height_map))
                .flat_map(|module| {
                    (0..=elevations)
                        .map(|i| i as i8 * elevations_increments)
                        .flat_map(|elevation| module.build_variants(elevation))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        }
    }

    pub fn collapse(
        &self,
        constraints: WfcConstraints<SIZE, MODULE_SIZE>,
        rng: &mut impl Rng,
    ) -> [[Module<MODULE_SIZE>; SIZE]; SIZE] {
        let mut grid: Grid<SIZE, MODULE_SIZE> =
            std::array::from_fn(|_| std::array::from_fn(|_| self.modules.iter().collect()));

        let (north, east, south, west) = (
            constraints.north.map(|c| {
                vec![Module {
                    constraints: ModuleConstraints {
                        south: c.map(|o| o.map(|h| h as i8)),
                        ..Default::default()
                    },
                    ..Default::default()
                }]
            }),
            constraints.east.map(|c| {
                vec![Module {
                    constraints: ModuleConstraints {
                        west: c.map(|o| o.map(|h| h as i8)),
                        ..Default::default()
                    },
                    ..Default::default()
                }]
            }),
            constraints.south.map(|c| {
                vec![Module {
                    constraints: ModuleConstraints {
                        north: c.map(|o| o.map(|h| h as i8)),
                        ..Default::default()
                    },
                    ..Default::default()
                }]
            }),
            constraints.west.map(|c| {
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

        Self::propagate(&mut grid, &constraints); // propagate grid constraints first
        while let Some((_, coord)) = Self::find_min_enthropy(&grid) {
            let collapse = *grid[coord.y][coord.x].choose(rng).unwrap();
            grid[coord.y][coord.x] = vec![collapse];
            Self::propagate(&mut grid, &constraints);
        }

        grid.map(|row| row.map(|cell| **cell.choose(rng).unwrap()))
    }

    fn propagate(
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

    fn find_min_enthropy(grid: &Grid<SIZE, MODULE_SIZE>) -> Option<(usize, glam::USizeVec2)> {
        itertools::iproduct!(0..SIZE, 0..SIZE)
            .filter_map(|(y, x)| {
                let enthropy = grid[y][x].len();
                (enthropy > 1).then_some((enthropy, glam::usizevec2(x, y)))
            })
            .reduce(|acc, item| if item.0 < acc.0 { item } else { acc })
    }
}
