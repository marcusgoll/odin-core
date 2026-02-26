#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationCommand {
    Export,
    Validate,
    Import,
}

pub fn run(command: MigrationCommand) -> anyhow::Result<()> {
    match command {
        MigrationCommand::Export => {
            println!("migrate export is not implemented yet");
        }
        MigrationCommand::Validate => {
            println!("migrate validate is not implemented yet");
        }
        MigrationCommand::Import => {
            println!("migrate import is not implemented yet");
        }
    }

    Ok(())
}
