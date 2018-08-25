extern crate redo;

use std::{error::Error, fmt};
use redo::{Command, History};

#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    type Error = Box<dyn Error>;

    fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
        receiver.push(self.0);
        Ok(())
    }

    fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
        self.0 = receiver.pop().ok_or("`receiver` is empty")?;
        Ok(())
    }
}

impl fmt::Display for Add {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Add '{}'.", self.0)
    }
}

fn main() {
    let mut history = History::default();
    assert!(history.apply(Add('a')).unwrap().is_none());
    assert!(history.apply(Add('b')).unwrap().is_none());
    assert!(history.apply(Add('c')).unwrap().is_none());
    assert!(history.apply(Add('d')).unwrap().is_none());
    assert!(history.apply(Add('e')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcde");
    history.undo().unwrap().unwrap();
    history.undo().unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abc");
    let abcde = history.apply(Add('f')).unwrap().unwrap();
    assert!(history.apply(Add('g')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfg");
    history.undo().unwrap().unwrap();
    let abcfg = history.apply(Add('h')).unwrap().unwrap();
    assert!(history.apply(Add('i')).unwrap().is_none());
    assert!(history.apply(Add('j')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhij");
    history.undo().unwrap().unwrap();
    let abcfhij = history.apply(Add('k')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhik");
    history.undo().unwrap().unwrap();
    let abcfhik = history.apply(Add('l')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhil");
    assert!(history.apply(Add('m')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhilm");
    let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
    history.apply(Add('n')).unwrap().unwrap();
    assert!(history.apply(Add('o')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abno");
    history.undo().unwrap().unwrap();
    let abno = history.apply(Add('p')).unwrap().unwrap();
    assert!(history.apply(Add('q')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abnpq");

    let abnpq = history.go_to(abcde, 5).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcde");
    assert_eq!(history.go_to(abcfg, 5).unwrap().unwrap(), abcde);
    assert_eq!(history.as_receiver(), "abcfg");
    assert_eq!(history.go_to(abcfhij, 7).unwrap().unwrap(), abcfg);
    assert_eq!(history.as_receiver(), "abcfhij");
    assert_eq!(history.go_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
    assert_eq!(history.as_receiver(), "abcfhik");
    assert_eq!(history.go_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
    assert_eq!(history.as_receiver(), "abcfhilm");
    assert_eq!(history.go_to(abno, 4).unwrap().unwrap(), abcfhilm);
    assert_eq!(history.as_receiver(), "abno");
    assert_eq!(history.go_to(abnpq, 5).unwrap().unwrap(), abno);
    history.set_saved(true);
    assert_eq!(history.as_receiver(), "abnpq");

    println!("{}", history.display().colored(true));
}
