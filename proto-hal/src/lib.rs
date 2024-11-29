#![no_std]

pub mod regs;

pub mod gpio;

#[cfg(feature = "stm32")]
pub mod stm32;

pub use macros;
pub mod macro_utils;
pub mod prelude;

/// Types that encapsulate a resource that can be configured to be
/// in a "reset" state implement this trait.
pub trait IntoReset {
    /// The form of the implementor type in the "reset" state.
    type Reset;

    /// Transform the implementor type into the "reset" state.
    fn into_reset(self) -> Self::Reset;
}

#[cfg(test)]
mod tests {
    mod macros {
        use crate::prelude::*;
        use macros::block;

        // mod syscfg {
        //     use super::*;

        //     #[field(width = 3)]
        //     enum MemMode {
        //         MainFlash,
        //         SystemFlash,
        //         Fsmc,
        //         SRam1,
        //         QuadSpi,
        //     }

        //     mod mem_mode {
        //         struct Spec;

        //         unsafe impl ::proto_hal::regs::field::FieldSpec for Spec {
        //             const WIDTH: u8 = 3;
        //         }

        //         enum States {
        //             MainFlash,
        //             SystemFlash,
        //             Fsmc,
        //             SRam1,
        //             QuadSpi,
        //         }
        //     }

        //     #[register(infer_offsets)]
        //     struct MemRmp {
        //         mem_mode: MemMode,

        //         #[field(offset = 0x08)]
        //         fb_mode: bool,
        //     }
        //     // struct Cfgr1;
        //     // struct ExtiCr1;
        //     // struct ExtiCr2;

        //     // #[block(base_addr = 0x4001_0000, infer_offsets)]
        //     // struct SysCfg {
        //     //     memrmp: MemRmp,
        //     //     cfgr1: Cfgr1,
        //     //     exticr1: ExtiCr1,
        //     //     exticr2: ExtiCr2,
        //     // }
        // }

        // pub mod cordic {
        //     use super::*;

        //     #[states(width = 4)]
        //     pub enum Func {
        //         #[state(entitlements = [scale::N0])]
        //         Cos,
        //         #[state(entitlements = [scale::N0])]
        //         Sin,
        //         #[state(entitlements = [scale::N0])]
        //         ATan2,
        //         #[state(entitlements = [scale::N0])]
        //         Magnitude,
        //         ATan,
        //         #[state(entitlements = [scale::N1])]
        //         CosH,
        //         #[state(entitlements = [scale::N1])]
        //         SinH,
        //         #[state(entitlements = [scale::N1])]
        //         ATanH,
        //         #[state(entitlements = [scale::N1, scale::N2, scale::N3, scale::N4])]
        //         Ln,
        //         #[state(entitlements = [scale::N0, scale::N1, scale::N2])]
        //         Sqrt,
        //     }

        //     #[states(width = 4)]
        //     pub enum Precision {
        //         P4 = 1,
        //         P8,
        //         P12,
        //         P16,
        //         P20,
        //         P24,
        //         P28,
        //         P32,
        //         P36,
        //         P40,
        //         P44,
        //         P48,
        //         P52,
        //         P56,
        //         P60,
        //     }

        //     #[states(width = 3)]
        //     pub enum Scale {
        //         N0,
        //         N1,
        //         N2,
        //         N3,
        //         N4,
        //         N5,
        //         N6,
        //         N7,
        //     }

        //     #[states(width = 1)]
        //     pub enum Enable {
        //         Disabled,
        //         Enabled,
        //     }

        //     #[states(width = 1)]
        //     pub enum NData {
        //         One,
        //         Two,
        //     }

        //     #[states(width = 1)]
        //     pub enum DataSize {
        //         Q15,
        //         Q31,
        //     }

        //     #[states(width = 1)]
        //     pub enum Rrdy {
        //         NoData,
        //         Ready,
        //     }

        //     #[register(infer_offsets)]
        //     pub struct Csr {
        //         #[field(read, write, reset = Cos)]
        //         func: Func,
        //         #[field(read, write, reset = P20)]
        //         precision: Precision,
        //         #[field(read, write, reset = N0)]
        //         scale: Scale,

