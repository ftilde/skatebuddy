use base64::Engine;
use littlefs2::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use crate::Context;

use super::menu::{MenuItem, MenuSelection};

struct FileList<'a> {
    dir: &'a Path,
    flash: &'a mut drivers::flash::FlashRessources,
}

impl<'a, const N: usize> super::menu::Paginated<N> for FileList<'a> {
    type Item = littlefs2::fs::DirEntry;

    async fn access(&mut self, i: usize) -> arrayvec::ArrayVec<Self::Item, N> {
        self.flash
            .with_fs(|fs| {
                fs.read_dir_and_then(self.dir, |dir_it| {
                    let filtered = dir_it.filter_map(|f| {
                        let f = f.unwrap();
                        if f.file_name() == "." || f.path().as_ref() == ".." {
                            None
                        } else {
                            Some(f)
                        }
                    });
                    let page = filtered.skip(i * N).take(N);
                    Ok(arrayvec::ArrayVec::from_iter(page))
                })
            })
            .await
            .unwrap()
    }

    async fn num_pages(&mut self) -> usize {
        self.flash
            .with_fs(|fs| fs.read_dir_and_then(self.dir, |dir_it| Ok(dir_it.count().div_ceil(N))))
            .await
            .unwrap()
    }
}

impl MenuItem for DirEntry {
    fn button_text(&self) -> &str {
        self.file_name().as_ref()
    }
}

pub async fn files(ctx: &mut Context) {
    //TODO use cstring literals when rust 1.77 is out
    ctx.flash
        .with_fs(|fs| {
            fs.create_dir_all(b"/test_dir/t1\0".try_into().unwrap())?;
            fs.create_dir_all(b"/test_dir/t2\0".try_into().unwrap())?;
            fs.write(b"/test_dir/hello.txt\0".try_into().unwrap(), b"hello")?;
            fs.write(b"/test_dir/world.txt\0".try_into().unwrap(), b"world")?;
            fs.write(b"/test_dir/empty.txt\0".try_into().unwrap(), b"")?;
            Ok(())
        })
        .await
        .unwrap();
    let mut current_dir: PathBuf = b"\0".into();
    'outer: loop {
        loop {
            let options = FileList {
                dir: &current_dir,
                flash: &mut ctx.flash,
            };

            if let MenuSelection::Item(f) = crate::apps::menu::paginated_grid_menu::<4, _, _>(
                &mut ctx.touch,
                &mut ctx.twi0,
                &mut ctx.button,
                &mut ctx.lcd,
                &mut ctx.battery,
                options,
            )
            .await
            {
                if f.metadata().is_dir() {
                    current_dir = if f.file_name() == ".." {
                        crate::println!("{}", f.path().parent().unwrap().as_ref());
                        f.path()
                            .parent()
                            .unwrap()
                            .parent()
                            .unwrap_or_else(|| b"\0".into())
                    } else {
                        f.path().into()
                    };
                    break;
                } else {
                    file_menu(ctx, f.path()).await;
                }
            } else {
                break 'outer;
            }
        }
    }
}

pub async fn file_menu(ctx: &mut Context, path: &Path) {
    #[derive(Copy, Clone)]
    enum Opt {
        Back,
        Print,
        Delete,
    }

    let options = [
        ("Print", Opt::Print),
        ("Delete", Opt::Delete),
        ("Back", Opt::Back),
    ];

    loop {
        match crate::apps::menu::paginated_grid_menu::<4, _, _>(
            &mut ctx.touch,
            &mut ctx.twi0,
            &mut ctx.button,
            &mut ctx.lcd,
            &mut ctx.battery,
            options.as_slice(),
        )
        .await
        {
            MenuSelection::HardwareButton | MenuSelection::Item((_, Opt::Back)) => {
                return;
            }
            MenuSelection::Item((_, Opt::Print)) => {
                let engine = base64::engine::GeneralPurpose::new(
                    &base64::alphabet::STANDARD,
                    base64::engine::GeneralPurposeConfig::new(),
                );
                ctx.flash
                    .with_fs(|fs| {
                        fs.open_file_and_then(path, |f| {
                            let mut bytebuf = [0u8; 32];
                            let mut base64buf = [0u8; 64];

                            crate::println!("===== {}", path.as_ref());

                            loop {
                                let i = f.read(&mut bytebuf)?;
                                if i == 0 {
                                    break;
                                }

                                let i = engine.encode_slice(&bytebuf[..i], &mut base64buf).unwrap();
                                let s = core::str::from_utf8(&base64buf[..i]).unwrap();
                                crate::println!("{}", s);
                            }
                            crate::println!("=====");

                            Ok(())
                        })
                    })
                    .await
                    .unwrap();
            }
            MenuSelection::Item((_, Opt::Delete)) => {
                let options = [("Really Delete", true), ("Back", false)].into();

                if crate::apps::menu::grid_menu(ctx, options, false).await {
                    ctx.flash.with_fs(|fs| fs.remove(path)).await.unwrap();

                    return;
                }
            }
        }
    }
}
