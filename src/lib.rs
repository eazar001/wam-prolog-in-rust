#![allow(dead_code)]
#![allow(unused)]
#![allow(clippy::new_without_default)]

mod machine;

use self::Cell::*;
use self::Store::*;
use self::Mode::{Read, Write};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Debug};
use std::cmp::Ordering;
use machine::instructions::*;
use env_logger;
use log::{info, warn, error, debug, trace, Level};
use log::Level::*;


// heap address represented as usize that corresponds to the vector containing cell data
type HeapAddress = usize;
// x-register address which identifies the register that holds the cell data in the corresponding variable
type Register = usize;
type FunctorArity = usize;
type FunctorName = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Functor(pub FunctorName, pub FunctorArity);

#[derive(Debug, Clone, Eq, PartialEq)]
enum Cell {
    Str(HeapAddress),
    Ref(HeapAddress),
    Func(Functor)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Store {
    HeapAddr(HeapAddress),
    XAddr(Register)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Mode {
    Read,
    Write
}

// the "global stack"
#[derive(Debug, Clone, Eq, PartialEq)]
struct Heap {
    // all the data that resides on the heap
    cells: Vec<Cell>
}

#[derive(Clone, Eq, PartialEq)]
struct Registers {
    // the "h" counter contains the location of the next cell to be pushed onto the heap
    h: HeapAddress,
    // variable register mapping a variable to cell data (x-register)
    x: HashMap<Register, Cell>,
    // subterm register containing heap address of next subterm to be matched (s-register)
    s: Register,
    // program/instruction counter, containing address of the next instruction to be executed
    p: Register,
    // address of the next instruction in the code area to follow up after successful return from a call
    cp: Register
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Env {
    heap: Heap,
    // the "push-down-list" contains StoreAddresses and serves as a unification stack
    pdl: Vec<Store>,
    registers: Registers,
    mode: Mode,
    fail: bool,
}

impl Debug for Registers {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        let mut keys: Vec<&usize> = self.x.keys().collect();
        keys.sort();

        write!(f, "[")?;

        for key in &keys[..keys.len()-1] {
            write!(f, "{}: {:?}, ", key, self.x[key])?;
        }

        Ok(write!(f, "{}: {:?}]", keys.len(), self.x[&keys.len()])?)
    }
}

impl Display for Cell {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Ref(a) => Ok(write!(f, "{:?}", Ref(*a))?),
            Str(a) => Ok(write!(f, "{:?}", Str(*a))?),
            Func(f1) => Ok(write!(f, "Functor({})", f1)?)
        }
    }
}

impl Display for Functor {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        Ok(write!(f, "{}/{}", self.name(), self.arity())?)
    }
}

impl From<&str> for Functor {
    fn from(s: &str) -> Functor {
        let v: Vec<&str> = s.split('/').collect();

        assert_eq!(v.len(), 2);

        Functor(String::from(v[0]), String::from(v[1]).parse().unwrap())
    }
}

impl Functor {
    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn arity(&self) -> usize {
        self.1
    }
}

impl Env {
    pub fn new() -> Env {
        Env {
            heap: Heap::new(),
            pdl: Vec::new(),
            registers: Registers::new(),
            mode: Read,
            fail: false
        }
    }

    fn push_heap(&mut self, cell: Cell) {
        trace!("\t\tHEAP[{}] <- {}", self.heap_counter(), cell);

        self.heap.cells.push(cell);
    }

    fn get_x(&self, xi: Register) -> Option<&Cell> {
        self.registers.get_x(xi)
    }

    fn insert_x(&mut self, xi: Register, cell: Cell) -> Option<Cell> {
        trace!("\t\tX{} <- {:?}", xi, cell);

        self.registers.insert_x(xi, cell)
    }

    fn get_s(&self) -> Register {
        self.registers.s
    }

    fn inc_s(&mut self, value: usize) {
        trace!("\t\tS <- S + {}", value);
        self.registers.s += value;
    }

    fn set_s(&mut self, value: usize) {
        trace!("\t\tS <- {}", value);

        self.registers.s = value;
    }

    fn set_fail(&mut self, value: bool) {
        trace!("\t\tFail <- {}", value);

        self.fail = value;
    }

    fn heap_counter(&self) -> usize {
        self.registers.h
    }

