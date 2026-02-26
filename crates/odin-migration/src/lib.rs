pub mod checksum;
pub mod export;
pub mod inventory;
pub mod model;
pub mod validate;

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationCommand {
    Export {
        source_root: PathBuf,
        odin_dir: PathBuf,
        out_dir: PathBuf,
    },
    Validate,
    Import,
    Inventory {
        input_dir: PathBuf,
        output_path: PathBuf,
    },
}

pub fn run(command: MigrationCommand) -> anyhow::Result<()> {
    match command {
        MigrationCommand::Export {
            source_root,
            odin_dir,
            out_dir,
        } => {
            export::write_bundle(&source_root, &odin_dir, &out_dir)?;
            println!("migrate export bundle written to {}", out_dir.display());
        }
        MigrationCommand::Validate => {
            println!("migrate validate is not implemented yet");
        }
        MigrationCommand::Import => {
            println!("migrate import is not implemented yet");
        }
        MigrationCommand::Inventory {
            input_dir,
            output_path,
        } => {
            inventory::write_inventory_snapshot(&input_dir, &output_path)?;
            println!(
                "migrate inventory snapshot written to {}",
                output_path.display()
            );
        }
    }

    Ok(())
}
