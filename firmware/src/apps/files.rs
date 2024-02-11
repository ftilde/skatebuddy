use littlefs2::path::PathBuf;

use crate::Context;

pub async fn files(ctx: &mut Context) {
    //TODO use cstring literals when rust 1.77 is out
    ctx.flash
        .with_fs(|fs| {
            fs.create_dir_all(b"/test_dir/t1\0".try_into().unwrap())?;
            fs.create_dir_all(b"/test_dir/t2\0".try_into().unwrap())
        })
        .await
        .unwrap();
    let mut current_dir: PathBuf = b"\0".into();
    'outer: loop {
        let options: arrayvec::ArrayVec<_, 4> = ctx
            .flash
            .with_fs(|fs| {
                fs.read_dir_and_then(&current_dir, |dir_it| {
                    let mut o = arrayvec::ArrayVec::new();
                    for f in dir_it {
                        let f = f.unwrap();
                        crate::println!("{}", f.path().as_ref());
                        if f.file_name() == "." || f.path().as_ref() == ".." {
                            continue;
                        }
                        if o.try_push(f).is_err() {
                            break;
                        }
                    }
                    Ok(o)
                })
            })
            .await
            .unwrap();

        let options: [_; 4] = core::array::from_fn(|i| {
            if let Some(f) = options.get(i) {
                (f.file_name().as_ref(), Some(f))
            } else {
                ("", None)
            }
        });
        loop {
            if let Some(f) = crate::apps::menu::grid_menu(ctx, options, None).await {
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
                    let l = f.metadata().len();
                    crate::println!("len: {}", l);
                }
            } else {
                break 'outer;
            }
        }
    }
}
