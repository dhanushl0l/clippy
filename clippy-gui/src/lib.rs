use clippy::UserCred;

#[derive(Debug)]
pub enum Thumbnail {
    Image((Vec<u8>, (u32, u32))),
    Text(String),
}

pub enum Waiting {
    CheckUser(Result<bool, String>),
    SigninOTP(Result<(), String>),
    Login(Result<UserCred, String>),
    Signin(Result<UserCred, String>),
    None,
}

pub fn str_formate(text: &str) -> String {
    let mut result = String::new();
    let mut count = 0;

    let lines: Vec<_> = text.lines().collect();
    let line_count = lines.len();

    if line_count == 1 {
        return text.trim().to_string();
    }

    for line in lines {
        if count >= 11 {
            break;
        }

        result.push_str(line);
        result.push('\n');
        count += 1;
    }

    result
}

#[macro_export]
macro_rules! set_lock {
    ($lock:expr, $value:expr) => {
        if let Ok(mut val) = $lock.try_lock() {
            *val = $value;
        }
    };
}
