#[derive(Debug, Clone)]
struct Chunk {
    position: glam::IVec2,
    slots: [[Slot; Self::SIZE]; Self::SIZE],
}

impl Chunk {
    pub const SIZE: usize = 32;
}

#[derive(Debug, Clone)]
struct Slot {
    position: glam::IVec3,
    options: Vec<(Module, Orientation)>,
}

impl Slot {
    fn new(position: glam::IVec3) -> Self {
        Self {
            position,
            options: Module::all().into_iter().zip(Orientation::all()).collect(),
        }
    }

    fn entropy(&self) -> u32 {
        self.options.len() as _
    }

    fn collapsed(&self) -> bool {
        self.entropy() == 1
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Module {
    Empty,
    Floor,
    SmallStairs,
}

impl Module {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::Empty, Self::Floor, Self::SmallStairs]
    }

    fn connectors(&self, face: Face) -> Vec<Connector> {
        match self {
            Self::Empty => vec![Connector::Empty],
            Self::Floor => match face {
                Face::Top | Face::Bottom => vec![Connector::Empty],
                _ => vec![Connector::Empty, Connector::Floor],
            },
            Self::SmallStairs => match face {
                Face::Top | Face::Bottom => vec![Connector::Empty],
                Face::Up => vec![Connector::Floor],
                _ => vec![Connector::Empty, Connector::Floor],
            },
        }
    }

    fn allowed(&self, face: Face) -> Vec<(Self, Orientation)> {
        let connectors = self.connectors(face);

        Self::all()
            .into_iter()
            .flat_map(|module| Orientation::all().into_iter())
            .map(|(module)| {})
            .collect()
    }
}

enum Connector {
    Empty,
    Floor,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Face {
    Up,
    Down,
    Left,
    Right,
    Top,
    Bottom,
}

impl Face {
    const fn all() -> impl IntoIterator<Item = Self> {
        [
            Self::Up,
            Self::Down,
            Self::Left,
            Self::Right,
            Self::Top,
            Self::Bottom,
        ]
    }

    const fn rotate(&self, rotation: Rotation) -> Option<Self> {
        match self {
            Self::Up => Some(match rotation {
                Rotation::North => *self,
                Rotation::East => Self::Right,
                Rotation::South => Self::Down,
                Rotation::West => Self::Left,
            }),
            Self::Down => Some(match rotation {
                Rotation::North => *self,
                Rotation::East => Self::Left,
                Rotation::South => Self::Up,
                Rotation::West => Self::Right,
            }),
            Self::Left => Some(match rotation {
                Rotation::North => *self,
                Rotation::East => Self::Up,
                Rotation::South => Self::Right,
                Rotation::West => Self::Down,
            }),
            Self::Right => Some(match rotation {
                Rotation::North => *self,
                Rotation::East => *self,
                Rotation::South => *self,
                Rotation::West => *self,
            }),
            Self::Top => None,
            Self::Bottom => None,
        }
    }

    const fn opposite(&self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Rotation {
    North,
    East,
    South,
    West,
}

impl Rotation {
    const fn all() -> impl IntoIterator<Item = Self> {
        [Self::North, Self::East, Self::South, Self::West]
    }

    fn random() -> Self {
        match rand::random::<u32>() % 4 {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            _ => Self::West,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungen() {
        dbg!(Module::Empty.allowed(Face::Bottom));
    }
}
