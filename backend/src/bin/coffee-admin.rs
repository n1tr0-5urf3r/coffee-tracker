use clap::{Parser, Subcommand};
use image::Luma;
use qrcode::QrCode;

#[derive(Parser)]
#[command(name = "coffee-admin")]
#[command(about = "Coffee Tracker administration utility")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a printable QR code PNG for a machine.
    Qr {
        #[arg(long)]
        machine_id: String,
        #[arg(long)]
        base_url: String,
        #[arg(long, default_value = "machine-qr.png")]
        out: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Qr {
            machine_id,
            base_url,
            out,
        } => {
            let base = base_url.trim_end_matches('/');
            let target = format!("{base}/?machine_id={machine_id}");
            let code = QrCode::new(target.as_bytes())?;
            let image = code.render::<Luma<u8>>().min_dimensions(1024, 1024).build();
            image.save(&out)?;
            println!("wrote {out} for {target}");
        }
    }
    Ok(())
}