    fn inc_heap_counter(&mut self, value: usize) {
        trace!("\t\tH <- H + {}", value);

        self.registers.h += value;
    }

    fn set_mode(&mut self, mode: Mode) {
        trace!("\t\tMode <- {:?}", mode);

        self.mode = mode;
    }

    fn empty_pdl(&mut self) -> bool {
        self.pdl.is_empty()
    }

    fn push_pdl(&mut self, address: Store) {
        self.pdl.push(address);
    }

    fn pop_pdl(&mut self) -> Option<Store> {
        self.pdl.pop()
    }

    fn call(&mut self, _functor: Functor) {
        unimplemented!()
    }

    fn proceed(&mut self) {
//        unimplemented!()
    }

    // put_structure f/n, Xi
    fn put_structure(&mut self, f: Functor, xi: Register) {
        trace!("put_structure: ");
        let h = self.heap_counter();

        self.push_heap(Str(h+1));
        self.push_heap(Func(f));
        self.insert_x(xi, self.heap.cells[h].clone());
        self.inc_heap_counter(2);
    }

    // set_variable Xi
    fn set_variable(&mut self, xi: Register) {
        let h = self.heap_counter();

        trace!("set_variable: ");

        self.push_heap(Ref(h));
        self.insert_x(xi, self.heap.cells[h].clone());
        self.inc_heap_counter(1);
    }

    // set_value Xi
    fn set_value(&mut self, xi: Register) {
        trace!("set_value: ");

        self.push_heap(self.get_x(xi).cloned().unwrap());
        self.inc_heap_counter(1);
    }

    fn deref(&self, address: Store) -> Store {
        let mut address = address;
        let start_address = address;

        loop {
            let (cell, a) = match address {
                HeapAddr(addr) => (&self.heap.cells[addr], addr),
                XAddr(addr) => {
                    let e = &format!("Illegal access: register {}, does not exist", addr);
                    let c = self.get_x(addr).expect(e);

                    (c, addr)
                }
            };

            match cell {
                Ref(value) => {
                    if *value != a {
                        // keep following the reference chain
                        address = HeapAddr(*value);
                    } else {
                        // ref cell is unbound return the address
                        trace!("\t\tderef: {:?} -> {:?}", start_address, address);
                        return address
                    }
                },
                Str(addr) => {
                    trace!("\t\tderef: {:?} -> {:?}", start_address, address);
                    return address
                },
                Func(_) => {
                    trace!("\t\tderef: {:?} -> {:?}", start_address, address);
                    return address
                }
            }
        }
    }

    // get_structure f/n, Xi
    fn get_structure(&mut self, f: Functor, xi: Register) {
        trace!("get_structure: ");
        let (cell, address) = match self.deref(XAddr(xi)) {
            HeapAddr(addr) => (self.heap.cells[addr].clone(), addr),
            XAddr(addr) => (self.get_x(xi).cloned().unwrap(), addr)
        };

        match cell {
            Ref(_) => {
                let h = self.heap_counter();

                self.push_heap(Str(h+1));
                self.push_heap(Func(f.clone()));
                self.bind(HeapAddr(address), HeapAddr(h));

                self.inc_heap_counter(2);
                self.set_mode(Write);
            },
            Str(a) => {
                match self.heap.cells[a] {
                    Func(ref functor) => {
                        if functor == &f {
                            self.set_s(a+1);
                            self.set_mode(Read);
                        } else {
                            self.set_fail(true);
                        }
                    }
                    _ => panic!()
                }
            },
            Func(_) => {
                self.set_fail(true);
            }
        }
    }

    // unify_variable Xi
    fn unify_variable(&mut self, xi: Register) {
        trace!("unify_variable: ");
        match self.mode {
            Read => {
                let s = self.get_s();

                self.insert_x(xi, self.heap.cells[s].clone());
            },
            Write => {
                let h = self.heap_counter();

                self.push_heap(Ref(h));
                self.insert_x(xi, self.heap.cells[h].clone());
                self.inc_heap_counter(1);
            }
        }

        self.inc_s(1);
    }

    // unify_value Xi
    fn unify_value(&mut self, xi: Register) {
        trace!("unify_value ({:?}): ", self.mode);

        match self.mode {
            Read => {
                let s = self.get_s();

                self.unify(XAddr(xi), HeapAddr(s))
            },
            Write => {
                self.push_heap(self.get_x(xi).unwrap().clone());
                self.inc_heap_counter(1);
            }
        }

        self.inc_s(1);
    }

