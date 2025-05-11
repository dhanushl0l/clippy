use clippy::UserCred;

pub enum Thumbnail {
    Image((Vec<u8>, (u32, u32))),
    Text(String),
}

pub enum Waiting {
    CheckUser(Option<bool>),
    SigninOTP(Option<bool>),
    Login(Option<UserCred>),
    Signin(Option<UserCred>),
    None,
}

pub fn str_formate(text: &str) -> String {
    let mut result = String::new();
    let mut count = 0;

    for line in text.lines() {
        if count >= 11 {
            result = result.strip_suffix('\n').unwrap().to_string();
            break;
        }

        if line.len() > 100 {
            result.push_str(&line[..100]);
            result.push_str("....\n");
            count += 1;
        } else {
            result.push_str(line);
            result.push('\n');
            count += 1;
        }
    }

    result
}
