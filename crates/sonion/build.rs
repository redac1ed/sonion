fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../browser/icon.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../browser/icon.ico");
        res.set("ProductName", "sonion");
        res.set("FileDescription", "sonion command line tools");
        res.set("CompanyName", "sonion");
        res.set("LegalCopyright", "Copyright (c) redac1ed. MIT License.");
        res.compile().expect("failed to embed windows resources");
    }
}