    fn unify(&mut self, a1: Store, a2: Store) {
        trace!("\t\tunify: {:?} <-> {:?}", a1, a2);

        self.push_pdl(a1);
        self.push_pdl(a2);

        self.set_fail(false);

        while !(self.empty_pdl() || self.fail) {
            let (a1, a2) = (self.pop_pdl().unwrap(), self.pop_pdl().unwrap());

            let d1 = self.deref(a1);
            let d2 = self.deref(a2);

            if d1 != d2 {
                let c1 = self.get_store_cell(d1);
                let c2 = self.get_store_cell(d2);

                if c1.is_ref() || c2.is_ref() {
                    self.bind(d1, d2);
                } else {
                    let v1 = match c1.address() {
                        Some(addr) => addr,
                        None => panic!()
                    };

                    let v2 = match c2.address() {
                        Some(addr) => addr,
                        None => panic!()
                    };

                    let (f1, f2) = (self.get_functor(c1), self.get_functor(c2));

                    if f1 == f2 {
                        let n1 = f1.arity();

                        for i in 1..n1 {
                            self.push_pdl(HeapAddr(v1+i));
                            self.push_pdl(HeapAddr(v2+i));
                        }
                    } else {
                        self.set_fail(true);
                    }
                }
            }
        }
    }

    // extracts functor only if cell is a structure or a functor cell
    fn get_functor(&self, cell: &Cell) -> Functor {
        match cell {
            Str(addr) => {
                if let Func(f) = self.heap.cells[*addr].clone() {
                    trace!("\t\tget_functor: {:?} -> {}", cell, f);
                    f
                } else {
                    error!("encountered a structure that doesn't point to a functor");
                    panic!("invalid cell: structure cell pointing to non-functor data")
                }
            },
            Func(f) => {
                warn!("accessing a functor from a functor-cell, but this normally shouldn't happen");
                trace!("\t\tget_functor: {:?} -> {}", cell, f);
                f.clone()
            },
            Ref(_) => {
                error!("tried getting a functor from a ref-cell");
                panic!("invalid cell-type for functor retrieval used");
            }
        }
    }

    fn get_store_cell(&self, address: Store) -> &Cell {
        match address {
            HeapAddr(addr) => &self.heap.cells[addr],
            XAddr(addr) => self.get_x(addr).unwrap()
        }
    }

    fn bind(&mut self, a1: Store, a2: Store) {
        let (c1, c2) = (self.get_store_cell(a1), self.get_store_cell(a2));
        let (a1, a2) = (c1.address().unwrap(), c2. address().unwrap());

        if c1.is_ref() && (!c2.is_ref() || a2 < a1) {
            trace!("\t\tbind: HEAP[{}] <- {:?} | ({:?} <- {:?})", a1, c2.clone(), c1.clone(), c2.clone());
            self.heap.cells[a1] = c2.clone();
        } else {
            trace!("\t\tbind: HEAP[{}] <- {:?} | ({:?} <- {:?})", a2, c1.clone(), c2.clone(), c1.clone());
            self.heap.cells[a2] = c1.clone();
        }
    }
}

impl Registers {
    fn new() -> Registers {
        Registers {
            h: 0,
            x: HashMap::new(),
            s: 0,
            p: 0,
            cp: 0
        }
    }

    fn get_x(&self, register: Register) -> Option<&Cell> {
        self.x.get(&register)
    }

    fn insert_x(&mut self, register: Register, cell: Cell) -> Option<Cell> {
        self.x.insert(register, cell)
    }
}

impl Heap {
    fn new() -> Heap {
        Heap {
            cells: Vec::new()
        }
    }
}

impl Cell {
    fn is_ref(&self) -> bool {
        if let Ref(_) = self {
            return true
        }

        false
    }

    fn is_str(&self) -> bool {
        if let Str(_) = self {
            return true
        }

        false
    }

    fn is_func(&self) -> bool {
        if let Func(_) = self {
            return true
        }

        false
    }

    fn address(&self) -> Option<HeapAddress> {
        match self {
            Str(addr) => Some(*addr),
            Ref(addr) => Some(*addr),
            Func(_) => None
        }
    }
}

