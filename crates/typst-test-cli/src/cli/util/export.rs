use crate::cli::{CompileArgs, Configure, ExportArgs, OperationArgs, Run, RunArgs};
use crate::project::Project;
use crate::test::runner::RunnerConfig;

use super::Context;

#[derive(clap::Args, Debug, Clone)]
#[group(id = "export-args")]
pub struct Args {
    #[command(flatten)]
    pub compile_args: CompileArgs,

    #[command(flatten)]
    pub export_args: ExportArgs,

    #[command(flatten)]
    pub run_args: RunArgs,

    #[command(flatten)]
    pub op_args: OperationArgs,
}

impl Configure for Args {
    fn configure(
        &self,
        ctx: &mut Context,
        project: &Project,
        config: &mut RunnerConfig,
    ) -> anyhow::Result<()> {
        self.compile_args.configure(ctx, project, config)?;
        self.export_args.configure(ctx, project, config)?;
        self.run_args.configure(ctx, project, config)?;

        Ok(())
    }
}

impl Run for Args {
    fn compile_args(&self) -> &CompileArgs {
        &self.compile_args
    }

    fn run_args(&self) -> &RunArgs {
        &self.run_args
    }

    fn op_args(&self) -> &OperationArgs {
        &self.op_args
    }
}

pub fn run(ctx: &mut Context, args: &Args) -> anyhow::Result<()> {
    args.run(ctx)
}