        //         #[field(offset = 0x10, reset = Disabled)]
        //         ien: Enable,
        //         #[field(read, write, reset = Disabled)]
        //         dmaren: Enable,
        //         #[field(read, write, reset = Disabled)]
        //         dmawen: Enable,
        //         #[field(read, write, reset = One)]
        //         nres: NData,
        //         #[field(read, write, reset = One)]
        //         nargs: NData,
        //         #[field(read, write, reset = Q15)]
        //         ressize: DataSize,
        //         #[field(read, write, reset = Q15)]
        //         argsize: DataSize,

        //         #[field(offset = 0x1f, read, reset = NoData)]
        //         rrdy: Rrdy,
        //     }

        //     #[register(infer_offsets)]
        //     pub struct WData {
        //         #[field(write(effect = unresolve(csr::rrdy)))]
        //         arg: u32,
        //     }

        //     #[register(infer_offsets)]
        //     pub struct RData {
        //         #[field(read(entitlements = [csr::rrdy::Ready], effect = unresolve(csr::rrdy)))]
        //         res: u32,
        //     }

        //     #[block(
        //         base_addr = 0x4002_1000,
        //         infer_offsets,
        //         entitlements = [super::ahb::cordic_en::Enabled]
        //     )]
        //     pub struct Cordic {
        //         csr: Csr,
        //         wdata: WData,
        //         rdata: RData,
        //     }
        // }

        #[block(
            base_addr = 0x4002_1000,
            auto_increment,
            entitlements = [super::ahb::cordic_en::Enabled]
        )]
        mod cordic {
            #[register(auto_increment)]
            mod csr {
                #[field(width = 4, read, write, auto_increment)]
                mod func {
                    #[state(entitlements = [scale::N0], reset)]
                    struct Cos;

                    #[state(entitlements = [scale::N0])]
                    struct Sin;

                    #[state(entitlements = [scale::N0])]
                    struct ATan2;

                    #[state(entitlements = [scale::N0])]
                    struct Magnitude;
                    #[state]
                    struct ATan;

                    #[state(entitlements = [scale::N1])]
                    struct CosH;

                    #[state(entitlements = [scale::N1])]
                    struct SinH;

                    #[state(entitlements = [scale::N1])]
                    struct ATanH;

                    #[state(entitlements = [scale::N1, scale::N2, scale::N3, scale::N4])]
                    struct Ln;

                    #[state(entitlements = [scale::N0, scale::N1, scale::N2])]
                    struct Sqrt;
                }

                #[field(width = 4, read, write, auto_increment)]
                mod precision {
                    #[state(bits = 0b001, reset)]
                    struct P4;
                    #[state]
                    struct P8;
                    #[state]
                    struct P12;
                    #[state]
                    struct P16;
                    #[state]
                    struct P20;
                    #[state]
                    struct P24;
                    #[state]
                    struct P28;
                    #[state]
                    struct P32;
                    #[state]
                    struct P36;
                    #[state]
                    struct P40;
                    #[state]
                    struct P44;
                    #[state]
                    struct P48;
                    #[state]
                    struct P52;
                    #[state]
                    struct P56;
                    #[state]
                    struct P60;
                }
            }

            // #[register]
            // mod wdata {
            //     #[field(offset = 0, width = 32, write(effect = unresolve(csr::rrdy)))]
            //     mod arg {
            //         #[value]
            //         struct Argument(u32);
            //     }
            // }

            // #[register]
            // mod rdata {
            //     #[field(offset = 0, width = 32, read(entitlements = [csr::rrdy::Ready], effect = unresolve(csr::rrdy)))]
            //     mod res {
            //         #[value]
            //         struct Result(u32);
            //     }
            // }
        }

        /*
        let ahb = ctx.device.ahb;

        let cordic = ctx.device.cordic;

        cordic.csr(|csr| ...); // doesn't compile

        let cordic = cordic.attach(ahb.cordic_en);

        let cordic = cordic.csr(|csr| {
            csr
                .func::<Sin>()
                .precision::<P40>()
                .transition()
        });

        let csr = cordic.csr
            .func::<Sin>()
            .precision::<P40>()
            .transition();

        let func = csr.func.lossy_transition::<Cos>();

        // re-collect
        csr.func = func.lossy_transition();
        cordic.csr = csr.transition();

        let (cordic, cordic_en) = cordic.release();
        ahb.cordic_en = cordic_en;

        */
    }
}
