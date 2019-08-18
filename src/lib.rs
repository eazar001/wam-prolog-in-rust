pub mod ast;

use self::ast::{Assertion, Atom, Clause, Const, Term, Var};
use lalrpop_util::lalrpop_mod;
use lazy_static::lazy_static;
use pancurses::*;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::sync::Mutex;

lalrpop_mod!(pub parser);

lazy_static! {
    static ref KB: Database = {
        vec![
            Assertion {
                head: Atom::new(
                    "p",
                    vec![
                        Term::Var(Var("X".to_string(), 0)),
                        Term::Var(Var("Y".to_string(), 0)),
                    ],
                ),
                clause: vec![
                    Atom::new(
                        "q",
                        vec![
                            Term::Var(Var("X".to_string(), 0)),
                            Term::Var(Var("Z".to_string(), 0)),
                        ],
                    ),
                    Atom::new(
                        "r",
                        vec![
                            Term::Var(Var("Z".to_string(), 0)),
                            Term::Var(Var("Y".to_string(), 0)),
                        ],
                    ),
                ],
            },
            Assertion {
                head: Atom::new(
                    "q",
                    vec![
                        Term::Atom(Atom::new("a", vec![])),
                        Term::Atom(Atom::new("b", vec![])),
                    ],
                ),
                clause: vec![],
            },
            Assertion {
                head: Atom::new(
                    "r",
                    vec![
                        Term::Atom(Atom::new("b", vec![])),
                        Term::Atom(Atom::new("c", vec![])),
                    ],
                ),
                clause: vec![],
            },
        ]
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment(HashMap<Var, Term>);
pub type Database = Vec<Assertion>;

#[derive(Debug, Copy, Clone)]
enum UnifyErr {
    NoUnify,
}

#[derive(Debug, Copy, Clone)]
enum SolveErr {
    NoSolution,
}

#[derive(Debug, Clone)]
struct ChoicePoint {
    database: Database,
    environment: Environment,
    clause: Clause,
    depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Solution {
    No,
    Yes(Environment),
}

impl Display for Environment {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        let mut env: Vec<_> = self.0.iter().filter(|(Var(_, n), t)| *n == 0).collect();
        let mut response = String::from("\n");

        if env.is_empty() {
            return Ok(write!(f, "Yes")?);
        }

        env.sort();

        for (Var(x, n), t) in &env[..env.len() - 1] {
            response.push_str(&format!("{} = {}\n", x, self.substitute_term(t)))
        }

        let (Var(x, n), t) = env.last().unwrap();
        response.push_str(&format!("{} = {} ", x, self.substitute_term(t)));

        Ok(write!(f, "{}", response)?)
    }
}

impl ChoicePoint {
    fn new(database: Database, environment: Environment, clause: Clause, depth: usize) -> Self {
        ChoicePoint {
            database,
            environment,
            clause,
            depth,
        }
    }
}

impl Environment {
    fn new() -> Self {
        Environment(HashMap::new())
    }

    fn insert(&mut self, x: &Var, t: &Term) {
        self.0.insert(x.clone(), t.clone());
    }

    fn env(mut self, map: HashMap<Var, Term>) -> Self {
        self.0 = map;
        self
    }

    fn lookup(&self, x: &Var) -> Term {
        match self.0.get(x) {
            Some(t) => t.clone(),
            None => Term::Var(x.clone()),
        }
    }

    fn substitute_term(&self, t: &Term) -> Term {
        match t {
            Term::Var(x) => {
                let s = self.lookup(x);

                if Term::Var(x.clone()) == s {
                    return s;
                }

                self.substitute_term(&s)
            }
            t @ Term::Const(_) => t.clone(),
            Term::Atom(Atom {
                name: Const(name),
                args,
                ..
            }) => Term::Atom(Atom::new(
                name,
                args.iter().map(|t| self.substitute_term(t)).collect(),
            )),
        }
    }

    fn unify_terms(&self, t1: &Term, t2: &Term) -> Result<Self, UnifyErr> {
        match (self.substitute_term(t1), self.substitute_term(t2)) {
            (ref t1, ref t2) if t1 == t2 => Ok(self.clone()),
            (Term::Var(ref y), ref t) | (ref t, Term::Var(ref y)) => {
                if occurs(y, t) {
                    return Err(UnifyErr::NoUnify);
                }

                let (v, t) = (y.clone(), t.clone());
                let mut env = Environment::new().env(self.0.clone());

                env.insert(&v, &t);

                Ok(env)
            }
            (
                Term::Atom(Atom {
                    name: ref c1,
                    args: ref ts1,
                    ..
                }),
                Term::Atom(Atom {
                    name: ref c2,
                    args: ref ts2,
                    ..
                }),
            ) if c1 == c2 => self.unify_lists(ts1, ts2),
            _ => Err(UnifyErr::NoUnify),
        }
    }

    fn unify_lists(&self, l1: &[Term], l2: &[Term]) -> Result<Self, UnifyErr> {
        if l1.len() != l2.len() {
            return Err(UnifyErr::NoUnify);
        }

        let terms = l1.iter().zip(l2.iter());
        let mut env = self.clone();

        for (t1, t2) in terms {
            match env.unify_terms(t1, t2) {
                Err(UnifyErr::NoUnify) => {
                    return Err(UnifyErr::NoUnify);
                }
                Ok(e) => env = e,
            }
        }

        Ok(env)
    }

    fn unify_atoms(&self, a1: &Atom, a2: &Atom) -> Result<Self, UnifyErr> {
        let Atom {
            name: c1,
            args: ts1,
            ..
        } = a1;

        let Atom {
            name: c2,
            args: ts2,
            ..
        } = a2;

        if c1 == c2 {
            return self.unify_lists(ts1, ts2);
        }

        Err(UnifyErr::NoUnify)
    }
}

fn occurs(x: &Var, t: &Term) -> bool {
    match t {
        Term::Var(y) => x == y,
        Term::Const(_) => false,
        Term::Atom(Atom { args, .. }) => args.iter().any(|t| occurs(x, t)),
    }
}

fn renumber_term(n: usize, t: &Term) -> Term {
    match t {
        Term::Var(Var(x, _)) => Term::Var(Var(x.clone(), n)),
        c @ Term::Const(_) => c.clone(),
        Term::Atom(Atom {
            name: Const(c),
            args: ts,
            ..
        }) => Term::Atom(Atom::new(
            c,
            ts.iter().map(|t| renumber_term(n, t)).collect(),
        )),
    }
}

fn renumber_atom(n: usize, a: &Atom) -> Atom {
    let Atom {
        name: Const(c),
        args: ts,
        ..
    } = a;

    Atom::new(c, ts.iter().map(|t| renumber_term(n, t)).collect())
}

fn display_solution(
    window: &Window,
    ch: &[ChoicePoint],
    env: &Environment,
) -> Result<(), SolveErr> {
    match (&env.to_string()[..], ch) {
        ("Yes", _) => {
            window.printw("Yes.");
            window.refresh();
        }
        (answer, []) => {
            window.printw(String::from(answer));
            window.refresh();
        }
        (answer, ch) => {
            window.printw(String::from(answer));
            window.refresh();

            match window.getch() {
                Some(Input::Character(c)) if c == ';' => {
                    continue_search(window, ch);
                }
                None | _ => {
                    return Err(SolveErr::NoSolution);
                }
            }
        }
    }

    Ok(())
}

fn continue_search(window: &Window, ch: &[ChoicePoint]) -> Result<(), SolveErr> {
    match ch.split_first() {
        None => Err(SolveErr::NoSolution),
        Some((
            ChoicePoint {
                database: asrl,
                environment: env,
                clause: gs,
                depth: n,
            },
            cs,
        )) => solve(window, cs, asrl, env, gs, *n),
    }
}

fn solve(
    window: &Window,
    ch: &[ChoicePoint],
    asrl: &[Assertion],
    env: &Environment,
    c: &[Atom],
    n: usize,
) -> Result<(), SolveErr> {
    match c.split_first() {
        None => display_solution(window, ch, env),
        Some((a, next_c)) => match reduce_atom(env, n, a, asrl) {
            None => continue_search(window, ch),
            Some((next_asrl, next_env, mut d)) => {
                let mut next_ch = ch.to_vec();
                next_ch.push(ChoicePoint {
                    database: next_asrl,
                    environment: env.clone(),
                    clause: c.to_vec(),
                    depth: n,
                });

                d.extend_from_slice(next_c);

                solve(window, &next_ch, asrl, &next_env, &d, n + 1)
            }
        },
    }
}

fn reduce_atom(
    env: &Environment,
    n: usize,
    a: &Atom,
    asrl: &[Assertion],
) -> Option<(Vec<Assertion>, Environment, Vec<Atom>)> {
    match asrl.split_first() {
        None => None,
        Some((
            Assertion {
                head: b,
                clause: lst,
            },
            next_asrl,
        )) => {
            let next_env = env.unify_atoms(a, &renumber_atom(n, b));

            match next_env {
                Ok(next_env) => Some((
                    next_asrl.to_vec(),
                    next_env,
                    lst.iter().map(|a| renumber_atom(n, a)).collect(),
                )),
                Err(UnifyErr::NoUnify) => reduce_atom(env, n, a, next_asrl),
            }
        }
    }
}

pub fn solve_toplevel(c: Clause) {
    let window = initscr();
    let env = Environment::new();
    window.keypad(true);

    match solve(&window, &[], &KB, &env, &c, 1) {
        Err(SolveErr::NoSolution) => {
            window.printw("No.");
            window.refresh();
        }
        Ok(()) => (),
    }

    window.printw("\nPress any key to continue\n");
    window.refresh();
    noecho();
    window.getch();
    endwin();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fail() {
        solve_toplevel(vec![Atom::new(
            "p",
            vec![Term::Atom(Atom::new("b", vec![]))],
        )])
    }

    #[test]
    fn test_q_2() {
        solve_toplevel(vec![Atom::new(
            "q",
            vec![
                Term::Var(Var("X".to_string(), 0)),
                Term::Var(Var("Y".to_string(), 0)),
            ],
        )])
    }

    #[test]
    fn test_r_2() {
        solve_toplevel(vec![Atom::new(
            "r",
            vec![
                Term::Var(Var(String::from("X"), 0)),
                Term::Var(Var(String::from("Y"), 0)),
            ],
        )])
    }

    #[test]
    fn test_p_2() {
        solve_toplevel(vec![Atom::new(
            "p",
            vec![
                Term::Var(Var("U".to_string(), 0)),
                Term::Var(Var("V".to_string(), 0)),
            ],
        )])
    }
}
