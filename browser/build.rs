fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Sonion Browser");
        res.set("FileDescription", "Sonion Browser");
        res.set("CompanyName", "sonion");
        res.set("LegalCopyright", "Copyright (c) redac1ed. MIT License.");
        res.compile().expect("failed to embed windows resources");
    }
}
