use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{self, WrapErr};
use lib::project::Paths;
use lib::stdx;
use lib::test::Id;
use termcolor::Color;

use crate::cli::Context;
use crate::ui;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "util-migrate-args")]
pub struct Args {
    /// Confirm the migration
    #[arg(long)]
    pub confirm: bool,

    /// The name of the new sub directories the tests get moved to
    #[arg(long, default_value = "self")]
    pub name: String,
}

pub fn run(ctx: &mut Context, args: &Args) -> eyre::Result<()> {
    let project = ctx.project()?;
    let paths = project.paths();
    let mut w = ctx.ui.stderr();

    let mappings = collect_old_structure(paths, &args.name)?;

    if mappings.is_empty() {
        writeln!(w, "No tests need to be moved")?;
        return Ok(());
    }

    if args.confirm {
        writeln!(w, "Moving tests:")?;
    } else {
        writeln!(w, "These tests would be moved:")?;
    }

    for (old, (new, collision)) in &mappings {
        if *collision {
            ui::write_bold_colored(&mut w, Color::Red, |w| write!(w, "*"))?;
            write!(w, " ")?;
        } else {
            write!(w, "  ")?;
        }
        ui::write_test_id(&mut w, old)?;
        write!(w, " -> ")?;
        ui::write_test_id(&mut w, new)?;
        writeln!(w)?;
    }

    writeln!(w)?;

    let mut has_colission = false;
    for (old, (new, collision)) in &mappings {
        if !*collision {
            migrate_test(paths, old, new)?;
        } else {
            has_colission = true;
        }
    }

    if has_colission {
        ctx.ui.hint_with(|w| {
            ui::write_bold_colored(w, Color::Red, |w| write!(w, "*"))?;
            writeln!(
                w,
                " denotes paths which were excluded because of another test with the same id."
            )?;
            write!(w, "Try another name using ")?;
            ui::write_colored(w, Color::Cyan, |w| write!(w, "--name"))?;
            writeln!(w)
        })?;
    }

    if !args.confirm {
        ctx.ui.warning("Make sure to back up your code!")?;

        ctx.ui.hint_with(|w| {
            write!(w, "Use ")?;
            ui::write_colored(w, Color::Cyan, |w| write!(w, "--confirm"))?;
            writeln!(w, " to move the tests automatically")
        })?;
        ctx.ui.hint_with(|w| {
            write!(w, "Use ")?;
            ui::write_colored(w, Color::Cyan, |w| write!(w, "--name"))?;
            writeln!(w, " to configure the sub directory name")
        })?;
    }

    Ok(())
}

pub fn collect_old_structure(
    paths: &Paths,
    migration_name: &str,
) -> eyre::Result<BTreeMap<Id, (Id, bool)>> {
    let mut entries = BTreeSet::new();
    for entry in paths.test_root().read_dir()? {
        let entry = entry?;
        if entry.metadata()?.is_dir() {
            collect_old_structure_inner(paths, &entry.path(), &mut entries)?;
        }
    }

    let mut mappings = BTreeMap::new();
    'outer: for id in &entries {
        'inner: for internal in id.ancestors().skip(1) {
            if !entries.contains(internal) {
                continue 'inner;
            }

            let old = Id::new(internal)?;
            let new = Id::new(format!("{internal}/{migration_name}"))?;
            let colission = entries.contains(&new);

            if mappings.insert(old, (new, colission)).is_some() {
                continue 'outer;
            }
        }
    }

    Ok(mappings)
}

fn collect_old_structure_inner(
    paths: &Paths,
    path: &Path,
    entries: &mut BTreeSet<Id>,
) -> eyre::Result<()> {
    if path.join("test.typ").try_exists()? {
        entries.insert(Id::new_from_path(path.strip_prefix(paths.test_root())?)?);
    }

    for entry in fs::read_dir(path).wrap_err_with(|| format!("{path:?}"))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();

        if name == "ref" || name == "out" || name == "diff" {
            continue;
        }

        if entry.metadata()?.is_dir() {
            collect_old_structure_inner(paths, &path, entries)?;
        }
    }

    Ok(())
}

fn migrate_test_part(
    paths: &Paths,
    old: &Id,
    new: &Id,
    f: fn(&Paths, &Id) -> PathBuf,
) -> eyre::Result<()> {
    let old = f(paths, old);
    let new = f(paths, new);

    if old.try_exists()? {
        fs::rename(&old, &new).wrap_err(format!("moving {old:?} to {new:?}"))?;
    }

    Ok(())
}

fn migrate_test(paths: &Paths, old: &Id, new: &Id) -> eyre::Result<()> {
    let test_dir = paths.test_dir(new);
    stdx::fs::create_dir(&test_dir, true).wrap_err(format!("creating to {test_dir:?}"))?;
    migrate_test_part(paths, old, new, Paths::test_script)?;
    migrate_test_part(paths, old, new, Paths::test_ref_script)?;
    migrate_test_part(paths, old, new, Paths::test_ref_dir)?;
    let out_dir = paths.test_out_dir(old);
    stdx::fs::remove_dir(&out_dir, true).wrap_err(format!("removing to {out_dir:?}"))?;
    let diff_dir = paths.test_diff_dir(old);
    stdx::fs::remove_dir(&diff_dir, true).wrap_err(format!("removing to {diff_dir:?}"))?;
    Ok(())
}
