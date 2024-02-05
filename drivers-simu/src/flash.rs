use littlefs2::{driver::Storage, fs::Filesystem};

pub struct FlashRessources {
    file: memmap::MmapMut,
}

impl FlashRessources {
    pub fn new() -> Self {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("simu_flash.bin")
            .unwrap();
        file.set_len((Flash::BLOCK_COUNT * Flash::BLOCK_SIZE) as _)
            .unwrap();

        let file = unsafe { memmap::MmapMut::map_mut(&file).unwrap() };
        Self { file }
    }
    pub async fn on<'a>(&'a mut self) -> Flash<'a> {
        Flash { ressources: self }
    }

    pub async fn with_fs<'a, R>(
        &mut self,
        f: impl FnOnce(&mut Filesystem<Flash>) -> littlefs2::io::Result<R>,
    ) -> littlefs2::io::Result<R> {
        let mut flash = self.on().await;

        let mut alloc = Filesystem::allocate();
        let mut fs = Filesystem::mount(&mut alloc, &mut flash)?;
        f(&mut fs)
    }
}

pub struct Flash<'a> {
    #[allow(unused)]
    ressources: &'a mut FlashRessources,
}

const SECTOR_SIZE: usize = 4096;

impl<'a> Flash<'a> {
    pub async fn read(&mut self, addr: u32 /*actually 24 bit*/, out: &mut [u8]) {
        out.copy_from_slice(&self.ressources.file[addr as usize..][..out.len()])
    }

    pub async fn write(&mut self, addr: u32 /*actually 24 bit*/, buf: &[u8]) {
        let flash = &mut self.ressources.file[addr as usize..][..buf.len()];
        for (f, b) in flash.iter_mut().zip(buf.iter()) {
            *f = *f & b;
        }
    }

    pub async fn erase(&mut self, addr: u32 /*actually 24 bit*/) {
        let base = (addr as usize / SECTOR_SIZE) * SECTOR_SIZE;
        let sector = &mut self.ressources.file[base as usize..][..SECTOR_SIZE];
        sector.fill(0xff);
    }
}

impl<'a> littlefs2::driver::Storage for Flash<'a> {
    const READ_SIZE: usize = 4;

    const WRITE_SIZE: usize = 4;

    const BLOCK_SIZE: usize = SECTOR_SIZE;

    const BLOCK_COUNT: usize = 2048 * 4096 / Self::BLOCK_SIZE;

    type CACHE_SIZE = littlefs2::consts::U512;

    type LOOKAHEAD_SIZE = littlefs2::consts::U128;

    const BLOCK_CYCLES: isize = 50_000;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
        smol::future::block_on(self.read(off as _, buf));
        Ok(0)
    }

    fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
        smol::future::block_on(self.write(off as _, data));
        Ok(0)
    }

    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        let mut i = 0;
        while i < len {
            smol::future::block_on(self.erase((off + i) as _));
            i += SECTOR_SIZE;
        }
        Ok(0)
    }
}
