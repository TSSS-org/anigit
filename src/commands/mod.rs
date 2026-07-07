//! Implementations of each anigit subcommand. Each module corresponds to one
//! command from `cli.rs`. Kept one-file-per-command so the v1 build can be
//! parallelized/filled in piece by piece.

pub mod add;
pub mod blame;
pub mod branch;
pub mod checkout;
pub mod clone_fork;
pub mod commit;
pub mod compare;
pub mod config;
pub mod diff;
pub mod init;
pub mod log;
pub mod merge;
pub mod reflog;
pub mod refresh;
pub mod remote;
pub mod revert;
pub mod show;
pub mod status;
pub mod tag;
pub mod uninstall;
