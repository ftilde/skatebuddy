use littlefs2::path::Path;

use crate::Context;

pub async fn files(ctx: &mut Context) {
    'outer: loop {
        const ROOT: &Path = &Path::from_str_with_nul("/\0");
        let options: arrayvec::ArrayVec<_, 4> = ctx
            .flash
            .with_fs(|fs| {
                fs.read_dir_and_then(ROOT, |dir_it| {
                    let mut o = arrayvec::ArrayVec::new();
                    for f in dir_it {
                        if o.try_push(f.unwrap()).is_err() {
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
                (f.file_name().as_str_ref_with_trailing_nul(), Some(f))
            } else {
                ("", None)
            }
        });
        loop {
            if let Some(f) = crate::apps::menu::grid_menu(ctx, options, None).await {
                let l = f.metadata().len();
                crate::println!("len: {}", l);
            } else {
                break 'outer;
            }
        }
    }
}
