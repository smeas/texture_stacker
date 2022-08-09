use winres::VersionInfo;

fn main() {
    let version = (0, 2, 1, 0);
    let version_num = version.0 << 48 | version.1 << 32 | version.2 << 16 | version.3;

    let mut res = winres::WindowsResource::new();
    res.set_version_info(VersionInfo::FILEVERSION, version_num);
    res.set_version_info(VersionInfo::PRODUCTVERSION, version_num);
    res.set("LegalCopyright", "© 2022 Jonatan Johansson");
    res.compile().unwrap();
}