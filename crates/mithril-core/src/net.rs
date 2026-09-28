//! Cell arena and interaction net container.

use crate::port::Port;

#[derive(Clone)]
pub struct Net {
    pub cells: Vec<[u64; 2]>,
    pub free: Vec<u32>,
    pub redexes: Vec<(Port, Port)>,
    /// Redexes no compile-time rule can fire (an opaque primitive, a
    /// division whose divisor is zero, a call kept as a call): they stay
    /// in the residual program instead of the worklist.
    pub residual: Vec<(Port, Port)>,
    /// Next Dup label: every sharing site gets a fresh one (two dups meet
    /// as siblings only when they carry the same label). 24 bits, wraps.
    pub labels: u32,
}

impl Default for Net {
    fn default() -> Net {
        Net::new()
    }
}

impl Net {
    pub fn new() -> Net {
        Net {
            cells: Vec::new(),
            free: Vec::new(),
            redexes: Vec::new(),
            residual: Vec::new(),
            labels: 1,
        }
    }

    /// Allocate a cell holding ports `a` and `b`, reusing a freed slot if
    /// one is available (LIFO), otherwise growing the arena.
    pub fn alloc(&mut self, a: Port, b: Port) -> u32 {
        if let Some(i) = self.free.pop() {
            self.cells[i as usize] = [a.0, b.0];
            i
        } else {
            self.cells.push([a.0, b.0]);
            (self.cells.len() - 1) as u32
        }
    }

    /// Return a cell's slot to the free list for later reuse.
    pub fn free_cell(&mut self, i: u32) {
        self.free.push(i);
    }

    pub fn cell(&self, i: u32) -> [u64; 2] {
        self.cells[i as usize]
    }

    pub fn set(&mut self, i: u32, slot: usize, p: Port) {
        self.cells[i as usize][slot] = p.0;
    }

    /// Stable textual form: one non-freed cell per `cell <i>: ...` line
    /// (indexes decimal, in arena order), followed by one `redex: ...`
    /// line per pending redex, in redex-vector order.
    pub fn dump(&self) -> String {
        let mut freed = vec![false; self.cells.len()];
        for &i in &self.free {
            freed[i as usize] = true;
        }

        let mut out = String::new();
        for (i, c) in self.cells.iter().enumerate() {
            if freed[i] {
                continue;
            }
            let a = Port(c[0]);
            let b = Port(c[1]);
            out.push_str(&format!(
                "cell {}: {:?}({}) {:?}({})\n",
                i,
                a.tag(),
                a.payload(),
                b.tag(),
                b.payload()
            ));
        }
        for (a, b) in &self.redexes {
            out.push_str(&format!(
                "redex: {:?}({}) {:?}({})\n",
                a.tag(),
                a.payload(),
                b.tag(),
                b.payload()
            ));
        }
        out
    }
}
