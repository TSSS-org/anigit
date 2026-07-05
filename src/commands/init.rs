use anyhow::Result;
use std::env;

use crate::repo::Repo;

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::init(&cwd)?;
    let branch = repo.current_branch()?;
    println!(
        "Initialized empty anigit repo in {}/.anigit",
        cwd.display()
    );
    println!("Current branch: {branch}");
    Ok(())
}
