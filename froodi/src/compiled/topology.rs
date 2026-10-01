//! Const topology exists only during linking; executors retain numeric edges alone.

const LIMIT: usize = 1024;

pub struct Topology {
    count: usize,
    closed: bool,
    shape: Shape,
}

enum Shape {
    Empty,
    Leaf(&'static [Option<usize>]),
    Branch(&'static Topology, &'static Topology),
}

impl Topology {
    pub const EMPTY: Self = Self {
        count: 0,
        closed: true,
        shape: Shape::Empty,
    };
    pub const OPEN: Self = Self {
        count: 0,
        closed: false,
        shape: Shape::Empty,
    };

    pub const fn leaf(targets: &'static [Option<usize>]) -> Self {
        let mut closed = true;
        let mut index = 0;
        while index < targets.len() {
            if targets[index].is_none() {
                closed = false;
            }
            index += 1;
        }
        Self {
            count: 1,
            closed,
            shape: Shape::Leaf(targets),
        }
    }

    pub const fn branch(left: &'static Self, right: &'static Self) -> Self {
        Self {
            count: left.count + right.count,
            closed: left.closed && right.closed,
            shape: Shape::Branch(left, right),
        }
    }

    const fn flatten(&self, nodes: &mut [&'static [Option<usize>]; LIMIT], offset: usize) {
        match self.shape {
            Shape::Empty => (),
            Shape::Leaf(targets) => nodes[offset] = targets,
            Shape::Branch(left, right) => {
                left.flatten(nodes, offset);
                right.flatten(nodes, offset + left.count);
            }
        }
    }

    pub const fn validate(&self) {
        // Open compositions and larger graphs still use the runtime validation.
        if !self.closed || self.count > LIMIT {
            return;
        }
        let mut nodes: [&[Option<usize>]; LIMIT] = [&[]; LIMIT];
        self.flatten(&mut nodes, 0);
        let mut color = [0u8; LIMIT];
        let mut stack = [0usize; LIMIT];
        let mut next = [0usize; LIMIT];
        let mut start = 0;
        while start < self.count {
            if color[start] == 0 {
                let mut depth = 1;
                stack[0] = start;
                color[start] = 1;
                while depth != 0 {
                    let node = stack[depth - 1];
                    if next[node] == nodes[node].len() {
                        color[node] = 2;
                        depth -= 1;
                    } else {
                        let target = match nodes[node][next[node]] {
                            Some(target) => target,
                            None => panic!("open dependency in closed topology"),
                        };
                        next[node] += 1;
                        assert!(target < self.count, "static dependency outside topology");
                        assert!(color[target] != 1, "dependency cycle in closed static registry");
                        if color[target] == 0 {
                            color[target] = 1;
                            stack[depth] = target;
                            depth += 1;
                        }
                    }
                }
            }
            start += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Topology;

    const CYCLE: Topology = Topology::leaf(&[Some(0)]);
    const N2: Topology = Topology::branch(&CYCLE, &CYCLE);
    const N4: Topology = Topology::branch(&N2, &N2);
    const N8: Topology = Topology::branch(&N4, &N4);
    const N16: Topology = Topology::branch(&N8, &N8);
    const N32: Topology = Topology::branch(&N16, &N16);
    const N64: Topology = Topology::branch(&N32, &N32);
    const N128: Topology = Topology::branch(&N64, &N64);
    const N256: Topology = Topology::branch(&N128, &N128);
    const N512: Topology = Topology::branch(&N256, &N256);
    const AT_LIMIT: Topology = Topology::branch(&N512, &N512);
    const OVER_LIMIT: Topology = Topology::branch(&AT_LIMIT, &CYCLE);
    const FALLBACK: () = OVER_LIMIT.validate();

    #[test]
    #[should_panic(expected = "dependency cycle in closed static registry")]
    fn checks_the_last_supported_size() {
        AT_LIMIT.validate();
    }

    #[test]
    fn larger_graphs_defer_even_cycles_to_runtime() {
        let () = FALLBACK;
    }
}
