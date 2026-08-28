use std::{fs, path::Path};

mod temp_dir;

use temp_dir::TestDirectory;

pub(crate) struct Fixture(TestDirectory);

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = TestDirectory::new();
        fs::create_dir_all(root.path().join("content/en")).expect("fixture content directory");
        fs::create_dir_all(root.path().join("templates")).expect("fixture template directory");
        fs::write(
            root.path().join("heine.toml"),
            "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n",
        )
        .expect("fixture configuration");
        Self(root)
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }

    pub(crate) fn write(&self, relative: impl AsRef<Path>, contents: &str) {
        let relative = relative.as_ref();
        let path = self.path().join(relative);
        fs::create_dir_all(path.parent().expect("fixture file has a parent"))
            .expect("fixture parent directory");
        let mut contents = contents.to_owned();
        if relative == Path::new("heine.toml") {
            if !contents.contains("[site]") {
                contents.push_str("\n[site]\ntime_zone = 'Europe/Berlin'\n");
            }
            if !contents.contains("[i18n]") {
                contents.push_str(
                    "\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n",
                );
            }
        }
        fs::write(path, contents).expect("fixture file");
    }
}
