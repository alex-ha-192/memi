// Gives the Windows .exe its icon and version details. Nothing to do elsewhere.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "Memi")
            .set("FileDescription", "Memi")
            .set("CompanyName", "Memi")
            .set("LegalCopyright", "GPL-3.0-or-later");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Couldn't add the Windows icon: {e}");
        }
    }
}
