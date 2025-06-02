#[macro_export]
macro_rules! log_eprintln {
    ($expr:expr) => {
        if let Err(e) = $expr {
            eprintln!("Error: {:?}", e);
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($expr:expr) => {
        if let Err(e) = $expr {
            error!("Error: {:?}", e);
        }
    };
}
