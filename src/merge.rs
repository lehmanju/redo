/// Macro for merging commands.
///
/// # Examples
/// ```
/// # use redo::*;
/// #[derive(Debug)]
/// struct Add(String);
///
/// impl Command<String> for Add {
///     type Error = ();
///
///     fn apply(&mut self, s: &mut String) -> Result<(), ()> {
///         s.push_str(&self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), ()> {
///         let len = s.len() - self.0.len();
///         s.truncate(len);
///         Ok(())
///     }
///
///     fn merge(&mut self, Add(s): Self) -> Result<(), Self> {
///         self.0.push_str(&s);
///         Ok(())
///     }
/// }
///
/// fn main() -> Result<(), Error<String, Add>> {
///     let mut record = Record::default();
///
///     let cmd = merge![Add("a".into()), Add("b".into()), Add("c".into())].unwrap();
///     record.apply(cmd)?;
///     assert_eq!(record.as_receiver(), "abc");
///     record.undo().unwrap()?;
///     assert_eq!(record.as_receiver(), "");
///     record.redo().unwrap()?;
///     assert_eq!(record.into_receiver(), "abc");
///
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! merge {
    ($cmd1:expr, $cmd2:expr) => {{
        let mut cmd = $cmd1;
        match cmd.merge($cmd2) {
            Ok(_) => Ok(cmd),
            Err(err) => Err(err),
        }
    }};
    ($cmd1:expr, $cmd2:expr, $($tail:expr),+) => {{
        let mut cmd = $cmd1;
        match cmd.merge($cmd2) {
            Ok(_) => merge![cmd, $($tail),*],
            Err(err) => Err(err),
        }
    }};
}
