use std::fmt::{Write as _};
use std::fs::File;
use std::io::{Write as _};
use std::process::Command;
use rle::SplitableSpan;
use crate::list::{OpSet, Time};
use crate::localtime::TimeSpan;
use crate::rle::KVPair;
use crate::ROOT_TIME;

pub fn name_of(time: Time) -> String {
    if time == ROOT_TIME { "ROOT".into() }
    else { format!("{}", time) }
}

#[derive(Debug, Clone, Copy)]
pub enum DotColor {
    Red, Green, Blue, Black
}

impl OpSet {
    pub fn make_graph<I: Iterator<Item=(TimeSpan, DotColor)>>(&self, filename: &str, iter: I) {
        let mut out = String::new();
        out.write_str("strict digraph {\n").unwrap();
        out.write_str("rankdir=\"BT\"\n").unwrap();

        for (span, color) in iter {
            for time in span.iter() {
                let name = name_of(time);

                // This is horribly inefficient but I don't care.
                let (KVPair(_, op), offset) = self.operations.find_packed_with_offset(time);
                let mut op = op.clone();
                op.truncate_keeping_right(offset);
                op.truncate(1);

                let txn = self.history.entries.find_packed(time);

                // let label = if op.tag == Ins {
                let label = if op.content_known {
                    format!("{}: {:?} {} '{}'", time, op.tag, op.pos, &op.content)
                } else {
                    format!("{}: {:?} {}", time, op.tag, op.pos)
                };
                out.write_fmt(format_args!("{} [color={:?} label=\"{}\"]\n", name, color, label)).unwrap();
                txn.with_parents(time, |parents| {
                    for p in parents {
                        out.write_fmt(format_args!("{} -> {}\n", name, name_of(*p))).unwrap();
                    }
                });
            }
        }

        out.write_str("}\n").unwrap();

        let mut f = File::create("out.dot").unwrap();
        f.write_all(out.as_bytes()).unwrap();
        f.flush().unwrap();
        drop(f);

        let out = Command::new("dot")
            .arg("-Tsvg")
            .stdin(File::open("out.dot").unwrap())
            .output().unwrap();

        // dbg!(out.status);
        // stdout().write_all(&out.stdout);
        // stderr().write_all(&out.stderr);

        let mut f = File::create(filename).unwrap();
        f.write_all(&out.stdout).unwrap();

    }
}

#[cfg(test)]
mod test {
    use crate::list::m2::dot::DotColor::*;
    use crate::list::OpSet;
    use crate::ROOT_TIME;

    #[test]
    fn foo() {
        let mut ops = OpSet::new();
        ops.get_or_create_agent_id("seph");
        ops.get_or_create_agent_id("mike");
        ops.push_insert(0, &[ROOT_TIME], 0, "a");
        ops.push_insert(1, &[ROOT_TIME], 0, "b");
        ops.push_delete(0, &[0, 1], 0, 2);

        ops.make_graph("test.svg", [((0..ops.len()).into(), Red)].iter().copied());
    }
}