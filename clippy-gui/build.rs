fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../assets/win-ico/clippy.ico");
        res.compile().expect("Failed to compile resources");
    }
}
