macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

#[cfg(not(feature = "host"))]
mod device_app;
#[cfg(feature = "host")]
mod host_app;

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    if first.as_deref() == Some("--install-bootlogo") {
        let Some(card) = std::env::var_os("SLOT_ROOT") else {
            eprintln!("slot: --install-bootlogo needs SLOT_ROOT");
            std::process::exit(2);
        };
        let card = std::path::Path::new(&card);
        slot::bootlogo::refresh(card);
        slot::bootlogo::remember(card);
        return;
    }
    if first.as_deref() == Some("--init-root") {
        let Some(dir) = args.next() else {
            eprintln!("slot: --init-root needs a directory");
            std::process::exit(2);
        };
        slot::root::ensure(std::path::Path::new(&dir));
        return;
    }
    slot::crash::install();

    #[cfg(feature = "host")]
    host_app::run();

    #[cfg(not(feature = "host"))]
    device_app::run();
}
