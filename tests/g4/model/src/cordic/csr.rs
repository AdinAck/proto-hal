pub mod argsize;
pub mod dmaren;
pub mod dmawen;
pub mod func;
pub mod ien;
pub mod nargs;
pub mod nres;
pub mod precision;
pub mod ressize;
pub mod rrdy;
pub mod scale;

use proto_hal_build::model::{Register, model::PeripheralEntry};

use argsize::argsize;
use dmaren::dmaren;
use dmawen::dmawen;
use func::func;
use ien::ien;
use nargs::nargs;
use nres::nres;
use precision::precision;
use ressize::ressize;
use rrdy::rrdy;
use scale::scale;

pub struct Output {
    pub argsize: argsize::Output,
    pub ressize: ressize::Output,
    pub nargs: nargs::Output,
    pub nres: nres::Output,
}

pub fn csr<'cx>(cordic: &mut PeripheralEntry<'cx>) -> Output {
    let mut csr = cordic.add_register(Register::new("csr", 0).reset(0x50));

    let [n0, n1, n2, n3, n4, ..] = scale(&mut csr);
    func(&mut csr, func::Entitlements { n0, n1, n2, n3, n4 });
    precision(&mut csr);
    ien(&mut csr);
    dmaren(&mut csr);
    dmawen(&mut csr);
    let nres = nres(&mut csr);
    let nargs = nargs(&mut csr);
    let ressize = ressize(&mut csr);
    let argsize = argsize(&mut csr);
    rrdy(&mut csr);

    Output {
        argsize,
        ressize,
        nargs,
        nres,
    }
}