impl Store {
    fn is_heap(&self) -> bool {
        if let HeapAddr(_) = self {
            return true
        }

        false
    }

    fn is_x(&self) -> bool {
        if let XAddr(_) = self {
            return true
        }

        false
    }

    fn address(&self) -> usize {
        match self {
            HeapAddr(addr) => *addr,
            XAddr(addr) => *addr
        }
    }
}

impl PartialOrd for Store {
    fn partial_cmp(&self, other: &Store) -> Option<Ordering> {
        match self {
            HeapAddr(a1) => {
                if other.is_heap() {
                    let a2 = other.address();

                    return Some(a1.cmp(&a2))
                }
            },
            XAddr(_) => return None
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_logger() {
        env_logger::builder()
            .is_test(true)
            .default_format_timestamp(false)
            .try_init()
            .unwrap()
    }

    // set_variable Xi
    #[test]
    fn test_set_variable() {
        let mut env = Env::new();

        env.set_variable(0);

        let expected_heap_cells = vec![Ref(0)];
        let heap_cells = env.heap.cells;
        let registers = env.registers;

        assert_eq!(heap_cells, expected_heap_cells);
        register_is(&registers, 0, Ref(0));
    }

    // set_value Xi
    #[test]
    fn test_set_value() {
        let mut env = Env::new();

        env.set_variable(0);
        env.set_variable(1);

        env.set_value(0);
        env.set_value(1);

        let expected_heap_cells = vec![Ref(0), Ref(1), Ref(0), Ref(1)];
        let heap_cells = env.heap.cells;
        let registers = env.registers;

        assert_eq!(heap_cells, expected_heap_cells);
        register_is(&registers, 0, Ref(0));
        register_is(&registers, 1, Ref(1));
        assert_eq!(registers.x.len(), 2);
    }

    // put_structure f/n, Xi
    #[test]
    fn test_put_structure() {
        let mut env = Env::new();

        env.put_structure(Functor(String::from("foo"), 2), 0);
        env.set_variable(1);
        env.set_variable(2);
        env.set_value(1);

        let expected_heap_cells = vec![
            Str(1),
            Func(Functor::from("foo/2")),
            Ref(2),
            Ref(3),
            Ref(2)
        ];

        let heap_cells = env.heap.cells;
        let registers = env.registers;

        assert_eq!(heap_cells, expected_heap_cells);
        register_is(&registers, 0, Str(1));
        register_is(&registers, 1, Ref(2));
        register_is(&registers, 2, Ref(3));
        assert_eq!(registers.x.len(), 3);
    }

    #[test]
    fn test_deref() {
        let mut env = Env::new();

        env.heap.cells = vec![
            Ref(2),
            Ref(3),
            Ref(1),
            Ref(3),
            Str(5),
            Func(Functor::from("f/2")),
            Ref(3)
        ];

        env.insert_x(3, Ref(4));

        assert_eq!(env.deref(HeapAddr(0)), HeapAddr(3));
        assert_eq!(env.deref(HeapAddr(1)), HeapAddr(3));
        assert_eq!(env.deref(HeapAddr(2)), HeapAddr(3));
        assert_eq!(env.deref(HeapAddr(3)), HeapAddr(3));
        assert_eq!(env.deref(HeapAddr(4)), HeapAddr(4));
        assert_eq!(env.deref(HeapAddr(5)), HeapAddr(5));
        assert_eq!(env.deref(HeapAddr(6)), HeapAddr(3));
        assert_eq!(env.deref(XAddr(3)), HeapAddr(4));
    }

    #[test]
    fn test_exercise_2_1() {
        // L0 program: p(Z, h(Z, W), f(W)).
        let mut env = Env::new();

        let h = String::from("h");
        let f = String::from("f");
        let p = String::from("p");

        // put_structure h/2, x3
        env.put_structure(Functor(h.clone(), 2), 2);
        // set_variable, x2
        env.set_variable(1);
        // set_variable, x5
        env.set_variable(4);
        // put_structure f/1, x4
        env.put_structure(Functor(f.clone(), 1), 3);
        // set_value, x5
        env.set_value(4);
        // put_structure p/3, x1
        env.put_structure(Functor(p.clone(), 3), 0);
        // set_value x2
        env.set_value(1);
        // set_value x3
        env.set_value(2);
        // set_value x4
        env.set_value(3);


        let expected_heap_cells = vec![
            Str(1),
            Func(Functor(h, 2)),
            Ref(2),
            Ref(3),
            Str(5),
            Func(Functor(f, 1)),
            Ref(3),
            Str(8),
            Func(Functor(p, 3)),
            Ref(2),
            Str(1),
            Str(5),
        ];

        let (heap_cells, registers) = (env.heap.cells, &env.registers);
        assert_eq!(heap_cells, expected_heap_cells);

        register_is(registers, 0, Str(8));
        register_is(registers, 1, Ref(2));
        register_is(registers, 2, Str(1));
        register_is(registers, 3, Str(5));
        register_is(registers, 4, Ref(3));
    }

    #[test]
    fn test_exercise_2_3() {
        init_test_logger();

        // L0 Program: p(Z, h(Z, W), f(W)) = p(f(X), h(Y, f(a)), Y).
        let mut env = Env::new();

        let h = String::from("h");
        let f = String::from("f");
        let p = String::from("p");
        let a = String::from("a");

        // put_structure h/2, x3
        env.put_structure(Functor::from("h/2"), 3);
        // set_variable, x2
        env.set_variable(2);
        // set_variable, x5
        env.set_variable(5);
        // put_structure f/1, x4
        env.put_structure(Functor::from("f/1"), 4);
        // set_value, x5
        env.set_value(5);
        // put_structure p/3, x1
        env.put_structure(Functor::from("p/3"), 1);
        // set_value x2
        env.set_value(2);
        // set_value x3
        env.set_value(3);
        // set_value x4
        env.set_value(4);

        // get_structure p/3, x1
        env.get_structure(Functor::from("p/3"), 1);
        // unify_variable x2
        env.unify_variable(2);
        // unify_variable x3
        env.unify_variable(3);
        // unify_variable x4
        env.unify_variable(4);
        // get_structure f/1, x2
        env.get_structure(Functor::from("f/1"), 2);
        // unify_variable x5
        env.unify_variable(5);
        // get_structure h/2, x3
        env.get_structure(Functor::from("h/2"), 3);
        // unify_value x4
        env.unify_value(4);
        // unify_variable x6
        env.unify_variable(6);
        // get_structure f/1, x6
        env.get_structure(Functor::from("f/1"), 6);
        // unify_variable x7
        env.unify_variable(7);
        // get_structure a/0, x7
        env.get_structure(Functor::from("a/0"), 7);

        let expected_heap_cells = vec![
            Str(1),
            Func(Functor::from("h/2")),
            Str(13),
            Str(16),
            Str(5),
            Func(Functor::from("f/1")),
            Ref(3),
            Str(8),
            Func(Functor::from("p/3")),
            Ref(2),
            Str(1),
            Str(5),
            Str(13),
            Func(Functor::from("f/1")),
            Ref(3),
            Str(16),
            Func(Functor::from("f/1")),
            Str(19),
            Str(19),
            Func(Functor::from("a/0"))
        ];

//        env.bind(XAddr(6), XAddr(5));

        debug!("{:?}", env.heap.cells);
        debug!("{:?}", env.registers);
        debug!("{:?}.", !env.fail);

        let (heap_cells, registers) = (&env.heap.cells, &env.registers);

        assert_eq!(heap_cells, &expected_heap_cells);


        register_is(registers, 1, Str(8));
        register_is(registers, 2, Ref(2));
        register_is(registers, 3, Str(1));
        register_is(registers, 4, Str(5));
        register_is(registers, 5, Ref(14));
        register_is(registers, 6, Ref(3));
        register_is(registers, 7, Ref(17));
    }

    #[test]
    fn test_functor_eq() {
        let f1 = Functor::from("foo/1");
        let f2 = Functor::from("bar/1");

        assert_ne!(f1, f2);

        let f2 = Functor::from("foo/1");
        assert_eq!(f1, f2);

        let f2 = Functor::from("foo/2");
        assert_ne!(f1, f2);
    }

    fn register_is(registers: &Registers, register: Register, cell: Cell) {
        assert_eq!(registers.get_x(register).cloned().unwrap(), cell);
    }
}
